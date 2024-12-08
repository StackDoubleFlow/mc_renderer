mod block;
mod cli;
mod debug_menu;
mod resources;

use bevy::core_pipeline::experimental::taa::TemporalAntiAliasBundle;
use bevy::diagnostic::{
    EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin, SystemInformationDiagnosticsPlugin,
};
use bevy::pbr::wireframe::{WireframeConfig, WireframePlugin};
use bevy::pbr::ScreenSpaceAmbientOcclusionBundle;
use bevy::prelude::*;
use bevy::render::settings::{RenderCreation, WgpuFeatures, WgpuSettings};
use bevy::render::RenderPlugin;
use bevy::utils::HashMap;
use bevy::window::{CursorGrabMode, PresentMode, PrimaryWindow};
use bevy_atmosphere::prelude::*;
use bevy_fly_camera::{FlyCamera, FlyCameraPlugin};
use block::{BlockBundle, BlockPalette, BlockPlugin};
use color_eyre::Result;
use debug_menu::McDebugMenuPlugin;
use iyes_perf_ui::prelude::*;
use mc_schems::{Blocks, Schematic};
use resources::mc_meta::{McMetaAsset, McMetaAssetLoader};
use resources::McAssetLoaderPlugin;
use std::f32::consts::PI;
use std::fs;

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum AppLoadState {
    #[default]
    LoadingTextures,
    Finished,
}

fn setup_lights(mut commands: Commands, mut ambient_light: ResMut<AmbientLight>) {
    ambient_light.brightness = 1000.0;
    commands.spawn(DirectionalLightBundle {
        transform: Transform::from_xyz(0.0, 20.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
        directional_light: DirectionalLight {
            illuminance: 1000.0,
            ..default()
        },
        ..default()
    });
    commands.spawn(DirectionalLightBundle {
        transform: Transform::from_xyz(0.0, 20.0, -20.0).looking_at(Vec3::ZERO, Vec3::Y),
        directional_light: DirectionalLight {
            illuminance: 1000.0,
            ..default()
        },
        ..default()
    });
}

fn setup(
    mut commands: Commands,
    block_world: Res<BlockWorld>,
    mut block_palette: ResMut<BlockPalette>,
) {
    let world_parent = commands
        .spawn(SpatialBundle {
            transform: Transform::from_rotation(Quat::from_rotation_y(PI)),
            ..default()
        })
        .id();

    let (sx, sy, sz) = block_world.blocks.size();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let block = block_world.blocks.get_block_at(x, y, z);
                if block == "minecraft:air" {
                    continue;
                }

                let idx = block_palette.get_or_add(block);
                commands
                    .spawn(BlockBundle::new(
                        idx,
                        IVec3::new(x as i32, y as i32, z as i32),
                    ))
                    .set_parent(world_parent);
            }
        }
    }
}

#[derive(Component)]
struct McCamera;

fn setup_camera(mut commands: Commands) {
    commands
        .spawn(Camera3dBundle::default())
        .insert(ScreenSpaceAmbientOcclusionBundle::default())
        .insert(TemporalAntiAliasBundle::default())
        .insert(McCamera)
        .insert(AtmosphereCamera::default())
        .insert(FlyCamera {
            enabled: false,
            ..default()
        });
    commands.insert_resource(AtmosphereModel::new(Nishita {
        sun_position: Vec3::new(1.0, 1.0, 0.0),
        ..default()
    }));
    commands.spawn(PerfUiCompleteBundle::default());
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

#[derive(Resource)]
struct BlockWorld {
    blocks: Blocks,
    entities: HashMap<(u32, u32, u32), Entity>,
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = cli::parse();
    let schematic = Schematic::deserialize(&fs::read(cli.schem_file)?)?;

    let asset_pack = resources::asset_pack::load_asset_pack()?;
    let models = block::get_block_models_for(&asset_pack, &schematic)?;

    App::new()
        .add_plugins((
            DefaultPlugins
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(WgpuSettings {
                        // WARN this is a native only feature. It will not work with webgl or webgpu
                        features: WgpuFeatures::POLYGON_MODE_LINE,
                        ..default()
                    }),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "MC Renderer".into(),
                        present_mode: if cli.no_vsync {
                            PresentMode::AutoNoVsync
                        } else {
                            PresentMode::AutoVsync
                        },
                        ..default()
                    }),
                    ..default()
                }),
            // You need to add this plugin to enable wireframe rendering
            WireframePlugin,
        ))
        // Wireframes can be configured with this resource. This can be changed at runtime.
        .insert_resource(WireframeConfig {
            // The global wireframe config enables drawing of wireframes on every mesh,
            // except those with `NoWireframe`. Meshes with `Wireframe` will always have a
            // wireframe, regardless of the global configuration.
            global: cli.wireframe,
            // Controls the default color of all wireframes. Used as the default color for global
            // wireframes. Can be changed per mesh using the `WireframeColor` component.
            default_color: Color::WHITE,
        })
        .insert_resource(Msaa::Off)
        .add_plugins((McDebugMenuPlugin, FlyCameraPlugin))
        // Perf UI
        .add_plugins((
            PerfUiPlugin,
            FrameTimeDiagnosticsPlugin,
            EntityCountDiagnosticsPlugin,
            SystemInformationDiagnosticsPlugin,
            AtmospherePlugin,
            McAssetLoaderPlugin,
            BlockPlugin,
        ))
        .init_state::<AppLoadState>()
        .init_asset::<McMetaAsset>()
        .init_asset_loader::<McMetaAssetLoader>()
        .insert_resource(BlockWorld {
            blocks: schematic.blocks,
            entities: HashMap::new(),
        })
        .insert_resource(models)
        .add_systems(OnEnter(AppLoadState::Finished), setup)
        .add_systems(Startup, (setup_camera, setup_lights))
        .add_systems(Update, mouse_grab)
        .run();

    Ok(())
}
