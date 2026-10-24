use std::collections::HashMap;
use std::path::{Path, PathBuf};

use contract::{Actor, Context, PackageError, Result, ScriptError};
use package::PackageJson;
use process::{Process, ScriptPackage};

#[derive(Debug)]
pub struct RunScriptActorPayload {
    pub script: String,
    pub args: Vec<String>,
    /// Run a dependency bin only, never a package.json script (`craft exec`).
    pub bin_only: bool,
}

impl std::fmt::Display for RunScriptActorPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.script, self.args.join(" "))
    }
}

pub struct RunScriptActor {
    context: Context,
    payload: RunScriptActorPayload,
}

impl Actor<RunScriptActorPayload> for RunScriptActor {
    fn with(context: Context, payload: RunScriptActorPayload) -> Self {
        Self { context, payload }
    }

    /// Runs the named package.json script with its `pre`/`post` scripts, or else a dependency
    /// bin of that name, the way `npm run` and `npx` resolve them.
    async fn run(&self) -> Result<()> {
        let name = &self.payload.script;
        let manifest = read_manifest(&self.context.package_json_path()).await?;
        let scripts = manifest
            .as_ref()
            .and_then(|manifest| manifest.scripts.clone())
            .unwrap_or_default();

        let runs_script = !self.payload.bin_only && scripts.contains_key(name);
        if runs_script {
            return self.run_lifecycle(name, &scripts, manifest.as_ref()).await;
        }
        if let Some(bin) = find_bin(&self.context.cwd, name) {
            debug::info!("Running bin {}", bin.display());
            let process = Process::new(&bin).args(&self.payload.args);
            return self.run_process(name, process, manifest.as_ref()).await;
        }
        if self.payload.bin_only {
            return Err(ScriptError::BinNotFound { name: name.clone() }.into());
        }

        let mut available: Vec<String> = scripts.into_keys().collect();
        available.sort();
        Err(ScriptError::NotFound {
            name: name.clone(),
            available,
        }
        .into())
    }
}

pub struct ListScriptsActor {
    context: Context,
}

impl Actor<()> for ListScriptsActor {
    fn with(context: Context, _: ()) -> Self {
        Self { context }
    }

    /// Prints each script with its command, as `npm run` does without a name.
    async fn run(&self) -> Result<()> {
        let manifest = PackageJson::from_file(&self.context.package_json_path()).await?;
        let mut scripts: Vec<(String, String)> =
            manifest.scripts.unwrap_or_default().into_iter().collect();
        if scripts.is_empty() {
            println!("No scripts in package.json");
            return Ok(());
        }
        scripts.sort();

        let project = manifest.name.unwrap_or_else(|| "this project".to_string());
        println!("Scripts in {project}:");
        for (name, command) in scripts {
            println!("  {name}\n    {command}");
        }
        Ok(())
    }
}

impl RunScriptActor {
    /// Runs `pre<name>`, `<name>` and `post<name>`, stopping at the first failure. Only the
    /// main script receives the command-line arguments.
    async fn run_lifecycle(
        &self,
        name: &str,
        scripts: &HashMap<String, String>,
        manifest: Option<&PackageJson>,
    ) -> Result<()> {
        let events = [
            format!("pre{name}"),
            name.to_string(),
            format!("post{name}"),
        ];

        for event in events {
            let Some(script) = scripts.get(&event) else {
                continue;
            };
            let command = if event == name {
                process::with_args(script, &self.payload.args, &self.context.cwd)
            } else {
                script.clone()
            };

            debug::info!("Running {}: {}", event, command);
            let process = Process::shell(&command).lifecycle(&event, script);
            self.run_process(&event, process, manifest).await?;
        }
        Ok(())
    }

    async fn run_process(
        &self,
        name: &str,
        process: Process,
        manifest: Option<&PackageJson>,
    ) -> Result<()> {
        let manifest_path = self.context.package_json_path();
        let package = ScriptPackage {
            name: manifest.and_then(|manifest| manifest.name.as_deref()),
            version: manifest.and_then(|manifest| manifest.version.as_deref()),
            manifest_path: &manifest_path,
        };
        let code = process
            .script_env(&self.context.cwd, &self.context.cwd, &package)?
            .run()
            .await?;
        if code != 0 {
            return Err(ScriptError::Failed {
                name: name.to_string(),
                code,
            }
            .into());
        }
        Ok(())
    }
}

async fn read_manifest(package_json_path: &Path) -> Result<Option<PackageJson>> {
    match PackageJson::from_file(package_json_path).await {
        Ok(manifest) => Ok(Some(manifest)),
        Err(error) if matches!(error.downcast_ref(), Some(PackageError::NoPackageJson)) => Ok(None),
        Err(error) => Err(error),
    }
}

/// The nearest `node_modules/.bin/<name>`, searching from `cwd` up through its ancestors.
fn find_bin(cwd: &Path, name: &str) -> Option<PathBuf> {
    let is_plain_name = !name.is_empty() && !name.contains(['/', '\\']) && name != "..";
    if !is_plain_name {
        return None;
    }

    cwd.ancestors()
        .map(|dir| fsutil::bin_command(&dir.join("node_modules").join(".bin"), name))
        .find(|bin| bin.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_script_actor_payload_display() {
        let payload = RunScriptActorPayload {
            script: "echo".to_string(),
            args: vec!["test".to_string()],
            bin_only: false,
        };

        assert_eq!(format!("{}", payload), "echo test");
    }

    #[test]
    fn test_run_script_actor_creation() {
        let payload = RunScriptActorPayload {
            script: "craft".to_string(),
            args: vec!["test".to_string()],
            bin_only: false,
        };

        let context = Context::new("/work/app".into(), "/home/me/.craft".into(), false);
        let actor = RunScriptActor::with(context, payload);
        assert_eq!(actor.payload.script, "craft");
        assert_eq!(actor.payload.args, vec!["test"]);
    }

    #[test]
    fn test_find_bin_searches_ancestors_and_rejects_paths() {
        let dir = tempfile::tempdir().unwrap();
        let bin_dir = dir.path().join("node_modules/.bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::write(fsutil::bin_command(&bin_dir, "tsc"), "").unwrap();
        let nested = dir.path().join("packages/web");
        std::fs::create_dir_all(&nested).unwrap();

        assert_eq!(
            find_bin(&nested, "tsc"),
            Some(fsutil::bin_command(&bin_dir, "tsc"))
        );
        assert_eq!(find_bin(&nested, "eslint"), None);
        assert_eq!(find_bin(&nested, "../.bin/tsc"), None);
    }
}
