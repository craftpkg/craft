use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};

use anyhow::Context;
use contract::Result;
use tokio::process::Command;

/// Exit code reported for a child that ended without one and without a known signal.
const UNKNOWN_FAILURE_CODE: i32 = 1;
/// Shells report a child killed by signal N as 128 + N.
const SIGNAL_EXIT_BASE: i32 = 128;

pub struct Process {
    command: OsString,
    args: Vec<OsString>,
    cwd: Option<PathBuf>,
    envs: Vec<(OsString, OsString)>,
}

impl Process {
    pub fn new(command: impl AsRef<OsStr>) -> Self {
        Self {
            command: command.as_ref().to_os_string(),
            args: Vec::new(),
            cwd: None,
            envs: Vec::new(),
        }
    }

    /// Runs `script` through the platform shell: `sh -c` on Unix, `cmd /d /s /c` on Windows.
    pub fn shell(script: &str) -> Self {
        #[cfg(unix)]
        let process = Self::new("sh").arg("-c");
        #[cfg(windows)]
        let process = Self::new("cmd").args(["/d", "/s", "/c"]);

        process.arg(script)
    }

    pub fn arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args
            .extend(args.into_iter().map(|arg| arg.as_ref().to_os_string()));
        self
    }

    pub fn current_dir(mut self, dir: &Path) -> Self {
        self.cwd = Some(dir.to_path_buf());
        self
    }

    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.envs
            .push((key.as_ref().to_os_string(), value.as_ref().to_os_string()));
        self
    }

    /// Runs to completion with inherited stdio and returns the exit code. Ctrl-C reaches the
    /// child through the terminal's process group, so craft keeps waiting instead of dying
    /// first and orphaning it.
    pub async fn run(self) -> Result<i32> {
        let mut command = Command::new(&self.command);
        command
            .args(&self.args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        if let Some(cwd) = &self.cwd {
            command.current_dir(cwd);
        }
        for (key, value) in &self.envs {
            command.env(key, value);
        }

        let mut child = command
            .spawn()
            .with_context(|| format!("failed to start {}", Path::new(&self.command).display()))?;

        let status = loop {
            tokio::select! {
                status = child.wait() => break status?,
                _ = tokio::signal::ctrl_c() => {}
            }
        };
        Ok(exit_code(status))
    }
}

#[cfg(unix)]
fn exit_code(status: ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;

    match (status.code(), status.signal()) {
        (Some(code), _) => code,
        (None, Some(signal)) => SIGNAL_EXIT_BASE + signal,
        (None, None) => UNKNOWN_FAILURE_CODE,
    }
}

#[cfg(not(unix))]
fn exit_code(status: ExitStatus) -> i32 {
    status.code().unwrap_or(UNKNOWN_FAILURE_CODE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_returns_the_exit_code() {
        assert_eq!(Process::shell("exit 0").run().await.unwrap(), 0);
        assert_eq!(Process::shell("exit 3").run().await.unwrap(), 3);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_signal_exit_follows_shell_convention() {
        assert_eq!(Process::shell("kill -TERM $$").run().await.unwrap(), 143);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_passes_env_and_cwd() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("marker"), "").unwrap();

        let code = Process::shell("test \"$GREETING\" = hi && test -f marker")
            .env("GREETING", "hi")
            .current_dir(dir.path())
            .run()
            .await
            .unwrap();

        assert_eq!(code, 0);
    }

    #[tokio::test]
    async fn test_missing_command_is_an_error() {
        let error = Process::new("craft-test-no-such-command")
            .run()
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "failed to start craft-test-no-such-command"
        );
    }
}
