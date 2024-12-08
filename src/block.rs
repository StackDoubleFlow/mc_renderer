use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::render::render_asset::RenderAssetUsages;
use color_eyre::Result;
use mc_schems::Schematic;
use minecraft_assets::api::AssetPack;
use minecraft_assets::schemas::blockstates::multipart::StateValue;
use minecraft_assets::schemas::blockstates::ModelProperties;
use minecraft_assets::schemas::models::{Axis, BlockFace, Element, ElementFace, Textures};

use crate::resources::textures::{resolve_textures_completely, TextureAtlas};
use crate::AppLoadState;

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
        Color::srgb(r, g, b)
    } else {
        warn!(
            "Unknown tint with block {} and idx {}",
            block_name, tint_idx
        );
        Color::WHITE
    }
}

fn element_mesh(
    element: &Element,
    atlas: &TextureAtlas,
    textures: &Textures,
    uv_rot: Vec2,
) -> (Mesh, bool) {
    let min = Vec3::from_array(element.from);
    let max = Vec3::from_array(element.to);

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    let mut has_transparency = false;

    let mut common_face = |face: &ElementFace,
                           face_positions: [([f32; 3], [f32; 2]); 4],
                           normal: [f32; 3],
                           use_x_rot: bool| {
        // Find texture in atlas layout
        let Some(texture) = face.texture.resolve(textures) else {
            warn!("Could not resolve texture variable: {:?}", face.texture);
            return;
        };

        let texture = atlas.get_tex_details(texture);
        if texture.has_transparency {
            has_transparency = true;
        }

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

            let rot = if use_x_rot { uv_rot.x } else { uv_rot.y };
            let [uv_x, uv_y] = rotate_uv_with_orig(rot, [0.5, 0.5], [uv_x, uv_y]);

            positions.push(pos);
            uvs.push(texture.get_atlas_uvs(uv_x, uv_y));
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
            false,
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
            false,
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
            true,
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
            true,
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
            true,
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
            true,
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

fn rotate_uv_with_orig(rot: f32, orig: [f32; 2], uv: [f32; 2]) -> [f32; 2] {
    let res = rot_vert_with_orig(
        Quat::from_rotation_y(rot),
        [orig[0], 0.0, orig[1]],
        [uv[0], 0.0, uv[1]],
    );
    [res[0], res[2]]
}

#[derive(Clone)]
pub struct ElementMesh {
    pub mesh: Handle<Mesh>,
    pub has_transparency: bool,
}

pub fn create_mesh_for_block(
    block: &str,
    atlas: &TextureAtlas,
    block_models: &BlockModels,
    mesh_assets: &mut Assets<Mesh>,
) -> (ElementMesh, Option<Color>) {
    let models = &block_models.0[block];

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    let mut has_transparency = false;

    for model in &models.0 {
        for element in &model.elements {
            let model_rot = Vec2::new(
                (-model.model_rot.0 as f32).to_radians(),
                (-model.model_rot.1 as f32).to_radians(),
            );
            let (mesh, elem_has_transparency) = element_mesh(
                element,
                atlas,
                &model.textures,
                if model.uv_lock { model_rot } else { Vec2::ZERO },
            );

            if elem_has_transparency {
                has_transparency = true;
            }

            let model_rot = Quat::from_rotation_y(model_rot.y) * Quat::from_rotation_x(model_rot.x);
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
                let p = (Vec3::from_array(p)).to_array();
                positions.push(p);
            }
        }
    }

    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices));
    let meshes = ElementMesh {
        mesh: mesh_assets.add(mesh),
        has_transparency,
    };
    (meshes, models.1)
}

#[derive(Resource)]
pub struct BlockModels(HashMap<String, (Vec<ProcessedModel>, Option<Color>)>);

pub fn get_block_models_for(asset_pack: &AssetPack, schem: &Schematic) -> Result<BlockModels> {
    let mut models = HashMap::new();
    let (sx, sy, sz) = schem.blocks.size();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let block = schem.blocks.get_block_at(x, y, z);
                if !models.contains_key(block) {
                    let (model_properties, tint) = get_block_model(&asset_pack, block)?;
                    let model = process_model(&asset_pack, model_properties)?;
                    models.insert(block.to_string(), (model, tint));
                }
            }
        }
    }
    Ok(BlockModels(models))
}

#[derive(Debug)]
struct ProcessedModel {
    model_rot: (i32, i32),
    uv_lock: bool,
    textures: Textures,
    elements: Vec<Element>,
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
            uv_lock: model_props.uv_lock,
            textures,
            elements,
        });
    }
    Ok(processed_models)
}

