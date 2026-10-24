use std::process::ExitCode;

use cli::Cli;
use contract::{Context, Result, ScriptError};
use manager::CraftManager;

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

    let context = Context::from_env(cli.verbose)?;
    let manager = CraftManager::new(context);

    manager.handle_command(action).await
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
