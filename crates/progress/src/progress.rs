use std::sync::{Arc, Mutex};
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

const TICK: Duration = Duration::from_millis(80);

/// How much an install tells the user while it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// A live status line on a terminal, plus the summary.
    Interactive,
    /// The summary only: for pipes, CI and debug logs, where a live line would garble output.
    Plain,
    /// Nothing but warnings and errors.
    Silent,
}

/// Reports install progress and results. Cheap to clone; every clone drives the same line.
#[derive(Clone)]
pub struct Reporter {
    mode: OutputMode,
    bar: Arc<Mutex<Option<ProgressBar>>>,
}

impl std::fmt::Debug for Reporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reporter")
            .field("mode", &self.mode)
            .finish()
    }
}

impl Default for Reporter {
    fn default() -> Self {
        Self::new(OutputMode::Silent)
    }
}

impl Reporter {
    pub fn new(mode: OutputMode) -> Self {
        Self {
            mode,
            bar: Arc::new(Mutex::new(None)),
        }
    }

    /// Shows `message` on the live status line, starting it if needed.
    pub fn status(&self, message: impl Into<String>) {
        if self.mode != OutputMode::Interactive {
            return;
        }
        let mut bar = self.lock();
        let bar = bar.get_or_insert_with(|| {
            let bar = ProgressBar::new_spinner();
            bar.set_style(
                ProgressStyle::with_template("{spinner} {msg}")
                    .unwrap_or_else(|_| ProgressStyle::default_spinner()),
            );
            bar.enable_steady_tick(TICK);
            bar
        });
        bar.set_message(message.into());
    }

    /// Prints a result line, above the status line while one is shown.
    pub fn line(&self, line: impl AsRef<str>) {
        if self.mode == OutputMode::Silent {
            return;
        }
        match self.lock().as_ref() {
            Some(bar) => bar.println(line.as_ref()),
            None => println!("{}", line.as_ref()),
        }
    }

    /// Prints a warning to stderr, pausing the status line so the two don't interleave.
    /// Warnings show even in silent mode.
    pub fn warn(&self, message: impl AsRef<str>) {
        let message = message.as_ref();
        match self.lock().as_ref() {
            Some(bar) => bar.suspend(|| eprintln!("warning: {message}")),
            None => eprintln!("warning: {message}"),
        }
    }

    /// Clears the status line.
    pub fn finish(&self) {
        if let Some(bar) = self.lock().take() {
            bar.finish_and_clear();
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<ProgressBar>> {
        self.bar
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_only_interactive_mode_draws_a_status_line() {
        let plain = Reporter::new(OutputMode::Plain);
        plain.status("Resolving");
        assert!(plain.lock().is_none());

        let interactive = Reporter::new(OutputMode::Interactive);
        interactive.status("Resolving");
        assert!(interactive.lock().is_some());
        interactive.finish();
        assert!(interactive.lock().is_none());
    }
}
