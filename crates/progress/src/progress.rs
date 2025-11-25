use indicatif::{ProgressBar, ProgressStyle};
use tokio::sync::mpsc::UnboundedReceiver;

#[derive(Debug)]
pub enum ProgressMessage {
    Inc(u64),
    SetLength(u64),
    SetMessage(String),
    Println(String),
    Finish(String),
    Fail(String),
}

pub struct Progress {
    pb: ProgressBar,
}

impl Progress {
    pub fn new() -> Self {
        let pb = ProgressBar::new(100);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} {msg} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
                )
                .expect("Invalid progress bar template")
                .progress_chars("#>-"),
        );
        Self { pb }
    }

    pub async fn run(self, mut rx: UnboundedReceiver<ProgressMessage>) {
        while let Some(msg) = rx.recv().await {
            match msg {
                ProgressMessage::Inc(n) => self.pb.inc(n),
                ProgressMessage::SetLength(len) => self.pb.set_length(len),
                ProgressMessage::SetMessage(msg) => self.pb.set_message(msg),
                ProgressMessage::Println(msg) => self.pb.println(msg),
                ProgressMessage::Finish(msg) => {
                    self.pb.finish_with_message(msg);
                    break;
                }
                ProgressMessage::Fail(msg) => {
                    self.pb.abandon_with_message(msg);
                    break;
                }
            }
        }
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
}
