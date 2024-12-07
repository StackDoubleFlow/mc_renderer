pub mod asset_pack;
pub mod mc_meta;
pub mod textures;

use bevy::prelude::*;

use crate::AppLoadState;

pub struct McAssetLoaderPlugin;

impl Plugin for McAssetLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppLoadState::LoadingTextures), textures::load_textures)
            .add_systems(
                Update,
                textures::check_textures.run_if(in_state(AppLoadState::LoadingTextures)),
            );
    }
}