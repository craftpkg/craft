use std::path::{Path, PathBuf};

use anyhow::{Context as _, anyhow, bail};
use contract::{Context, PackageError, Result};
use network::{Integrity, Network};
use package::PackageJson;
use tarball::gzip::unzip;

use crate::resolved_artifact::source_hash;
use crate::{ArtifactSource, ResolvedArtifact};

const COPY_EXCLUDED: [&str; 2] = ["node_modules", ".git"];
const UNVERSIONED: &str = "0.0.0";

/// `file:<dir>`: copied into the cache on every install, so edits to the source show up.
pub(crate) async fn resolve_directory(
    context: &Context,
    requested_name: Option<&str>,
    path: PathBuf,
) -> Result<ResolvedArtifact> {
    let manifest = read_manifest(&path)
        .await?
        .ok_or_else(|| anyhow!("{} has no package.json", path.display()))?;
    let artifact = artifact_from(
        Some(manifest),
        requested_name,
        ArtifactSource::Directory { path: path.clone() },
        None,
    )?;

    fsutil::replace_with_copy(&path, &artifact.package_dir(context), &COPY_EXCLUDED)
        .await
        .with_context(|| format!("failed to copy {}", path.display()))?;
    Ok(artifact)
}

/// `link:<dir>`: used in place; its own dependencies are the linked project's business.
pub(crate) async fn resolve_link(
    requested_name: Option<&str>,
    path: PathBuf,
) -> Result<ResolvedArtifact> {
    let is_dir = tokio::fs::metadata(&path)
        .await
        .is_ok_and(|metadata| metadata.is_dir());
    if !is_dir {
        bail!("link target {} is not a directory", path.display());
    }

    let manifest = read_manifest(&path).await?;
    artifact_from(
        manifest,
        requested_name,
        ArtifactSource::Link { path },
        None,
    )
}

/// `file:<tarball>`: extracted into the cache by content hash, so an unchanged file is reused.
pub(crate) async fn resolve_local_tarball(
    context: &Context,
    requested_name: Option<&str>,
    path: PathBuf,
) -> Result<ResolvedArtifact> {
    let bytes = tokio::fs::read(&path)
        .await
        .with_context(|| format!("failed to read {}", path.display()))?;
    let integrity = Integrity::sha512_of(&bytes);
    extract(
        context,
        requested_name,
        ArtifactSource::LocalTarball { path: path.clone() },
        integrity,
        &path,
    )
    .await
}

/// A tarball URL: downloaded once per URL, then extracted by content hash.
pub(crate) async fn resolve_remote_tarball(
    context: &Context,
    network: &Network,
    requested_name: Option<&str>,
    url: &str,
) -> Result<ResolvedArtifact> {
    let cache_dir = context.package_cache_dir();
    tokio::fs::create_dir_all(&cache_dir).await?;
    let tarball_path = cache_dir.join(format!("tarball-{}.tgz", source_hash(url)));
    if !tarball_path.exists() {
        network
            .download(url, tarball_path.clone(), None)
            .await
            .with_context(|| format!("failed to download {url}"))?;
    }

    let bytes = tokio::fs::read(&tarball_path).await?;
    let integrity = Integrity::sha512_of(&bytes);
    extract(
        context,
        requested_name,
        ArtifactSource::Tarball {
            url: url.to_string(),
        },
        integrity,
        &tarball_path,
    )
    .await
}

/// Where a `file:` or `link:` path points: relative paths are taken from `base_dir`, `~/` from
/// the user's home directory.
pub(crate) fn resolve_path(base_dir: &Path, path: &str) -> PathBuf {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
    let joined = match (path.strip_prefix("~/"), home) {
        (Some(relative), Some(home)) => PathBuf::from(home).join(relative),
        _ => base_dir.join(path),
    };
    fsutil::normalize_lexically(&joined)
}

async fn extract(
    context: &Context,
    requested_name: Option<&str>,
    source: ArtifactSource,
    integrity: Integrity,
    tarball: &Path,
) -> Result<ResolvedArtifact> {
    let package_dir =
        crate::resolved_artifact::source_cache_dir(context, "tarball", &integrity.to_string());
    if !package_dir.exists() {
        unzip(tarball.to_path_buf(), package_dir.clone()).await?;
    }

    let manifest = read_manifest(&package_dir).await?;
    artifact_from(manifest, requested_name, source, Some(integrity))
}

/// Names the package after its own `package.json`, falling back to what the dependent called
/// it; a bare location such as `craft add ./lib` has only the former.
fn artifact_from(
    manifest: Option<PackageJson>,
    requested_name: Option<&str>,
    source: ArtifactSource,
    integrity: Option<Integrity>,
) -> Result<ResolvedArtifact> {
    let manifest_name = manifest.as_ref().and_then(|manifest| manifest.name.clone());
    let name = manifest_name
        .or_else(|| requested_name.map(str::to_string))
        .ok_or_else(|| anyhow!("{} has no package name", describe(&source)))?;
    let version = manifest
        .as_ref()
        .and_then(|manifest| manifest.version.clone())
        .unwrap_or_else(|| UNVERSIONED.to_string());

    Ok(ResolvedArtifact {
        name,
        version,
        source,
        integrity,
        package: manifest,
    })
}

async fn read_manifest(dir: &Path) -> Result<Option<PackageJson>> {
    match PackageJson::from_file(&dir.join("package.json")).await {
        Ok(manifest) => Ok(Some(manifest)),
        Err(error) if matches!(error.downcast_ref(), Some(PackageError::NoPackageJson)) => Ok(None),
        Err(error) => Err(error),
    }
}

fn describe(source: &ArtifactSource) -> String {
    match source {
        ArtifactSource::Registry { tarball_url } | ArtifactSource::Tarball { url: tarball_url } => {
            tarball_url.clone()
        }
        ArtifactSource::LocalTarball { path }
        | ArtifactSource::Directory { path }
        | ArtifactSource::Link { path } => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_path_normalizes_relative_paths() {
        assert_eq!(
            resolve_path(Path::new("/work/app"), "./libs/../libs/local-lib"),
            PathBuf::from("/work/app/libs/local-lib")
        );
        assert_eq!(
            resolve_path(Path::new("/work/app"), "/opt/lib"),
            PathBuf::from("/opt/lib")
        );
    }

    #[tokio::test]
    async fn test_link_without_package_json_uses_the_requested_name() {
        let dir = tempfile::tempdir().unwrap();

        let artifact = resolve_link(Some("local"), dir.path().to_path_buf())
            .await
            .unwrap();

        assert_eq!(artifact.name, "local");
        assert_eq!(artifact.version, UNVERSIONED);
    }

    #[tokio::test]
    async fn test_bare_link_without_a_name_is_an_error() {
        let dir = tempfile::tempdir().unwrap();

        let error = resolve_link(None, dir.path().to_path_buf())
            .await
            .unwrap_err();

        assert!(error.to_string().ends_with("has no package name"));
    }

    #[tokio::test]
    async fn test_link_to_a_missing_directory_is_an_error() {
        let error = resolve_link(Some("x"), PathBuf::from("/nonexistent/x"))
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "link target /nonexistent/x is not a directory"
        );
    }
}
