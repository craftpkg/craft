use contract::{Pipeline, Result};
use lockfile::{Lockfile, PackageEntry};
use resolver::ResolvedArtifact;

pub struct LockfileGeneratorPipe {
    artifacts: Vec<ResolvedArtifact>,
}

impl LockfileGeneratorPipe {
    pub fn new(artifacts: Vec<ResolvedArtifact>) -> Self {
        Self { artifacts }
    }
}

impl Pipeline<()> for LockfileGeneratorPipe {
    async fn run(&self) -> Result<()> {
        let mut lockfile = Lockfile::new();

        for artifact in &self.artifacts {
            let entry = PackageEntry {
                name: artifact.name.clone(),
                version: artifact.version.clone(),
                resolved: artifact.download_url.clone(),
                integrity: artifact.integrity.clone(),
                dependencies: artifact
                    .package
                    .as_ref()
                    .and_then(|p| p.dependencies.clone()),
            };
            lockfile.add_package(entry);
        }

        let cwd = std::env::current_dir()?;
        let lockfile_path = cwd.join("craft.bin");

        debug::info!("Generating lockfile at {:?}", lockfile_path);
        lockfile.save_binary(&lockfile_path).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_lockfile_generation() -> Result<()> {
        let temp_dir = tempdir()?;
        env::set_current_dir(&temp_dir)?;

        let artifact = ResolvedArtifact {
            name: "test-pkg".to_string(),
            version: "1.0.0".to_string(),
            download_url: "http://example.com".to_string(),
            integrity: Some("sha512-test".to_string()),
            package: None,
        };

        let pipe = LockfileGeneratorPipe::new(vec![artifact]);
        pipe.run().await?;

        let lockfile_path = temp_dir.path().join("craft.bin");
        assert!(lockfile_path.exists());

        let loaded = Lockfile::load_binary(&lockfile_path).await?;
        assert!(loaded.has_package("test-pkg", "1.0.0"));

        Ok(())
    }
}
