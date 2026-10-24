use actor::{
    AddActorPayload, AddPackageActor, CleanCacheActor, CleanCacheActorPayload, InitActor,
    InstallActor, InstallOptions, ListScriptsActor, RemoveActorPayload, RemovePackageActor,
    RunScriptActor, RunScriptActorPayload,
};
use cli::{Action, CacheCommands};
use contract::{Actor, Context};

pub struct CraftManager {
    context: Context,
}

impl CraftManager {
    pub fn new(context: Context) -> Self {
        Self { context }
    }

    pub async fn handle_command(&self, action: Action) -> contract::Result<()> {
        let context = self.context.clone();

        match action {
            Action::Add {
                packages,
                dev,
                exact,
                ignore_scripts,
            } => {
                AddPackageActor::with(
                    context,
                    AddActorPayload {
                        packages,
                        is_dev: dev,
                        is_exact: exact,
                        ignore_scripts,
                    },
                )
                .run()
                .await
            }
            Action::Remove { packages } => {
                RemovePackageActor::with(context, RemoveActorPayload { packages })
                    .run()
                    .await
            }
            Action::Run { script, args } => {
                RunScriptActor::with(context, RunScriptActorPayload { script, args })
                    .run()
                    .await
            }
            Action::ListScripts => ListScriptsActor::with(context, ()).run().await,
            Action::Init => InitActor::with(context, ()).run().await,
            Action::Cache { command } => match command {
                CacheCommands::Clean { force } => {
                    CleanCacheActor::with(context, CleanCacheActorPayload { force })
                        .run()
                        .await
                }
            },
            Action::Install {
                frozen_lockfile,
                ignore_scripts,
            } => {
                let options = InstallOptions {
                    frozen_lockfile,
                    clean: false,
                    ignore_scripts,
                };
                InstallActor::with(context, options).run().await
            }
            Action::Ci { ignore_scripts } => {
                let options = InstallOptions {
                    frozen_lockfile: true,
                    clean: true,
                    ignore_scripts,
                };
                InstallActor::with(context, options).run().await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contract::Result;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_run_start_runs_start_script() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        write_package_json(
            &temp_dir,
            r#"{"scripts":{"start":"echo started > started.txt"}}"#,
        )
        .await?;

        temp_manager(&temp_dir)
            .handle_command(Action::Run {
                script: "start".to_string(),
                args: vec![],
            })
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
            .handle_command(Action::Run {
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
            .handle_command(Action::Install {
                frozen_lockfile: false,
                ignore_scripts: false,
            })
            .await;

        let error = result.err().map(|e| e.to_string());
        assert_eq!(
            error.as_deref(),
            Some("no package.json found; run `craft init` to create one")
        );
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
            .handle_command(Action::Remove {
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
        let cached_package = manager.context.store_dir().join("ms@2.1.3-0123456789ab");
        let legacy_package = manager.context.legacy_cache_dir().join("ms-2.1.3");
        tokio::fs::create_dir_all(&cached_package).await?;
        tokio::fs::create_dir_all(&legacy_package).await?;

        manager
            .handle_command(Action::Cache {
                command: CacheCommands::Clean { force: true },
            })
            .await?;

        assert!(!cached_package.exists());
        assert!(!manager.context.legacy_cache_dir().exists());
        assert!(manager.context.store_dir().exists());
        Ok(())
    }

    #[tokio::test]
    async fn test_cache_clean_without_force_keeps_cache() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let manager = temp_manager(&temp_dir);
        let cached_package = manager.context.store_dir().join("ms@2.1.3-0123456789ab");
        tokio::fs::create_dir_all(&cached_package).await?;

        manager
            .handle_command(Action::Cache {
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
