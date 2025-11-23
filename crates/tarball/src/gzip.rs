use flate2::read::GzDecoder;
use std::fs::File;
use std::path::PathBuf;
use tar::Archive;

pub async fn unzip(source: PathBuf, dest: PathBuf) -> contract::Result<()> {
    tokio::task::spawn_blocking(move || {
        let tar_gz = File::open(source)?;
        let tar = GzDecoder::new(tar_gz);
        let mut archive = Archive::new(tar);

        archive.unpack(dest)?;

        Ok(())
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;

    #[tokio::test]
    async fn test_unzip() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("test.tar.gz");
        let dest = dir.path().join("output");

        // Create a dummy tarball
        let file = File::create(&source).unwrap();
        let enc = GzEncoder::new(file, Compression::default());
        let mut tar = tar::Builder::new(enc);

        let mut header = tar::Header::new_gnu();
        header.set_path("test.txt").unwrap();
        header.set_size(12);
        header.set_cksum();

        tar.append(&header, "Hello World!".as_bytes()).unwrap();
        let enc = tar.into_inner().unwrap();
        enc.finish().unwrap();

        // Unzip
        unzip(source, dest.clone()).await.unwrap();

        // Verify
        let content = std::fs::read_to_string(dest.join("test.txt")).unwrap();
        assert_eq!(content, "Hello World!");
    }
}
