use cli::Cli;
use contract::Result;
use manager::CraftManager;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse_args();
    let command = cli.normalize();

    let manager = CraftManager::new();

    manager.handle_command(command).await
}
