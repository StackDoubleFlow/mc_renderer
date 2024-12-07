use bevy::asset::LoadedFolder;
use bevy::prelude::*;
use bevy::render::texture::ImageSampler;
use bevy::utils::{HashMap, HashSet};
use minecraft_assets::schemas::models::{Texture, Textures};

use crate::resources::mc_meta::McMetaAsset;
use crate::AppLoadState;

#[derive(Resource, Default)]
pub struct McTexturesFolder(Handle<LoadedFolder>);

/// System to start loading of textures
pub fn load_textures(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(McTexturesFolder(
        asset_server.load_folder("minecraft/textures/block"),
    ));
}

pub fn check_textures(
    mut commands: Commands,
    mut events: EventReader<AssetEvent<LoadedFolder>>,
    mut next_state: ResMut<NextState<AppLoadState>>,
    folder_res: Res<McTexturesFolder>,
    loaded_folders: Res<Assets<LoadedFolder>>,
    mut textures: ResMut<Assets<Image>>,
    mut mc_metas: ResMut<Assets<McMetaAsset>>,
) {
    let folder_handle = &folder_res.0;
    // Advance the `AppState` once all sprite handles have been loaded by the `AssetServer`
    for event in events.read() {
        if event.is_loaded_with_dependencies(folder_handle) {
            let folder = loaded_folders.get(folder_handle).unwrap();
            let atlas = create_texture_atlas(folder, &mut textures, &mut mc_metas);
            commands.insert_resource(atlas);
            next_state.set(AppLoadState::Finished);
        }
    }
}

#[derive(Resource)]
pub struct TextureAtlas {
    pub image: Handle<Image>,
    layout: TextureAtlasLayout,
    has_transparency: Vec<bool>,
    mapping: HashMap<String, AssetId<Image>>,
}

pub struct TextureDetails {
    pub rect: Rect,
    pub has_transparency: bool,
}

impl TextureAtlas {
    pub fn get_tex_details(&self, texture_name: &str) -> TextureDetails {
        let texture_name = texture_name
            .trim_start_matches("minecraft:")
            .trim_start_matches("block/");
        let texture_path = format!("minecraft/textures/block/{}.png", texture_name);
        let image_id = self.mapping[&texture_path];
        let idx_in_atlas = self.layout.get_texture_index(image_id).unwrap();
        let mut atlas_rect = self.layout.textures[idx_in_atlas];

        // Convert texture pixel coordinates to normalized
        atlas_rect.min /= self.layout.size;
        atlas_rect.max /= self.layout.size;

        TextureDetails {
            rect: atlas_rect,
            has_transparency: self.has_transparency[idx_in_atlas],
        }
    }
}

impl TextureDetails {
    pub fn get_atlas_uvs(&self, uv_x: f32, uv_y: f32) -> [f32; 2] {
        [
            self.rect.min.x.lerp(self.rect.max.x, uv_x),
            self.rect.min.y.lerp(self.rect.max.y, uv_y),
        ]
    }
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
                if pixel.0[3] != u8::MAX && pixel.0[3] != 0 {
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

/// Resolve texture substitutions
pub fn resolve_textures_completely(textures: Textures) -> Textures {
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
