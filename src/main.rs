mod cli;
mod debug_menu;
mod asset_pack;
mod mc_meta;

use bevy::asset::LoadedFolder;
use bevy::prelude::*;
use bevy::render::texture::ImageSampler;
use bevy::utils::hashbrown::HashSet;
use bevy::utils::HashMap;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use bevy_fly_camera::{FlyCamera, FlyCameraPlugin};
use color_eyre::Result;
use debug_menu::McDebugMenuPlugin;
use mc_meta::{McMetaAsset, McMetaAssetLoader};
use minecraft_assets::api::AssetPack;
use minecraft_assets::schemas::blockstates::multipart::StateValue;
use minecraft_assets::schemas::blockstates::ModelProperties;
use minecraft_assets::schemas::models::{Element, Textures};
use minecraft_assets::schemas::BlockStates;
use std::fs;
use std::sync::Mutex;
use mc_schems::{Blocks, Schematic};

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum AppLoadState {
    #[default]
    LoadingTextures,
    Finished,
}

#[derive(Resource, Default)]
struct McTexturesFolder(Handle<LoadedFolder>);

fn load_textures(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(McTexturesFolder(
        asset_server.load_folder("minecraft/textures/block"),
    ));
}

fn check_textures(
    mut next_state: ResMut<NextState<AppLoadState>>,
    mc_textures_folder: Res<McTexturesFolder>,
    mut events: EventReader<AssetEvent<LoadedFolder>>,
) {
    // Advance the `AppState` once all sprite handles have been loaded by the `AssetServer`
    for event in events.read() {
        if event.is_loaded_with_dependencies(&mc_textures_folder.0) {
            next_state.set(AppLoadState::Finished);
        }
    }
}

fn create_texture_atlas(
    folder: &LoadedFolder,
    textures: &mut ResMut<Assets<Image>>,
    mc_metas: &mut ResMut<Assets<McMetaAsset>>,
) -> (TextureAtlasLayout, Handle<Image>) {
    let mut texture_atlas_builder =
        TextureAtlasBuilder::default();

    let mut animated_textures = HashSet::new();
    for handle in folder.handles.iter() {
        let Ok(meta_id) = handle.id().try_typed::<McMetaAsset>() else {
            continue;
        };
        let meta_asset = mc_metas.get(meta_id).unwrap();
        animated_textures.insert(meta_asset.texture.id());
        // TODO: Actually insert animated texturess
    }

    // Build a texture atlas using the individual sprites
    for handle in folder.handles.iter() {
        let id = handle.id().typed_unchecked::<Image>();
        if animated_textures.contains(&id) {
            // We already handled animated textures above
            continue;
        }

        let Some(texture) = textures.get(id) else {
            // It may be an mcmeta file, likewise handled above
            continue;
        };

        texture_atlas_builder.add_texture(Some(id), texture);
    }

    let (texture_atlas_layout, texture) = texture_atlas_builder.finish().unwrap();
    let texture = textures.add(texture);

    // Update the sampling settings of the texture atlas
    let image = textures.get_mut(&texture).unwrap();
    image.sampler = ImageSampler::nearest();

    (texture_atlas_layout, texture)
}

