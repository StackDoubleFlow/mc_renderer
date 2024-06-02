mod asset_pack;
mod cli;
mod debug_menu;
mod mc_meta;

use bevy::asset::LoadedFolder;
use bevy::diagnostic::{
    EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin, SystemInformationDiagnosticsPlugin,
};
use bevy::pbr::wireframe::{WireframeConfig, WireframePlugin};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::settings::{RenderCreation, WgpuFeatures, WgpuSettings};
use bevy::render::texture::ImageSampler;
use bevy::render::RenderPlugin;
use bevy::utils::hashbrown::HashSet;
use bevy::utils::HashMap;
use bevy::window::{CursorGrabMode, PresentMode, PrimaryWindow};
use bevy_fly_camera::{FlyCamera, FlyCameraPlugin};
use color_eyre::Result;
use debug_menu::McDebugMenuPlugin;
use iyes_perf_ui::{PerfUiCompleteBundle, PerfUiPlugin};
use mc_meta::{McMetaAsset, McMetaAssetLoader};
use mc_schems::{Blocks, Schematic};
use minecraft_assets::api::AssetPack;
use minecraft_assets::schemas::blockstates::multipart::StateValue;
use minecraft_assets::schemas::blockstates::ModelProperties;
use minecraft_assets::schemas::models::{Axis, BlockFace, Element, ElementFace, Texture, Textures};
use std::fs;

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

struct TextureAtlas {
    image: Handle<Image>,
    layout: TextureAtlasLayout,
    has_transparency: Vec<bool>,
    mapping: HashMap<String, AssetId<Image>>,
}

fn create_texture_atlas(
    folder: &LoadedFolder,
    textures: &mut ResMut<Assets<Image>>,
    mc_metas: &mut ResMut<Assets<McMetaAsset>>,
) -> TextureAtlas {
    let mut texture_atlas_builder = TextureAtlasBuilder::default();

    let mut animated_textures = HashSet::new();
    for handle in folder.handles.iter() {
        let Ok(meta_id) = handle.id().try_typed::<McMetaAsset>() else {
            continue;
        };
        let meta_asset = mc_metas.get(meta_id).unwrap();
        animated_textures.insert(meta_asset.texture.id());
        // TODO: Actually insert animated texturess
    }

    let mut mapping = HashMap::new();
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

        let asset_path = handle.path().unwrap().to_string();
        mapping.insert(asset_path, id);
        texture_atlas_builder.add_texture(Some(id), texture);
    }

    let (texture_atlas_layout, texture) = texture_atlas_builder.finish().unwrap();
    let texture = textures.add(texture);

    // Update the sampling settings of the texture atlas
    let image = textures.get_mut(&texture).unwrap();
    image.sampler = ImageSampler::nearest();

    let dynamic = image.clone().try_into_dynamic().unwrap();
    let buf = dynamic.as_rgba8().unwrap();
    let mut has_transparency = Vec::new();
    'image: for idx in 0..texture_atlas_layout.len() {
        let rect = texture_atlas_layout.textures[idx];
        for x in rect.min.x as u32..rect.max.x as u32 {
            for y in rect.min.y as u32..rect.max.y as u32 {
                let pixel = buf.get_pixel(x, y);
                if pixel.0[3] != u8::MAX {
                    has_transparency.push(true);
                    continue 'image;
                }
            }
        }
        has_transparency.push(false);
    }

    TextureAtlas {
        image: texture,
        layout: texture_atlas_layout,
        has_transparency,
        mapping,
    }
}

fn get_tint_for_block(
    block_name: &str,
    block_props: &HashMap<&str, StateValue>,
    tint_idx: usize,
) -> Color {
    if block_name == "minecraft:redstone_wire" && tint_idx == 0 {
        let power: f32 = block_props
            .get("power")
            .map(|prop| match prop {
                StateValue::String(str) => str.parse().unwrap_or_default(),
                _ => 0.0,
            })
            .unwrap_or_default();
        let f = power / 15.0;
        let r = f * 0.6 + if f > 0.0 { 0.4 } else { 0.3 };
        let g = (f * f * 0.7 - 0.5).clamp(0.0, 1.0);
        let b = (f * f * 0.6 - 0.7).clamp(0.0, 1.0);
        Color::rgb(r, g, b)
    } else {
        warn!(
            "Unknown tint with block {} and idx {}",
            block_name, tint_idx
        );
        Color::WHITE
    }
}

