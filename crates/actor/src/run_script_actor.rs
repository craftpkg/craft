use contract::Actor;
use package::PackageJson;
use process::Process;

#[derive(Debug)]
pub struct RunScriptActorPayload {
    pub script: String,
}

impl std::fmt::Display for RunScriptActorPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.script)
    }
}

pub struct RunScriptActor {
    payload: RunScriptActorPayload,
}

impl Actor<RunScriptActorPayload> for RunScriptActor {
    fn with(payload: RunScriptActorPayload) -> Self {
        Self { payload }
    }

    async fn run(&self) -> contract::Result<()> {
        debug::info!("Running script: {}", self.payload.script);
        let mut script = self.payload.script.clone();
        let package_json = PackageJson::from_file().await?;

        if let Some(scripts) = package_json.scripts
            && let Some(package_script) = scripts.get(&script)
        {
            script = package_script.clone();
        }
        let cwd = std::env::current_dir()?;

        Process::run_script(&script, Some(cwd.to_str().unwrap_or("."))).await?;

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
            script: "echo test".to_string(),
        };

        assert_eq!(format!("{}", payload), "echo test");
    }

    #[test]
    fn test_run_script_actor_creation() {
        let payload = RunScriptActorPayload {
            script: "craft test".to_string(),
        };

        let actor = RunScriptActor::with(payload);
        assert_eq!(actor.payload.script, "craft test");
    }
}
