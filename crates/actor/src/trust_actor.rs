use contract::{Actor, Context, Result};
use package::ProjectManifest;

use crate::{InstallActor, InstallOptions};

/// Adds packages to `trustedDependencies`, then installs so their scripts run right away.
pub struct TrustActor {
    context: Context,
    packages: Vec<String>,
}

impl Actor<Vec<String>> for TrustActor {
    fn with(context: Context, packages: Vec<String>) -> Self {
        Self { context, packages }
    }

    async fn run(&self) -> Result<()> {
        let mut manifest = ProjectManifest::open(&self.context.package_json_path()).await?;
        if manifest.trust(&self.packages) {
            manifest.save().await?;
        }
        for name in &self.packages {
            self.context.reporter.line(format!("Trusted {name}"));
        }
        InstallActor::with(self.context.clone(), InstallOptions::default())
            .run()
            .await
    }
}