fn element_mesh(element: &Element, atlas: &TextureAtlas, textures: &Textures) -> (Mesh, bool) {
    let min = Vec3::from_array(element.from);
    let max = Vec3::from_array(element.to);

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    let mut has_transparency = false;

    let mut common_face =
        |face: &ElementFace, face_positions: [([f32; 3], [f32; 2]); 4], normal: [f32; 3]| {
            // Find texture in atlas layout
            let Some(texture) = face.texture.resolve(textures) else {
                warn!("Could not resolve texture variable: {:?}", face.texture);
                return;
            };
            let texture_name = texture
                .trim_start_matches("minecraft:")
                .trim_start_matches("block/");
            let texture_path = format!("minecraft/textures/block/{}.png", texture_name);
            let image_id = atlas.mapping[&texture_path];
            let idx_in_atlas = atlas.layout.get_texture_index(image_id).unwrap();
            let mut atlas_rect = atlas.layout.textures[idx_in_atlas];
            if atlas.has_transparency[idx_in_atlas] {
                has_transparency = true;
            }

            // Convert texture pixel coordinates to normalized
            atlas_rect.min /= atlas.layout.size;
            atlas_rect.max /= atlas.layout.size;

            let index_base = positions.len() as u32;
            let face_indices = [0, 1, 2, 2, 3, 0];
            for x in face_indices.map(|x| x + index_base) {
                indices.push(x);
            }
            for _ in 0..4 {
                normals.push(normal);
            }

            for (pos, norm_uv) in face_positions {
                let mut uv_x = norm_uv[0];
                let mut uv_y = norm_uv[1];
                if let Some(model_uv) = face.uv {
                    uv_x = (model_uv[0] / 16.0).lerp(model_uv[2] / 16.0, uv_x);
                    uv_y = (model_uv[1] / 16.0).lerp(model_uv[3] / 16.0, uv_y);
                }

                positions.push(pos);
                uvs.push([
                    atlas_rect.min.x.lerp(atlas_rect.max.x, uv_x),
                    atlas_rect.min.y.lerp(atlas_rect.max.y, uv_y),
                ])
            }
        };

    if let Some(face) = element.faces.get(&BlockFace::Up) {
        common_face(
            face,
            [
                ([max.x, max.y, min.z], [1.0, 0.0]),
                ([min.x, max.y, min.z], [0.0, 0.0]),
                ([min.x, max.y, max.z], [0.0, 1.0]),
                ([max.x, max.y, max.z], [1.0, 1.0]),
            ],
            [0.0, 1.0, 0.0],
        );
    }
    if let Some(face) = element.faces.get(&BlockFace::Down) {
        common_face(
            face,
            [
                ([max.x, min.y, max.z], [0.0, 0.0]),
                ([min.x, min.y, max.z], [1.0, 0.0]),
                ([min.x, min.y, min.z], [1.0, 1.0]),
                ([max.x, min.y, min.z], [0.0, 1.0]),
            ],
            [0.0, -1.0, 0.0],
        );
    }
    if let Some(face) = element.faces.get(&BlockFace::East) {
        common_face(
            face,
            [
                ([max.x, min.y, min.z], [1.0, 1.0]),
                ([max.x, max.y, min.z], [1.0, 0.0]),
                ([max.x, max.y, max.z], [0.0, 0.0]),
                ([max.x, min.y, max.z], [0.0, 1.0]),
            ],
            [1.0, 0.0, 0.0],
        );
    }
    if let Some(face) = element.faces.get(&BlockFace::West) {
        common_face(
            face,
            [
                ([min.x, min.y, max.z], [1.0, 1.0]),
                ([min.x, max.y, max.z], [1.0, 0.0]),
                ([min.x, max.y, min.z], [0.0, 0.0]),
                ([min.x, min.y, min.z], [0.0, 1.0]),
            ],
            [-1.0, 0.0, 0.0],
        );
    }
    if let Some(face) = element.faces.get(&BlockFace::South) {
        common_face(
            face,
            [
                ([min.x, min.y, max.z], [0.0, 1.0]),
                ([max.x, min.y, max.z], [1.0, 1.0]),
                ([max.x, max.y, max.z], [1.0, 0.0]),
                ([min.x, max.y, max.z], [0.0, 0.0]),
            ],
            [0.0, 0.0, 1.0],
        );
    }
    if let Some(face) = element.faces.get(&BlockFace::North) {
        common_face(
            face,
            [
                ([min.x, max.y, min.z], [0.0, 0.0]),
                ([max.x, max.y, min.z], [1.0, 0.0]),
                ([max.x, min.y, min.z], [1.0, 1.0]),
                ([min.x, min.y, min.z], [0.0, 1.0]),
            ],
            [0.0, 0.0, -1.0],
        );
    }

    (
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices)),
        has_transparency,
    )
}

