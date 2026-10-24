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
        /// Save the exact resolved version instead of a caret range.
        #[arg(short = 'E', long)]
        exact: bool,
        /// Run no lifecycle scripts.
        #[arg(long)]
        ignore_scripts: bool,
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
    Install {
        /// Install exactly what craft.lock records; fail if it is missing or out of date.
        /// On by default when the CI environment variable is set.
        #[arg(long)]
        frozen_lockfile: bool,
        /// Run no lifecycle scripts.
        #[arg(long)]
        ignore_scripts: bool,
    },
    /// Remove node_modules and install exactly what craft.lock records.
    Ci {
        /// Run no lifecycle scripts.
        #[arg(long)]
        ignore_scripts: bool,
    },
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

/// A command with its shortcuts resolved: `start`, `test` and unknown subcommands become `Run`,
/// and no command becomes `Install`.
#[derive(Debug, PartialEq)]
pub enum Action {
    Add {
        packages: Vec<String>,
        dev: bool,
        exact: bool,
        ignore_scripts: bool,
    },
    Remove {
        packages: Vec<String>,
    },
    Run {
        script: String,
        args: Vec<String>,
    },
    Cache {
        command: CacheCommands,
    },
    Install {
        frozen_lockfile: bool,
        ignore_scripts: bool,
    },
    Ci {
        ignore_scripts: bool,
    },
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }

    pub fn normalize(&self) -> Action {
        let Some(command) = &self.command else {
            return Action::Install {
                frozen_lockfile: false,
                ignore_scripts: false,
            };
        };

        match command {
            Commands::Add {
                packages,
                dev,
                exact,
                ignore_scripts,
            } => Action::Add {
                packages: packages.clone(),
                dev: *dev,
                exact: *exact,
                ignore_scripts: *ignore_scripts,
            },
            Commands::Remove { packages } => Action::Remove {
                packages: packages.clone(),
            },
            Commands::Run { script, args } => Action::Run {
                script: script.clone(),
                args: args.clone(),
            },
            Commands::Cache { command } => Action::Cache {
                command: command.clone(),
            },
            Commands::Start => run_script("start", &[]),
            Commands::Test => run_script("test", &[]),
            Commands::Install {
                frozen_lockfile,
                ignore_scripts,
            } => Action::Install {
                frozen_lockfile: *frozen_lockfile,
                ignore_scripts: *ignore_scripts,
            },
            Commands::Ci { ignore_scripts } => Action::Ci {
                ignore_scripts: *ignore_scripts,
            },
            Commands::External(args) => match args.split_first() {
                Some((script, args)) => run_script(script, args),
                None => Action::Install {
                    frozen_lockfile: false,
                    ignore_scripts: false,
                },
            },
        }
    }
}

fn run_script(script: &str, args: &[String]) -> Action {
    Action::Run {
        script: script.to_string(),
        args: args.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_packages() {
        let cli = Cli::parse_from(["craft", "add", "react", "express"]);
        match cli.command {
            Some(Commands::Add { packages, dev, .. }) => {
                assert_eq!(packages, vec!["react", "express"]);
                assert!(!dev);
            }
            _ => panic!("Expected Add command"),
        }
    }

    #[test]
    fn test_add_exact() {
        let cli = Cli::parse_from(["craft", "add", "-E", "ms"]);
        assert_eq!(
            cli.normalize(),
            Action::Add {
                packages: vec!["ms".to_string()],
                dev: false,
                exact: true,
                ignore_scripts: false,
            }
        );
    }

    #[test]
    fn test_add_dev_dependency() {
        let cli = Cli::parse_from(["craft", "add", "typescript", "-D"]);
        match cli.command {
            Some(Commands::Add { packages, dev, .. }) => {
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
            Action::Run { script, args } => {
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
        assert_eq!(
            normalized,
            Action::Install {
                frozen_lockfile: false,
                ignore_scripts: false,
            }
        );
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
            Action::Run { script, args } => {
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
            Action::Run { script, args } => {
                assert_eq!(script, "tsc");
                assert_eq!(args, vec!["--version", "--help"]);
            }
            _ => panic!("Expected normalized to Run"),
        }
    }

    #[test]
    fn test_start_command_normalizes_to_run() {
        let cli = Cli::parse_from(["craft", "start"]);
        assert_eq!(
            cli.normalize(),
            Action::Run {
                script: "start".to_string(),
                args: vec![],
            }
        );
    }

    #[test]
    fn test_install_frozen_lockfile_and_ci() {
        let frozen = Cli::parse_from(["craft", "install", "--frozen-lockfile"]);
        assert_eq!(
            frozen.normalize(),
            Action::Install {
                frozen_lockfile: true,
                ignore_scripts: false,
            }
        );
        assert_eq!(
            Cli::parse_from(["craft", "ci", "--ignore-scripts"]).normalize(),
            Action::Ci {
                ignore_scripts: true
            }
        );
    }

    #[test]
    fn test_default_command() {
        let cli = Cli::parse_from(["craft"]);
        assert_eq!(cli.command, None);

        // Verify normalization
        let normalized = cli.normalize();
        assert_eq!(
            normalized,
            Action::Install {
                frozen_lockfile: false,
                ignore_scripts: false,
            }
        );
    }
}
