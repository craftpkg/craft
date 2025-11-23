use actor::{
    AddActorPayload, AddPackageActor, InstallActor, RemoveActorPayload, RemovePackageActor,
};
use cli::Commands;
use contract::Actor;

pub struct CraftManager;

impl CraftManager {
    pub fn new() -> Self {
        Self
    }

    pub fn set_verbose(&self, verbose: bool) {
        if verbose {
            // SAFETY: We're setting a simple environment variable for this process only
            // This is safe as we control the key and value, and it doesn't affect other threads
            unsafe {
                std::env::set_var(contract::CRAFT_VERBOSE, "1");
            }
        }
    }

    pub async fn handle_command(&self, command: Commands) -> contract::Result<()> {
        match command {
            Commands::Add { packages, dev } => {
                AddPackageActor::with(AddActorPayload {
                    packages,
                    is_dev: dev,
                })
                .run()
                .await
            }
            Commands::Remove { packages } => {
                RemovePackageActor::with(RemoveActorPayload { packages })
                    .run()
                    .await
            }
            Commands::Run { script } => {
                println!("Running script: {}", script);
                Ok(())
            }
            Commands::Start => {
                println!("Starting application...");
                Ok(())
            }
            Commands::Test => {
                println!("Running tests...");
                Ok(())
            }
            Commands::Install => InstallActor::with(()).run().await,
        }
    }
}

impl Default for CraftManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cli::Commands;

    #[tokio::test]
    async fn test_handle_start_command() {
        let manager = CraftManager::new();
        // Just verifying it doesn't panic for now, as we are printing to stdout
        let _ = manager.handle_command(Commands::Start).await;
    }

    #[tokio::test]
    async fn test_handle_add_command() {
        let manager = CraftManager::new();
        let _ = manager
            .handle_command(Commands::Add {
                packages: vec!["react".to_string()],
                dev: false,
            })
            .await;
    }
}
