mod cli;
mod resources;

use color_eyre::Result;
use tracing::info;

fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt::init();

    let cli = cli::parse();
    resources::Resources::init(&cli.client_jar)?;
    info!("Hello world");

    Ok(())
}
