use std::io::IsTerminal;
use std::process::ExitCode;

use cli::Cli;
use contract::{Context, NetworkMode, Result, ScriptError};
use manager::CraftManager;
use progress::{OutputMode, Reporter};

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => report(&error),
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse_args();
    let action = cli.normalize();

    if cli.verbose {
        debug::init();
    }

    debug::trace!("CLI: {:?}", &cli);

    let mut context = Context::from_env(cli.verbose)?;
    context.reporter = Reporter::new(output_mode(&cli, context.ci));
    if cli.offline {
        context.registry.network_mode = NetworkMode::Offline;
    } else if cli.prefer_offline {
        context.registry.network_mode = NetworkMode::PreferOffline;
    }
    let manager = CraftManager::new(context);

    manager.handle_command(action).await
}

/// A live status line only on an interactive terminal; verbose logs and CI get plain output.
fn output_mode(cli: &Cli, ci: bool) -> OutputMode {
    if cli.silent {
        return OutputMode::Silent;
    }
    let is_terminal = std::io::stderr().is_terminal();
    let is_interactive = is_terminal && !ci && !cli.verbose;
    if is_interactive {
        OutputMode::Interactive
    } else {
        OutputMode::Plain
    }
}

/// A failed script already printed its own output, so craft adds one line and exits with the
/// script's code; any other error prints its full cause chain.
fn report(error: &anyhow::Error) -> ExitCode {
    if let Some(ScriptError::Failed { code, .. }) = error.downcast_ref::<ScriptError>() {
        eprintln!("error: {error}");
        return ExitCode::from(u8::try_from(*code).unwrap_or(1));
    }

    eprintln!("Error: {error:?}");
    ExitCode::FAILURE
}
