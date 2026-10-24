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
    use contract::Result;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_start_runs_start_script() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        write_package_json(
            &temp_dir,
            r#"{"scripts":{"start":"echo started > started.txt"}}"#,
        )
        .await?;

        temp_manager(&temp_dir)
            .handle_command(Commands::Start)
            .await?;

        assert!(temp_dir.path().join("started.txt").exists());
        Ok(())
    }

    #[tokio::test]
    async fn test_run_runs_named_script() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        write_package_json(
            &temp_dir,
            r#"{"scripts":{"greet":"echo hello > greeted.txt"}}"#,
        )
        .await?;

        temp_manager(&temp_dir)
            .handle_command(Commands::Run {
                script: "greet".to_string(),
                args: vec![],
            })
            .await?;

        assert!(temp_dir.path().join("greeted.txt").exists());
        Ok(())
    }

    #[tokio::test]
    async fn test_install_without_package_json_fails() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;

        let result = temp_manager(&temp_dir)
            .handle_command(Commands::Install)
            .await;

        let error = result.err().map(|e| e.to_string());
        assert_eq!(error.as_deref(), Some("no package.json found"));
        Ok(())
    }

    #[tokio::test]
    async fn test_remove_drops_dependency_from_package_json() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        write_package_json(
            &temp_dir,
            r#"{"dependencies":{"ms":"^2.1.3","lodash":"^4.17.21"}}"#,
        )
        .await?;

        temp_manager(&temp_dir)
            .handle_command(Commands::Remove {
                packages: vec!["ms".to_string()],
            })
            .await?;

        let content = tokio::fs::read_to_string(temp_dir.path().join("package.json")).await?;
        let package_json: serde_json::Value = serde_json::from_str(&content)?;
        assert!(package_json["dependencies"].get("ms").is_none());
        assert!(package_json["dependencies"].get("lodash").is_some());
        Ok(())
    }

    #[tokio::test]
    async fn test_cache_clean_with_force_empties_cache() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let manager = temp_manager(&temp_dir);
        let cached_package = manager.context.package_cache_dir().join("ms-2.1.3");
        tokio::fs::create_dir_all(&cached_package).await?;

        manager
            .handle_command(Commands::Cache {
                command: CacheCommands::Clean { force: true },
            })
            .await?;

        assert!(!cached_package.exists());
        assert!(manager.context.package_cache_dir().exists());
        Ok(())
    }

    #[tokio::test]
    async fn test_cache_clean_without_force_keeps_cache() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let manager = temp_manager(&temp_dir);
        let cached_package = manager.context.package_cache_dir().join("ms-2.1.3");
        tokio::fs::create_dir_all(&cached_package).await?;

        manager
            .handle_command(Commands::Cache {
                command: CacheCommands::Clean { force: false },
            })
            .await?;

        assert!(cached_package.exists());
        Ok(())
    }

    fn temp_manager(temp_dir: &TempDir) -> CraftManager {
        CraftManager::new(Context::new(
            temp_dir.path().to_path_buf(),
            temp_dir.path().join(".craft"),
            false,
        ))
    }

    async fn write_package_json(temp_dir: &TempDir, content: &str) -> Result<()> {
        tokio::fs::write(temp_dir.path().join("package.json"), content).await?;
        Ok(())
    }
}
