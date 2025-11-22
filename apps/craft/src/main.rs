use cli::Cli;
use contract::Result;
use manager::CraftManager;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse_args();
    let command = cli.normalize();

    let manager = CraftManager::new();

    // Initialize debug logging if verbose mode is enabled
    if cli.verbose {
        debug::init();
        manager.set_verbose(cli.verbose);
    }

    debug::trace!("CLI: {:?}", &cli);

    manager.handle_command(command).await
}
