use std::fmt::Display;

use contract::{Actor, Pipeline};
use package::InstallPackage;
use pipeline::{InstallPipe, LinkerPipe};

#[derive(Debug)]
pub struct AddActorPayload {
    pub packages: Vec<String>,
    pub is_dev: bool,
}

impl Display for AddActorPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.packages.join(" "))
    }
}

pub struct AddPackageActor {
    payload: AddActorPayload,
}

impl Actor<AddActorPayload> for AddPackageActor {
    fn with(payload: AddActorPayload) -> Self {
        Self { payload }
    }

    async fn run(&self) -> contract::Result<()> {
        let mut pkgs = Vec::new();

        for pkg in &self.payload.packages {
            pkgs.push(InstallPackage::from_literal(pkg, self.payload.is_dev));
        }

        debug::trace!("Installing packages: {pkgs:?}");

        let artifacts = InstallPipe::new(pkgs.clone()).run().await?;
        LinkerPipe::new(artifacts, pkgs).run().await?;

        Ok(())
    }
}