fn setup(
    mut commands: Commands,
    mc_textures_handle: Res<McTexturesFolder>,
    asset_server: Res<AssetServer>,
    block_models: Res<BlockModels>,
    block_world: Res<BlockWorld>,
    mut texture_atlases: ResMut<Assets<TextureAtlasLayout>>,
    loaded_folders: Res<Assets<LoadedFolder>>,
    mut textures: ResMut<Assets<Image>>,
    mut mc_metas: ResMut<Assets<McMetaAsset>>,
    // for testing
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let loaded_folder = loaded_folders.get(&mc_textures_handle.0).unwrap();
    let (texture_atlas_linear, linear_texture) = create_texture_atlas(
        loaded_folder,
        &mut textures,
        &mut mc_metas,
    );
    let atlas_linear_handle = texture_atlases.add(texture_atlas_linear.clone());

    let mesh = meshes.add(Cuboid::new(10.0, 10.0, 10.0).mesh());
    commands.spawn(
        PbrBundle {
            mesh,
            material: materials.add(StandardMaterial {
                base_color_texture: Some(linear_texture),
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
            transform: Transform::from_xyz(0.0, -0.0, -50.0),
            ..default()
        }
    );

    commands.spawn(PointLightBundle {
        transform: Transform::from_xyz(0.0, 0.0, -25.0),
        point_light: PointLight {
            intensity: 200.0,
            ..default()
        },
        ..default()
    });
}

#[derive(Component)]
struct McCamera;

fn setup_camera(mut commands: Commands) {
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

#[derive(Resource)]
struct BlockWorld {
    blocks: Blocks,
    entities: HashMap<(u32, u32, u32), Entity>,
}

fn decode_props(props: &str) -> HashMap<&str, StateValue> {
    let mut res = HashMap::new();
    if props.is_empty() {
        return res;
    }

    for prop in props.split(',') {
        let (k, v) = prop.split_once('=').unwrap();
        let v = StateValue::String(v.to_string());
        res.insert(k, v);
    }
    res
}

fn get_block_model(asset_pack: &AssetPack, block: &str) -> Result<ModelProperties> {
    let (name, props) = match block.split_once('[') {
        Some((name, props)) => (name, props.trim_end_matches(']')),
        None => (block, ""),
    };
    let blockstates = asset_pack.load_blockstates(name)?;
    let props = decode_props(props);
    let cases = blockstates.into_multipart();
    let mut variant = None;
    for case in cases {
        let applies = case.applies(props.iter().map(|(k, v)| (*k, v)));
        if applies {
            variant = Some(case.apply);
            break;
        }
    };
    let model = variant.expect("Could not match multipart model").models()[0].clone();

    Ok(model)
}

#[derive(Debug)]
struct ProcessedModel {
    textures: Textures,
    elements: Vec<Element>,
    x_rot: i32,
    y_rot: i32,
}

fn process_model(asset_pack: &AssetPack, model: ModelProperties) -> Result<ProcessedModel> {
    let models = asset_pack.load_block_model_recursive(&model.model)?;
    let mut textures = Textures::default();
    let mut elements = Vec::new();
    for model in models.into_iter().rev() {
        if let Some(mut model_textures) = model.textures {
            model_textures.merge(textures);
            textures = model_textures;
        }
        if let Some(mut model_elements) = model.elements {
            elements.append(&mut model_elements);
        }
    }
    textures.resolve(&textures.clone());
    Ok(ProcessedModel {
        textures,
        elements,
        x_rot: model.x,
        y_rot: model.y,
    })
}

#[derive(Resource)]
struct BlockModels(HashMap<String, ProcessedModel>);

fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = cli::parse();
    let schematic = Schematic::deserialize(&fs::read(cli.schem_file)?)?;
    let asset_pack = asset_pack::load_asset_pack(&cli.client_jar)?;
    let mut models = HashMap::new();
    let (sx, sy, sz) = schematic.blocks.size();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let block = schematic.blocks.get_block_at(x, y, z);
                if !models.contains_key(block) {
                    let model_properties = get_block_model(&asset_pack, block)?;
                    let model = process_model(&asset_pack, model_properties)?;
                    models.insert(block.to_string(), model);
                }
            }
        }
    }

    App::new()
        .add_plugins((DefaultPlugins, McDebugMenuPlugin, FlyCameraPlugin))
        .init_state::<AppLoadState>()
        .init_asset::<McMetaAsset>()
        .init_asset_loader::<McMetaAssetLoader>()
        .insert_resource(BlockWorld { blocks: schematic.blocks, entities: HashMap::new() })
        .insert_resource(BlockModels(models))
        .add_systems(OnEnter(AppLoadState::LoadingTextures), load_textures)
        .add_systems(
            Update,
            check_textures.run_if(in_state(AppLoadState::LoadingTextures)),
        )
        .add_systems(OnEnter(AppLoadState::Finished), setup)
        .add_systems(Startup, setup_camera)
        .add_systems(Update, mouse_grab)
        .run();

    Ok(())
}
