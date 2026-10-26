use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Cursor, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context as _, Result};
use package::PackageJson;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

const FORMAT: &str = "craft-packument-v1";

/// A package's registry metadata as resolving needs it: its dist-tags and published versions,
/// with every version's manifest left unparsed until one is chosen.
///
/// The encoding is a format line, a JSON header holding the dist-tags and where each manifest
/// lies, then the manifests exactly as the registry sent them.
#[derive(Debug)]
pub(crate) struct Packument {
    pub dist_tags: HashMap<String, String>,
    pub versions: Vec<String>,
    source: Source,
}

#[derive(Debug, Clone)]
enum Source {
    Cached(PathBuf),
    Memory(Arc<Vec<u8>>),
}

#[derive(Serialize, Deserialize)]
struct Header {
    #[serde(rename = "distTags")]
    dist_tags: HashMap<String, String>,
    /// Each version's manifest as its offset after the header and its length.
    versions: BTreeMap<String, (u64, u64)>,
}

#[derive(Deserialize)]
struct RegistryPackument<'a> {
    #[serde(rename = "dist-tags", default)]
    dist_tags: HashMap<String, String>,
    #[serde(borrow, default)]
    versions: HashMap<String, &'a RawValue>,
}

impl Packument {
    /// Encodes a packument as the registry sent it.
    pub fn encode(body: &[u8]) -> Result<Vec<u8>> {
        let packument: RegistryPackument = serde_json::from_slice(body)?;
        let mut versions: Vec<(String, &RawValue)> = packument.versions.into_iter().collect();
        versions.sort_by(|a, b| a.0.cmp(&b.0));

        let mut manifests = Vec::new();
        let mut index = BTreeMap::new();
        for (version, manifest) in versions {
            let bytes = manifest.get().as_bytes();
            index.insert(version, (manifests.len() as u64, bytes.len() as u64));
            manifests.extend_from_slice(bytes);
        }
        let header = serde_json::to_vec(&Header {
            dist_tags: packument.dist_tags,
            versions: index,
        })?;

        let mut encoded = Vec::with_capacity(FORMAT.len() + header.len() + manifests.len() + 2);
        encoded.extend_from_slice(FORMAT.as_bytes());
        encoded.push(b'\n');
        encoded.extend_from_slice(&header);
        encoded.push(b'\n');
        encoded.extend_from_slice(&manifests);
        Ok(encoded)
    }

    /// Reads the header of an encoded packument file, leaving its manifests on disk.
    pub async fn cached(path: PathBuf) -> Result<Self> {
        let source = Source::Cached(path);
        let header = source
            .clone()
            .read(|reader| Ok(read_header(reader)?.0))
            .await?;
        Ok(Self::with_header(header, source))
    }

    pub fn in_memory(encoded: Vec<u8>) -> Result<Self> {
        let header = read_header(&mut Cursor::new(&encoded))?.0;
        Ok(Self::with_header(header, Source::Memory(Arc::new(encoded))))
    }

    /// The manifest of `version`, parsed only now. The header is read again from the same open
    /// file, so a packument another install rewrote meanwhile is still read consistently.
    pub async fn manifest(&self, version: &str) -> Result<PackageJson> {
        let version = version.to_string();
        self.source
            .clone()
            .read(move |reader| {
                let (header, start) = read_header(reader)?;
                let (offset, length) = *header
                    .versions
                    .get(&version)
                    .with_context(|| format!("version {version} is no longer published"))?;
                reader.seek(SeekFrom::Start(start + offset))?;
                let mut manifest = vec![0; usize::try_from(length)?];
                reader.read_exact(&mut manifest)?;
                Ok(serde_json::from_slice(&manifest)?)
            })
            .await
    }

    fn with_header(header: Header, source: Source) -> Self {
        Self {
            dist_tags: header.dist_tags,
            versions: header.versions.into_keys().collect(),
            source,
        }
    }
}

impl Source {
    async fn read<T: Send + 'static>(
        self,
        read: impl FnOnce(&mut dyn ReadSeek) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        tokio::task::spawn_blocking(move || match self {
            Self::Cached(path) => {
                let mut reader = BufReader::new(open(&path)?);
                read(&mut reader)
            }
            Self::Memory(encoded) => read(&mut Cursor::new(encoded.as_slice())),
        })
        .await?
    }
}

trait ReadSeek: BufRead + Seek {}

impl<T: BufRead + Seek> ReadSeek for T {}

fn open(path: &Path) -> Result<File> {
    File::open(path).with_context(|| format!("could not read {}", path.display()))
}

/// The header, and where the manifests start.
fn read_header(reader: &mut dyn ReadSeek) -> Result<(Header, u64)> {
    let mut format = String::new();
    reader.read_line(&mut format)?;
    if format.trim_end() != FORMAT {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "not an encoded packument").into());
    }
    let mut header = Vec::new();
    reader.read_until(b'\n', &mut header)?;
    let start = (format.len() + header.len()) as u64;
    Ok((serde_json::from_slice(&header)?, start))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn registry_body() -> Vec<u8> {
        json!({
            "name": "pkg",
            "dist-tags": { "latest": "1.1.0" },
            "versions": {
                "1.0.0": { "name": "pkg", "version": "1.0.0", "dependencies": { "a": "^1.0.0" } },
                "1.1.0": { "name": "pkg", "version": "1.1.0", "description": "line\nbreak" },
            },
        })
        .to_string()
        .into_bytes()
    }

    #[tokio::test]
    async fn test_reads_only_the_chosen_manifest_back() {
        let packument = Packument::in_memory(Packument::encode(&registry_body()).unwrap()).unwrap();

        assert_eq!(packument.versions, ["1.0.0", "1.1.0"]);
        assert_eq!(packument.dist_tags["latest"], "1.1.0");
        let manifest = packument.manifest("1.0.0").await.unwrap();
        assert_eq!(manifest.version.as_deref(), Some("1.0.0"));
        assert_eq!(manifest.dependencies.unwrap()["a"], "^1.0.0");
        assert!(packument.manifest("2.0.0").await.is_err());
    }

    #[tokio::test]
    async fn test_reads_manifests_from_the_file_as_it_is_now() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pkg.packument");
        std::fs::write(&path, Packument::encode(&registry_body()).unwrap()).unwrap();
        let packument = Packument::cached(path.clone()).await.unwrap();

        let republished = json!({
            "dist-tags": { "latest": "1.0.0" },
            "versions": {
                "0.9.0": { "version": "0.9.0" },
                "1.0.0": { "version": "1.0.0", "description": "a longer manifest than before" },
            },
        });
        std::fs::write(
            &path,
            Packument::encode(republished.to_string().as_bytes()).unwrap(),
        )
        .unwrap();

        let manifest = packument.manifest("1.0.0").await.unwrap();
        assert_eq!(manifest.version.as_deref(), Some("1.0.0"));
        assert!(packument.manifest("1.1.0").await.is_err());
    }

    #[tokio::test]
    async fn test_rejects_files_in_another_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pkg.json");
        std::fs::write(&path, registry_body()).unwrap();

        assert!(Packument::cached(path).await.is_err());
    }
}
