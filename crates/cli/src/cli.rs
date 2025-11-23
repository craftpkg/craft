use std::fmt::Display;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "craft")]
#[command(about = "A package manager CLI", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(short, long, global = true)]
    pub verbose: bool,
}

impl Display for Cli {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.command)
    }
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum Commands {
    Add {
        #[arg(required = true)]
        packages: Vec<String>,
        #[arg(short = 'D', long)]
        dev: bool,
    },
    Remove {
        #[arg(required = true)]
        packages: Vec<String>,
    },
    Run {
        script: String,
    },
    Start,
    Test,
    Install,
}

impl Display for Commands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }

    pub fn normalize(&self) -> Commands {
        match &self.command {
            Some(cmd) => match cmd {
                Commands::Add { packages, dev } => Commands::Add {
                    packages: packages.clone(),
                    dev: *dev,
                },
                Commands::Remove { packages } => Commands::Remove {
                    packages: packages.clone(),
                },
                Commands::Run { script } => Commands::Run {
                    script: script.clone(),
                },
                Commands::Start => Commands::Start,
                Commands::Test => Commands::Run {
                    script: "test".to_string(),
                },
                Commands::Install => Commands::Install,
            },
            None => Commands::Install,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_packages() {
        let cli = Cli::parse_from(["craft", "add", "react", "express"]);
        match cli.command {
            Some(Commands::Add { packages, dev }) => {
                assert_eq!(packages, vec!["react", "express"]);
                assert!(!dev);
            }
            _ => panic!("Expected Add command"),
        }
    }

    #[test]
    fn test_add_dev_dependency() {
        let cli = Cli::parse_from(["craft", "add", "typescript", "-D"]);
        match cli.command {
            Some(Commands::Add { packages, dev }) => {
                assert_eq!(packages, vec!["typescript"]);
                assert!(dev);
            }
            _ => panic!("Expected Add command"),
        }
    }

    #[test]
    fn test_remove_packages() {
        let cli = Cli::parse_from(["craft", "remove", "react"]);
        match cli.command {
            Some(Commands::Remove { packages }) => {
                assert_eq!(packages, vec!["react"]);
            }
            _ => panic!("Expected Remove command"),
        }
    }

    #[test]
    fn test_run_script() {
        let cli = Cli::parse_from(["craft", "run", "build"]);
        match cli.command {
            Some(Commands::Run { script }) => {
                assert_eq!(script, "build");
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_test_command() {
        let cli = Cli::parse_from(["craft", "test"]);
        assert_eq!(cli.command, Some(Commands::Test));

        // Verify normalization
        let normalized = cli.normalize();
        match normalized {
            Commands::Run { script } => assert_eq!(script, "test"),
            _ => panic!("Expected normalized to Run {{ script: \"test\" }}"),
        }
    }

    #[test]
    fn test_default_command() {
        let cli = Cli::parse_from(["craft"]);
        assert_eq!(cli.command, None);

        // Verify normalization
        let normalized = cli.normalize();
        assert_eq!(normalized, Commands::Start);
    }
}