fn rot_vert_with_orig(rot: Quat, orig: [f32; 3], vert: [f32; 3]) -> [f32; 3] {
    let orig = Vec3::from_array(orig);
    let v = Vec3::from_array(vert) - orig;
    (Transform::from_rotation(rot).transform_point(v) + orig).to_array()
}

fn create_mesh_for_block(
    block: &str,
    atlas: &TextureAtlas,
    block_models: &BlockModels,
) -> (Mesh, Option<Color>, bool) {
    let models = &block_models.0[block];

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    let mut has_transparency = false;

    for model in &models.0 {
        for element in &model.elements {
            let (mesh, element_has_transparency) = element_mesh(element, atlas, &model.textures);

            if element_has_transparency {
                has_transparency = true;
            }

            let model_rot = Quat::from_euler(
                EulerRot::XYZ,
                (-model.model_rot.0 as f32).to_radians(),
                (-model.model_rot.1 as f32).to_radians(),
                0.0,
            );
            let rot_angle = element.rotation.angle.to_radians();
            let elem_rot = match element.rotation.axis {
                Axis::X => Quat::from_rotation_x(rot_angle),
                Axis::Y => Quat::from_rotation_y(rot_angle),
                Axis::Z => Quat::from_rotation_z(rot_angle),
            };
            let mat = Transform::from_rotation(model_rot * elem_rot).compute_matrix();

            let indices_offset = positions.len() as u32;
            let Indices::U32(mesh_indices) = mesh.indices().unwrap() else {
                unreachable!()
            };
            for &x in mesh_indices {
                indices.push(x + indices_offset);
            }

            // Comment below taken from mesh_normal_local_to_world() in mesh_functions.wgsl
            // regarding transform normals from local to world coordinates:

            // NOTE: The mikktspace method of normal mapping requires that the world normal is
            // re-normalized in the vertex shader to match the way mikktspace bakes vertex tangents
            // and normal maps so that the exact inverse process is applied when shading. Blender,
            // Unity, Unreal Engine, Godot, and more all use the mikktspace method. Do not
            // change this code unless you really know what you are doing.
            // http://www.mikktspace.com/

            let inverse_transpose_model = mat.inverse().transpose();
            let inverse_transpose_model = Mat3 {
                x_axis: inverse_transpose_model.x_axis.xyz(),
                y_axis: inverse_transpose_model.y_axis.xyz(),
                z_axis: inverse_transpose_model.z_axis.xyz(),
            };
            let Some(VertexAttributeValues::Float32x3(vert_normals)) =
                &mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
            else {
                unreachable!()
            };
            for n in vert_normals {
                normals.push(
                    inverse_transpose_model
                        .mul_vec3(Vec3::from(*n))
                        .normalize_or_zero()
                        .into(),
                );
            }

            let Some(VertexAttributeValues::Float32x2(vert_uv)) =
                &mesh.attribute(Mesh::ATTRIBUTE_UV_0)
            else {
                unreachable!()
            };
            for uv in vert_uv {
                uvs.push(*uv);
            }

            let Some(VertexAttributeValues::Float32x3(vert_positions)) =
                &mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                unreachable!()
            };
            for &p in vert_positions {
                let p = rot_vert_with_orig(elem_rot, element.rotation.origin, p);
                let p = rot_vert_with_orig(model_rot, [8.0, 8.0, 8.0], p);
                positions.push(p);
            }
        }
    }

    (
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices)),
        models.1,
        has_transparency,
    )
}

