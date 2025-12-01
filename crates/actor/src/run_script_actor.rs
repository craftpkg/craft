use contract::{Actor, Result};
use package::PackageJson;
use process::Process;

#[derive(Debug)]
pub struct RunScriptActorPayload {
    pub script: String,
    pub args: Vec<String>,
}

impl std::fmt::Display for RunScriptActorPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.script, self.args.join(" "))
    }
}

pub struct RunScriptActor {
    payload: RunScriptActorPayload,
}

impl RunScriptActor {
    async fn lookup_script(&self) -> Result<String> {
        let mut script = self.payload.script.clone();
        let package_json = PackageJson::from_file().await?;

        if let Some(scripts) = package_json.scripts
            && let Some(package_script) = scripts.get(&script)
        {
            script = package_script.clone();
        } else {
            // Check if it exists in node_modules/.bin
            let cwd = std::env::current_dir()?;
            let bin_path = cwd.join("node_modules").join(".bin").join(&script);

            if bin_path.exists() {
                script = bin_path.to_string_lossy().to_string();
            }
        }

        Ok(script)
    }
}

impl Actor<RunScriptActorPayload> for RunScriptActor {
    fn with(payload: RunScriptActorPayload) -> Self {
        Self { payload }
    }

    async fn run(&self) -> contract::Result<()> {
        debug::info!("Running script: {}", self.payload.script);
        let script = self.lookup_script().await?;

        let cwd = std::env::current_dir()?;

        let mut command_with_args = script;

        if !self.payload.args.is_empty() {
            command_with_args.push_str(" ");
            command_with_args.push_str(&self.payload.args.join(" "));
        }

        Process::run_script(&command_with_args, Some(cwd.to_str().unwrap_or("."))).await?;

        debug::info!("Script completed successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_script_actor_payload_display() {
        let payload = RunScriptActorPayload {
            script: "echo".to_string(),
            args: vec!["test".to_string()],
        };

        assert_eq!(format!("{}", payload), "echo test");
    }

    #[test]
    fn test_run_script_actor_creation() {
        let payload = RunScriptActorPayload {
            script: "craft".to_string(),
            args: vec!["test".to_string()],
        };

        let actor = RunScriptActor::with(payload);
        assert_eq!(actor.payload.script, "craft");
        assert_eq!(actor.payload.args, vec!["test"]);
    }

    // Note: Integration tests that change current_dir are disabled because
    // they interfere with parallel test execution. The script lookup and
    // execution logic is tested through the CLI integration and manual testing.
}
