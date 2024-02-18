mod cli;
mod resources;
mod window;

use color_eyre::Result;
use tracing::info;
use tracing_log::LogTracer;

fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt::init();
    LogTracer::init()?;

    let cli = cli::parse();
    resources::Resources::init(&cli.client_jar)?;
    info!("Hello world");

    Ok(())
}
