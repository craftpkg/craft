use cli::Commands;

pub struct CraftManager;

impl CraftManager {
    pub fn new() -> Self {
        Self
    }

    pub async fn handle_command(&self, command: Commands) {
        match command {
            Commands::Add { packages, dev } => {
                let dep_type = if dev {
                    "dev dependencies"
                } else {
                    "dependencies"
                };
                println!("Adding {} to package: {:?}", dep_type, packages);
            }
            Commands::Remove { packages } => {
                println!("Removing packages: {:?}", packages);
            }
            Commands::Run { script } => {
                println!("Running script: {}", script);
            }
            Commands::Start => {
                println!("Starting application...");
            }
            Commands::Test => {
                println!("Running tests...");
            }
            Commands::Install => {
                println!("Install All");
            }
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
        manager.handle_command(Commands::Start).await;
    }

    #[tokio::test]
    async fn test_handle_add_command() {
        let manager = CraftManager::new();
        manager
            .handle_command(Commands::Add {
                packages: vec!["react".to_string()],
                dev: false,
            })
            .await;
    }
}
