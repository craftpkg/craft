use contract::{Actor, Result};

pub struct InstallActor;

impl Actor<()> for InstallActor {
    fn with(_: ()) -> Self {
        Self
    }

    async fn run(&self) -> Result<()> {
        todo!()
    }
}
