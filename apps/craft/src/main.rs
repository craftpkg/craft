use cli::Cli;
use contract::{Context, Result};
use manager::CraftManager;

#[tokio::main]
async fn main() -> Result<()> {
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
