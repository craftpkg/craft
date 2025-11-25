use anyhow::Context;
use contract::Result;
use std::process::Stdio;
use tokio::process::Command;

pub struct Process {
    command: String,
    args: Vec<String>,
    cwd: Option<String>,
    envs: Vec<(String, String)>,
}

impl Process {
    pub fn new(command: &str) -> Self {
        Self {
            command: command.to_string(),
            args: Vec::new(),
            cwd: None,
            envs: Vec::new(),
        }
    }

    pub fn arg(mut self, arg: &str) -> Self {
        self.args.push(arg.to_string());
        self
    }

    pub fn args(mut self, args: &[&str]) -> Self {
        self.args.extend(args.iter().map(|s| s.to_string()));
        self
    }

    pub fn current_dir(mut self, dir: &str) -> Self {
        self.cwd = Some(dir.to_string());
        self
    }

    pub fn env(mut self, key: &str, val: &str) -> Self {
        self.envs.push((key.to_string(), val.to_string()));
        self
    }

    pub async fn run(self) -> Result<()> {
        let mut cmd = Command::new(&self.command);

        cmd.args(&self.args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        if let Some(cwd) = self.cwd {
            cmd.current_dir(cwd);
        }

        for (key, val) in self.envs {
            cmd.env(key, val);
        }

        let status = cmd
            .status()
            .await
            .with_context(|| format!("Failed to execute command: {}", self.command))?;

        if !status.success() {
            anyhow::bail!("Command failed with status: {}", status);
        }

        Ok(())
    }

    /// Run a script using the platform-specific shell.
    /// On Unix systems (Linux, macOS), uses `sh -c`.
    /// On Windows, uses `cmd /C`.
    pub async fn run_script(script: &str, cwd: Option<&str>) -> Result<()> {
        #[cfg(target_family = "unix")]
        let (shell, arg) = ("sh", "-c");

        #[cfg(target_family = "windows")]
        let (shell, arg) = ("cmd", "/C");

        let mut process = Self::new(shell).arg(arg).arg(script);

        if let Some(dir) = cwd {
            process = process.current_dir(dir);
        }

        process.run().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_process_run() {
        let result = Process::new("echo").arg("hello").run().await;

        assert!(result.is_ok());
    }
}
