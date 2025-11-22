use contract::Pipeline;
use contract::Result;
use package::InstallPackage;
use resolver::ResolvedArtifact;
use resolver::Resolver;

pub struct InstallPipe {
    packages: Vec<InstallPackage>,
    resolver: Resolver,
}

impl InstallPipe {
    pub fn new(packages: Vec<InstallPackage>) -> Self {
        Self {
            packages,
            resolver: Resolver::new(),
        }
    }

    async fn resolve_package(&self, package: &InstallPackage) -> Result<ResolvedArtifact> {
        debug::info!("Resolving package: {package:?}");
        let pkg = self.resolver.resolve(package).await?;

        Ok(pkg)
    }
}

impl Pipeline<()> for InstallPipe {
    async fn run(&self) -> Result<()> {
        for pkg in &self.packages {
            self.resolve_package(pkg).await?;
        }

        Ok(())
    }
}