fn setup(
    mut commands: Commands,
    mc_textures_handle: Res<McTexturesFolder>,
    block_models: Res<BlockModels>,
    block_world: Res<BlockWorld>,
    loaded_folders: Res<Assets<LoadedFolder>>,
    mut textures: ResMut<Assets<Image>>,
    mut mc_metas: ResMut<Assets<McMetaAsset>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut ambient_light: ResMut<AmbientLight>,
) {
    ambient_light.color = Color::WHITE;
    let loaded_folder = loaded_folders.get(&mc_textures_handle.0).unwrap();
    let atlas = create_texture_atlas(loaded_folder, &mut textures, &mut mc_metas);

    let mut mesh_map = HashMap::new();
    for block in block_world.blocks.blocks_in_palette() {
        let (mesh, tint, has_transparency) = create_mesh_for_block(block, &atlas, &*block_models);
        let mesh = meshes.add(mesh);
        mesh_map.insert(block.to_string(), (mesh, tint, has_transparency));
    }

    let base_material = StandardMaterial {
        base_color_texture: Some(atlas.image),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    };

    let transparent_material = materials.add(base_material.clone());
    let opaque_material = materials.add(StandardMaterial {
        alpha_mode: AlphaMode::Opaque,
        ..base_material.clone()
    });
    let mut tinted_materials = HashMap::new();

    let (sx, sy, sz) = block_world.blocks.size();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let block = block_world.blocks.get_block_at(x, y, z);
                if block == "minecraft:air" {
                    continue;
                }
                let (mesh, tint, has_transparency) = mesh_map[block].clone();
                let material = if let Some(tint) = tint {
                    tinted_materials
                        .entry(tint.as_rgba_u32())
                        .or_insert_with(|| {
                            materials.add(StandardMaterial {
                                base_color: tint,
                                ..base_material.clone()
                            })
                        })
                        .clone()
                } else {
                    if has_transparency {
                        transparent_material.clone()
                    } else {
                        opaque_material.clone()
                    }
                };
                commands.spawn(PbrBundle {
                    mesh,
                    material,
                    transform: Transform {
                        translation: Vec3::new(x as f32, y as f32, z as f32),
                        rotation: Quat::IDENTITY,
                        scale: Vec3::splat(1.0 / 16.0),
                    },
                    ..default()
                });
            }
        }
    }
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

fn get_block_model(
    asset_pack: &AssetPack,
    block: &str,
) -> Result<(Vec<ModelProperties>, Option<Color>)> {
    let (name, props) = match block.split_once('[') {
        Some((name, props)) => (name, props.trim_end_matches(']')),
        None => (block, ""),
    };
    let blockstates = asset_pack.load_blockstates(name)?;
    let props = decode_props(props);

    // This is hackish but whatever for now
    let tint = if name == "minecraft:redstone_wire" {
        Some(get_tint_for_block(name, &props, 0))
    } else {
        None
    };

    let cases = blockstates.into_multipart();
    let mut models = Vec::new();
    for case in cases {
        let applies = case.applies(props.iter().map(|(k, v)| (*k, v)));
        if applies {
            // TODO: for now I'm just choosing the first instead of randomly selecting
            models.push(case.apply.models()[0].clone());
        }
    }

    Ok((models, tint))
}

#[derive(Debug)]
struct ProcessedModel {
    model_rot: (i32, i32),
    textures: Textures,
    elements: Vec<Element>,
}

fn resolve_textures_completely(textures: Textures) -> Textures {
    let mut resolved_textures: std::collections::HashMap<String, Texture> = default();

    for (name, texture) in textures.iter() {
        let mut texture = texture.0.as_str();
        loop {
            if let Some(target) = texture.strip_prefix('#') {
                texture = textures[target].0.as_str();
            } else {
                break;
            }
        }
        resolved_textures.insert(name.clone(), texture.into());
    }

    resolved_textures.into()
}

fn process_model(
    asset_pack: &AssetPack,
    models: Vec<ModelProperties>,
) -> Result<Vec<ProcessedModel>> {
    let mut processed_models = Vec::new();
    for model_props in models {
        let mut textures = Textures::default();
        let mut elements = Vec::new();
        let models = asset_pack.load_block_model_recursive(&model_props.model)?;
        for model in models.into_iter().rev() {
            if let Some(mut model_textures) = model.textures {
                model_textures.merge(textures);
                textures = model_textures;
            }
            if let Some(mut model_elements) = model.elements {
                elements.append(&mut model_elements);
            }
        }
        textures = resolve_textures_completely(textures);
        processed_models.push(ProcessedModel {
            model_rot: (model_props.x, model_props.y),
            textures,
            elements,
        });
    }
    Ok(processed_models)
}

#[derive(Resource)]
struct BlockModels(HashMap<String, (Vec<ProcessedModel>, Option<Color>)>);

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
                    let (model_properties, tint) = get_block_model(&asset_pack, block)?;
                    let model = process_model(&asset_pack, model_properties)?;
                    models.insert(block.to_string(), (model, tint));
                }
            }
        }
    }

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
        .add_plugins((McDebugMenuPlugin, FlyCameraPlugin))
        // Perf UI
        .add_plugins((
            PerfUiPlugin,
            FrameTimeDiagnosticsPlugin,
            EntityCountDiagnosticsPlugin,
            SystemInformationDiagnosticsPlugin,
        ))
        .init_state::<AppLoadState>()
        .init_asset::<McMetaAsset>()
        .init_asset_loader::<McMetaAssetLoader>()
        .insert_resource(BlockWorld {
            blocks: schematic.blocks,
            entities: HashMap::new(),
        })
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
