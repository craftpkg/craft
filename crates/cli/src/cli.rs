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
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    Cache {
        #[command(subcommand)]
        command: CacheCommands,
    },
    Start,
    Test,
    Install,
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Subcommand, Debug, PartialEq, Clone)]
pub enum CacheCommands {
    Clean {
        #[arg(short, long)]
        force: bool,
    },
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
                Commands::Run { script, args } => Commands::Run {
                    script: script.clone(),
                    args: args.clone(),
                },
                Commands::Cache { command } => Commands::Cache {
                    command: command.clone(),
                },
                Commands::Start => Commands::Start,
                Commands::Test => Commands::Run {
                    script: "test".to_string(),
                    args: vec![],
                },
                Commands::Install => Commands::Install,
                Commands::External(args) => {
                    if let Some(script) = args.first() {
                        Commands::Run {
                            script: script.clone(),
                            args: args[1..].to_vec(),
                        }
                    } else {
                        Commands::Install
                    }
                }
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
            Some(Commands::Run { script, args }) => {
                assert_eq!(script, "build");
                assert!(args.is_empty());
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
            Commands::Run { script, args } => {
                assert_eq!(script, "test");
                assert!(args.is_empty());
            }
            _ => panic!("Expected normalized to Run {{ script: \"test\", args: [] }}"),
        }
    }

    #[test]
    fn test_default_to_install() {
        // craft with no args should default to install
        let cli = Cli::parse_from(["craft"]);
        let normalized = cli.normalize();
        assert_eq!(normalized, Commands::Install);
    }

    #[test]
    fn test_external_subcommand() {
        // craft tsc should be treated as external and normalized to Run
        let cli = Cli::parse_from(["craft", "tsc"]);
        match &cli.command {
            Some(Commands::External(args)) => {
                assert_eq!(args.len(), 1);
                assert_eq!(args[0], "tsc");
            }
            _ => panic!("Expected External command"),
        }

        // Verify normalization
        let normalized = cli.normalize();
        match normalized {
            Commands::Run { script, args } => {
                assert_eq!(script, "tsc");
                assert!(args.is_empty());
            }
            _ => panic!("Expected normalized to Run"),
        }
    }

    #[test]
    fn test_external_subcommand_with_args() {
        // craft tsc --version should be treated as external with args
        let cli = Cli::parse_from(["craft", "tsc", "--version", "--help"]);
        match &cli.command {
            Some(Commands::External(args)) => {
                assert_eq!(args.len(), 3);
                assert_eq!(args[0], "tsc");
                assert_eq!(args[1], "--version");
                assert_eq!(args[2], "--help");
            }
            _ => panic!("Expected External command"),
        }

        // Verify normalization
        let normalized = cli.normalize();
        match normalized {
            Commands::Run { script, args } => {
                assert_eq!(script, "tsc");
                assert_eq!(args, vec!["--version", "--help"]);
            }
            _ => panic!("Expected normalized to Run"),
        }
    }

    #[test]
    fn test_default_command() {
        let cli = Cli::parse_from(["craft"]);
        assert_eq!(cli.command, None);

        // Verify normalization
        let normalized = cli.normalize();
        assert_eq!(normalized, Commands::Install);
    }
}
