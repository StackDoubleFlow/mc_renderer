mod cli;
mod debug_menu;
mod resources;

use bevy::prelude::*;
use color_eyre::Result;
use debug_menu::McDebugMenuPlugin;

#[derive(Component)]
struct McCamera;

fn setup(mut commands: Commands) {
    commands.insert_resource(ClearColor(Color::BLUE));
    commands.spawn((Camera3dBundle::default(), McCamera));
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

    let cli = cli::parse();
    let asset_pack = resources::load_asset_pack(&cli.client_jar)?;

    App::new()
        .add_plugins((DefaultPlugins, McDebugMenuPlugin))
        .add_systems(Startup, setup)
        .run();

    Ok(())
}
