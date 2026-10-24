use actor::{
    AddActorPayload, AddPackageActor, CleanCacheActor, CleanCacheActorPayload, InstallActor,
    RemoveActorPayload, RemovePackageActor, RunScriptActor, RunScriptActorPayload,
};
use cli::{CacheCommands, Commands};
use contract::{Actor, Context};

pub struct CraftManager {
    context: Context,
}

impl CraftManager {
    pub fn new(context: Context) -> Self {
        Self { context }
    }

    pub async fn handle_command(&self, command: Commands) -> contract::Result<()> {
        let context = self.context.clone();

        match command {
            Commands::Add { packages, dev } => {
                AddPackageActor::with(
                    context,
                    AddActorPayload {
                        packages,
                        is_dev: dev,
                    },
                )
                .run()
                .await
            }
            Commands::Remove { packages } => {
                RemovePackageActor::with(context, RemoveActorPayload { packages })
                    .run()
                    .await
            }
            Commands::Run { script, args } => {
                RunScriptActor::with(context, RunScriptActorPayload { script, args })
                    .run()
                    .await
            }
            Commands::Cache { command } => match command {
                CacheCommands::Clean { force } => {
                    CleanCacheActor::with(context, CleanCacheActorPayload { force })
                        .run()
                        .await
                }
            },
            Commands::Start => {
                RunScriptActor::with(
                    context,
                    RunScriptActorPayload {
                        script: "start".to_string(),
                        args: vec![],
                    },
                )
                .run()
                .await
            }
            Commands::Test => {
                RunScriptActor::with(
                    context,
                    RunScriptActorPayload {
                        script: "test".to_string(),
                        args: vec![],
                    },
                )
                .run()
                .await
            }
            Commands::Install => InstallActor::with(context, ()).run().await,
            Commands::External(args) => {
                if let Some(script) = args.first() {
                    RunScriptActor::with(
                        context,
                        RunScriptActorPayload {
                            script: script.clone(),
                            args: args[1..].to_vec(),
                        },
                    )
                    .run()
                    .await
                } else {
                    InstallActor::with(context, ()).run().await
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cli::Commands;

    #[tokio::test]
    async fn test_handle_start_command() {
        let temp_dir = tempfile::tempdir().unwrap();
        let manager = CraftManager::new(Context::new(
            temp_dir.path().to_path_buf(),
            temp_dir.path().join(".craft"),
            false,
        ));
        // Just verifying it doesn't panic for now, as we are printing to stdout
        let _ = manager.handle_command(Commands::Start).await;
    }

    #[tokio::test]
    async fn test_handle_add_command() {
        let temp_dir = tempfile::tempdir().unwrap();
        let manager = CraftManager::new(Context::new(
            temp_dir.path().to_path_buf(),
            temp_dir.path().join(".craft"),
            false,
        ));
        let _ = manager
            .handle_command(Commands::Add {
                packages: vec!["react".to_string()],
                dev: false,
            })
            .await;
    }
}