#[derive(Default)]
struct BlockMaterials {
    base: StandardMaterial,
    opaque: Handle<StandardMaterial>,
    transparent: Handle<StandardMaterial>,
    /// mapping from tint color to material
    tints: HashMap<u32, Handle<StandardMaterial>>,
}

impl BlockMaterials {
    fn new(materials: &mut Assets<StandardMaterial>, atlas: &TextureAtlas) -> Self {
        let base = StandardMaterial {
            base_color_texture: Some(atlas.image.clone()),
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            fog_enabled: false,
            alpha_mode: AlphaMode::Blend,
            ..default()
        };

        Self {
            opaque: materials.add(StandardMaterial {
                alpha_mode: AlphaMode::Mask(0.5),
                ..base.clone()
            }),
            transparent: materials.add(base.clone()),
            tints: Default::default(),
            base,
        }
    }

    fn get_or_add_tint(
        &mut self,
        tint: Color,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        let linear_rgba: LinearRgba = tint.into();
        self.tints
            .entry(linear_rgba.as_u32())
            .or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: tint,
                    ..self.base.clone()
                })
            })
            .clone()
    }
}

#[derive(Resource)]
pub struct BlockPalette {
    blocks: Vec<String>,
    map: HashMap<String, usize>,
}

impl Default for BlockPalette {
    fn default() -> Self {
        Self {
            blocks: vec!["minecraft:air".to_string()],
            map: {
                let mut map = HashMap::new();
                map.insert("minecraft:air".to_string(), 0);
                map
            },
        }
    }
}

impl BlockPalette {
    pub fn get_or_add(&mut self, name: &str) -> usize {
        match self.map.get(name) {
            Some(idx) => *idx,
            None => {
                let idx = self.blocks.len();
                self.map.insert(name.to_string(), idx);
                self.blocks.push(name.to_string());
                idx
            }
        }
    }
}

#[derive(Resource, Default)]
struct BlockResources {
    // Mapping from palette index to mesh and tint
    meshes: HashMap<usize, (ElementMesh, Option<Color>)>,
    mats: BlockMaterials,
}

#[derive(Bundle)]
pub struct BlockBundle {
    pub block: Block,
    pub pbr: PbrBundle,
}

impl BlockBundle {
    pub fn new(idx: usize, pos: IVec3) -> Self {
        Self {
            block: Block { block: idx },
            pbr: PbrBundle {
                transform: Transform {
                    translation: pos.as_vec3(),
                    rotation: Quat::IDENTITY,
                    scale: Vec3::splat(1.0 / 16.0),
                },
                ..default()
            },
        }
    }
}

#[derive(Component)]
pub struct Block {
    block: usize,
}

fn init_new_blocks(
    mut blocks: Query<(&Block, &mut Handle<Mesh>, &mut Handle<StandardMaterial>), Added<Block>>,
    mut res: ResMut<BlockResources>,
    atlas: Res<TextureAtlas>,
    block_models: Res<BlockModels>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    palette: Res<BlockPalette>,
) {
    for (block, mut mesh_handle, mut mat_handle) in blocks.iter_mut() {
        let res = &mut *res;
        let (mesh, tint) = match res.meshes.get(&block.block) {
            Some(mesh) => mesh,
            None => {
                let mesh = create_mesh_for_block(
                    &palette.blocks[block.block],
                    &atlas,
                    &block_models,
                    &mut meshes,
                );
                res.meshes.insert(block.block, mesh);
                // it's stupid i know
                res.meshes.get(&block.block).unwrap()
            }
        };
        *mesh_handle = mesh.mesh.clone();
        let material = if let Some(tint) = tint {
            res.mats.get_or_add_tint(*tint, &mut materials)
        } else {
            if mesh.has_transparency {
                res.mats.transparent.clone()
            } else {
                res.mats.opaque.clone()
            }
        };
        *mat_handle = material;
    }
}

fn init_block_resources(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    atlas: Res<TextureAtlas>,
) {
    commands.insert_resource(BlockResources {
        meshes: default(),
        mats: BlockMaterials::new(&mut materials, &atlas),
    });
}

pub struct BlockPlugin;

impl Plugin for BlockPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            init_new_blocks.run_if(in_state(AppLoadState::Finished)),
        )
        .add_systems(OnEnter(AppLoadState::Finished), init_block_resources)
        .init_resource::<BlockPalette>()
        .init_resource::<BlockResources>();
    }
}
