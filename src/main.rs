mod cli;
mod debug_menu;
mod resources;

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use bevy_fly_camera::{FlyCamera, FlyCameraPlugin};
use color_eyre::Result;
use debug_menu::McDebugMenuPlugin;

#[derive(Component)]
struct McCamera;

fn setup(mut commands: Commands) {
    commands.insert_resource(ClearColor(Color::BLUE));
    commands
        .spawn(Camera3dBundle::default())
        .insert(McCamera)
        .insert(FlyCamera {
            enabled: false,
            ..default()
        });
}

fn mouse_grab(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut fly_camera: Query<&mut FlyCamera>,
    mut window: Query<&mut Window, With<PrimaryWindow>>,
) {
    let mut window = window.single_mut();
    let mut fly_camera = fly_camera.single_mut();

    let escape_pressed = keyboard_input.just_pressed(KeyCode::Escape);
    let t_pressed = keyboard_input.just_pressed(KeyCode::KeyT);
    if !fly_camera.enabled && escape_pressed {
        fly_camera.enabled = true;
        window.cursor.grab_mode = CursorGrabMode::Locked;
        window.cursor.visible = false;
    } else if fly_camera.enabled && (t_pressed || escape_pressed) {
        fly_camera.enabled = false;
        window.cursor.grab_mode = CursorGrabMode::None;
        window.cursor.visible = true;
    }
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
        .add_plugins((DefaultPlugins, McDebugMenuPlugin, FlyCameraPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, mouse_grab)
        .run();

    Ok(())
}
