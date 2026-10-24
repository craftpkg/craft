use std::fmt::Display;

use clap::{Args, Parser, Subcommand};
use contract::CRAFT_VERSION;

#[derive(Parser, Debug)]
#[command(name = "craft", version = CRAFT_VERSION)]
#[command(about = "A fast, strict package manager for Node.js", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Print what craft is doing.
    #[arg(short, long, global = true, conflicts_with = "silent")]
    pub verbose: bool,

    /// Print nothing but warnings and errors.
    #[arg(short, long, global = true)]
    pub silent: bool,

    /// Use only cached registry metadata and stored packages; never touch the network.
    #[arg(long, global = true, conflicts_with = "prefer_offline")]
    pub offline: bool,

    /// Use cached registry metadata without revalidating it; fetch only what is missing.
    #[arg(long, global = true)]
    pub prefer_offline: bool,
}

impl Display for Cli {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.command)
    }
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum Commands {
    /// Add packages to package.json and install them.
    Add {
        #[arg(required = true)]
        packages: Vec<String>,
        #[command(flatten)]
        save: SaveFlags,
        /// Run no lifecycle scripts.
        #[arg(long)]
        ignore_scripts: bool,
    },
    /// Remove packages from package.json and node_modules.
    #[command(visible_aliases = ["rm", "uninstall", "un"])]
    Remove {
        #[arg(required = true)]
        packages: Vec<String>,
    },
    /// Run a package.json script, or list them when no name is given.
    Run {
        script: Option<String>,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Manage the global package store and metadata cache.
    Cache {
        #[command(subcommand)]
        command: CacheCommands,
    },
    /// Create a package.json in the current directory.
    Init,
    /// Run the `start` script.
    Start,
    /// Run the `test` script.
    #[command(visible_alias = "t")]
    Test,
    /// Install the project's dependencies; with package names, add them like `craft add`.
    #[command(visible_alias = "i")]
    Install {
        packages: Vec<String>,
        #[command(flatten)]
        save: SaveFlags,
        /// Install exactly what craft.lock records; fail if it is missing or out of date.
        /// On by default when the CI environment variable is set.
        #[arg(long)]
        frozen_lockfile: bool,
        /// Run no lifecycle scripts.
        #[arg(long)]
        ignore_scripts: bool,
        /// Leave devDependencies out.
        #[arg(long, visible_alias = "prod")]
        production: bool,
    },
    /// Run a bin from the project's dependencies.
    Exec {
        command: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Run a package's bin without adding it to the project.
    #[command(visible_alias = "x")]
    Dlx {
        /// The package to install, when its bin is named differently.
        #[arg(short, long)]
        package: Option<String>,
        command: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Show direct dependencies with newer versions available.
    Outdated,
    /// Update dependencies within their specifiers, or to their latest versions with --latest.
    #[command(visible_aliases = ["up", "upgrade"])]
    Update {
        /// The dependencies to update; all of them when none are given.
        packages: Vec<String>,
        /// Move to the latest versions and rewrite the specifiers in package.json.
        #[arg(short = 'L', long)]
        latest: bool,
    },
    /// Allow packages to run install scripts, then install.
    Trust {
        #[arg(required = true)]
        packages: Vec<String>,
    },
    /// List the project's dependencies, and theirs with --depth.
    #[command(visible_alias = "ls")]
    List {
        /// How many levels of dependencies to show below the project's own.
        #[arg(long, default_value_t = 0)]
        depth: usize,
    },
    /// Show every chain of dependencies that installs a package.
    Why { package: String },
    /// Remove node_modules and install exactly what craft.lock records.
    Ci {
        /// Run no lifecycle scripts.
        #[arg(long)]
        ignore_scripts: bool,
    },
    #[command(external_subcommand)]
    External(Vec<String>),
}

/// How added packages are saved to package.json.
#[derive(Args, Debug, PartialEq, Clone, Copy, Default)]
pub struct SaveFlags {
    /// Save to devDependencies.
    #[arg(short = 'D', long)]
    pub dev: bool,
    /// Save the exact resolved version instead of a caret range.
    #[arg(short = 'E', long)]
    pub exact: bool,
    /// Save to optionalDependencies.
    #[arg(short = 'O', long = "save-optional", conflicts_with_all = ["dev", "peer"])]
    pub optional: bool,
    /// Save to peerDependencies, and to devDependencies so it is installed for development.
    #[arg(short = 'P', long = "save-peer", conflicts_with = "dev")]
    pub peer: bool,
}

#[derive(Subcommand, Debug, PartialEq, Clone)]
pub enum CacheCommands {
    /// Remove every stored package and cached registry response.
    Clean {
        /// Confirm the removal.
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
/// `install <pkg>` becomes `Add`, and no command becomes `Install`.
#[derive(Debug, PartialEq)]
pub enum Action {
    Add {
        packages: Vec<String>,
        dev: bool,
        exact: bool,
        optional: bool,
        peer: bool,
        ignore_scripts: bool,
    },
    Remove {
        packages: Vec<String>,
    },
    Run {
        script: String,
        args: Vec<String>,
    },
    ListScripts,
    Cache {
        command: CacheCommands,
    },
    Init,
    List {
        depth: usize,
    },
    Why {
        package: String,
    },
    Install {
        frozen_lockfile: bool,
        ignore_scripts: bool,
        production: bool,
    },
    Ci {
        ignore_scripts: bool,
    },
    Trust {
        packages: Vec<String>,
    },
    Outdated,
    Update {
        packages: Vec<String>,
        latest: bool,
    },
    Exec {
        command: String,
        args: Vec<String>,
    },
    Dlx {
        package: Option<String>,
        command: String,
        args: Vec<String>,
    },
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }

    pub fn normalize(&self) -> Action {
        let Some(command) = &self.command else {
            return default_install();
        };

        match command {
            Commands::Add {
                packages,
                save,
                ignore_scripts,
            } => add(packages, save, *ignore_scripts),
            Commands::Remove { packages } => Action::Remove {
                packages: packages.clone(),
            },
            Commands::Run {
                script: Some(script),
                args,
            } => run_script(script, args),
            Commands::Run { script: None, .. } => Action::ListScripts,
            Commands::Cache { command } => Action::Cache {
                command: command.clone(),
            },
            Commands::Init => Action::Init,
            Commands::List { depth } => Action::List { depth: *depth },
            Commands::Why { package } => Action::Why {
                package: package.clone(),
            },
            Commands::Start => run_script("start", &[]),
            Commands::Test => run_script("test", &[]),
            Commands::Install {
                packages,
                save,
                ignore_scripts,
                ..
            } if !packages.is_empty() => add(packages, save, *ignore_scripts),
            Commands::Install {
                frozen_lockfile,
                ignore_scripts,
                production,
                ..
            } => Action::Install {
                frozen_lockfile: *frozen_lockfile,
                ignore_scripts: *ignore_scripts,
                production: *production,
            },
            Commands::Trust { packages } => Action::Trust {
                packages: packages.clone(),
            },
            Commands::Outdated => Action::Outdated,
            Commands::Exec { command, args } => Action::Exec {
                command: command.clone(),
                args: args.clone(),
            },
            Commands::Dlx {
                package,
                command,
                args,
            } => Action::Dlx {
                package: package.clone(),
                command: command.clone(),
                args: args.clone(),
            },
            Commands::Update { packages, latest } => Action::Update {
                packages: packages.clone(),
                latest: *latest,
            },
            Commands::Ci { ignore_scripts } => Action::Ci {
                ignore_scripts: *ignore_scripts,
            },
            Commands::External(args) => match args.split_first() {
                Some((script, args)) => run_script(script, args),
                None => default_install(),
            },
        }
    }
}

fn add(packages: &[String], save: &SaveFlags, ignore_scripts: bool) -> Action {
    Action::Add {
        packages: packages.to_vec(),
        dev: save.dev,
        exact: save.exact,
        optional: save.optional,
        peer: save.peer,
        ignore_scripts,
    }
}

fn default_install() -> Action {
    Action::Install {
        frozen_lockfile: false,
        ignore_scripts: false,
        production: false,
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
            Some(Commands::Add { packages, save, .. }) => {
                assert_eq!(packages, vec!["react", "express"]);
                assert!(!save.dev);
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
                optional: false,
                peer: false,
                ignore_scripts: false,
            }
        );
    }

    #[test]
    fn test_add_dev_dependency() {
        let cli = Cli::parse_from(["craft", "add", "typescript", "-D"]);
        match cli.command {
            Some(Commands::Add { packages, save, .. }) => {
                assert_eq!(packages, vec!["typescript"]);
                assert!(save.dev);
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
                assert_eq!(script.as_deref(), Some("build"));
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
                production: false,
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
                production: false,
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
                production: false,
            }
        );
    }
}
