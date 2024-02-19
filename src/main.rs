mod cli;
mod resources;

use color_eyre::Result;
use tracing::info;
use bevy::prelude::*;

#[derive(Component)]
struct McCamera;

fn setup(mut commands: Commands) {
    commands.insert_resource(ClearColor(Color::BLUE));
    commands.spawn((
        Camera3dBundle::default(),
        McCamera,
    ));
}

struct InputWorld {
    palette: Vec<String>,
    blocks: Vec<u16>,
    dim_x: usize,
    dim_y: usize,
    dim_z: usize,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt::init();

    let cli = cli::parse();
    let asset_pack = resources::load_asset_pack(&cli.client_jar)?;

    App::new().add_plugins(DefaultPlugins).add_systems(Startup, setup).run();

    Ok(())
}
