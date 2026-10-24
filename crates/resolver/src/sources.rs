use std::path::{Path, PathBuf};

use anyhow::{Context as _, anyhow, bail};
use contract::{Context, PackageError, Result};
use network::{Integrity, Network};
use package::PackageJson;
use tarball::gzip::unzip;

use crate::git::{GitSpec, checkout, resolve_commit};
use crate::resolved_artifact::{git_package_dir, short_hash};
use crate::{ArtifactSource, ResolvedArtifact};

const COPY_EXCLUDED: [&str; 2] = ["node_modules", ".git"];
const UNVERSIONED: &str = "0.0.0";

/// `file:<dir>`: read in place; the linker copies it into the project on every install, so
/// edits to the source show up.
pub(crate) async fn resolve_directory(
    requested_name: Option<&str>,
    path: PathBuf,
) -> Result<ResolvedArtifact> {
    let manifest = read_manifest(&path)
        .await?
        .ok_or_else(|| anyhow!("{} has no package.json", path.display()))?;
    artifact_from(
        Some(manifest),
        requested_name,
        ArtifactSource::Directory { path },
        None,
    )
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
    let cache_dir = context.store_dir();
    tokio::fs::create_dir_all(&cache_dir).await?;
    let tarball_path = cache_dir.join(format!("tarball-url-{}.tgz", short_hash(url)));
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

/// A git dependency, pinned to the commit its committish names. Hosted repositories are
/// downloaded as the host's tarball of that commit; others, or hosted ones the tarball can't be
/// fetched for (private repositories), are checked out with git.
pub(crate) async fn resolve_git(
    context: &Context,
    network: &Network,
    requested_name: Option<&str>,
    spec: &str,
) -> Result<ResolvedArtifact> {
    let git_spec = GitSpec::parse(spec).ok_or_else(|| anyhow!("invalid git dependency: {spec}"))?;
    let sha = resolve_commit(&git_spec).await?;
    let package_dir = git_package_dir(context, &git_spec.clone_url, &sha);

    if !package_dir.exists() {
        let downloaded = match &git_spec.hosted {
            Some(hosted) => {
                download_tarball(context, network, &hosted.tarball_url(&sha), &package_dir).await
            }
            None => Err(anyhow!("{} is not a hosted repository", git_spec.clone_url)),
        };
        if let Err(error) = downloaded {
            debug::info!("Checking out {} with git: {error:#}", git_spec.clone_url);
            check_out_into(context, &git_spec.clone_url, &sha, &package_dir).await?;
        }
    }

    let manifest = read_manifest(&package_dir).await?;
    let source = ArtifactSource::Git {
        url: git_spec.clone_url,
        sha,
    };
    artifact_from(manifest, requested_name, source, None)
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

async fn download_tarball(
    context: &Context,
    network: &Network,
    url: &str,
    package_dir: &Path,
) -> Result<()> {
    let tarball_path = context
        .store_dir()
        .join(format!("git-{}.tgz", short_hash(url)));
    tokio::fs::create_dir_all(context.store_dir()).await?;
    network.download(url, tarball_path.clone(), None).await?;
    let extracted = unzip(tarball_path.clone(), package_dir.to_path_buf()).await;
    let _ = tokio::fs::remove_file(&tarball_path).await;
    extracted
}

async fn check_out_into(
    context: &Context,
    clone_url: &str,
    sha: &str,
    package_dir: &Path,
) -> Result<()> {
    let cache_dir = context.store_dir();
    tokio::fs::create_dir_all(&cache_dir).await?;
    let checkout_dir = tempfile::Builder::new()
        .prefix(".git-checkout-")
        .tempdir_in(&cache_dir)?;

    checkout(clone_url, sha, checkout_dir.path()).await?;
    fsutil::replace_with_copy(checkout_dir.path(), package_dir, &COPY_EXCLUDED).await?;
    Ok(())
}

async fn extract(
    context: &Context,
    requested_name: Option<&str>,
    source: ArtifactSource,
    integrity: Integrity,
    tarball: &Path,
) -> Result<ResolvedArtifact> {
    let package_dir =
        crate::resolved_artifact::store_entry(context, "tarball", &integrity.to_string());
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
        ArtifactSource::Git { url, sha } => format!("{url}#{sha}"),
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
