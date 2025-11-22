use indicatif::{ProgressBar, ProgressStyle};

pub struct Progress {
    pb: ProgressBar,
}

impl Progress {
    pub fn new() -> Self {
        let pb = ProgressBar::new(100);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
                )
                .unwrap()
                .progress_chars("#>-"),
        );
        Self { pb }
    }

    pub fn increment(&mut self, amount: u32) {
        self.pb.inc(amount as u64);
    }

    pub fn success(&self) {
        self.pb.finish_with_message("Success!");
    }

    pub fn fail(&self) {
        self.pb.abandon_with_message("Fail!");
    }
}

impl Default for Progress {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let _progress = Progress::new();
    }

    #[test]
    fn test_increment() {
        let mut progress = Progress::new();
        progress.increment(10);
    }
}
