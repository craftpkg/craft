use cli::Cli;
use manager::CraftManager;

#[tokio::main]
async fn main() {
    let cli = Cli::parse_args();
    let command = cli.normalize();

    let manager = CraftManager::new();
    manager.handle_command(command).await;
}
