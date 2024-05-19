use bevy::asset::io::Reader;
use bevy::asset::{Asset, AssetLoader, AssetPath, AsyncReadExt, Handle, LoadContext};
use bevy::reflect::TypePath;
use bevy::render::texture::Image;
use serde::Deserialize;
use thiserror::Error;
use bevy::utils::BoxedFuture;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum TextureAnimationFrameInfo {
    Index(u32),
    WithDelay {
        #[serde(default)]
        index: u32,
        #[serde(default)]
        time: u32,
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct TextureAnimationInfo {
    #[serde(default)]
    pub interpolate: bool,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub frametime: u32,
    #[serde(default)]
    pub frames: Vec<TextureAnimationFrameInfo>,
}

#[derive(Debug, Deserialize)]
pub struct McMetaAssetContents {
    #[serde(default)]
    pub animation: TextureAnimationInfo,
}


#[derive(Asset, TypePath, Debug)]
pub struct McMetaAsset {
    pub contents: McMetaAssetContents,
    pub texture: Handle<Image>,
}

#[derive(Default)]
pub struct McMetaAssetLoader;

/// Possible errors that can be produced by [`McMetaAssetLoader`]
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum McMetaAssetLoaderError {
    /// An [IO](std::io) Error
    #[error("Could not load asset: {0}")]
    Io(#[from] std::io::Error),
    /// A [serde_json] Error
    #[error("Could not parse JSON: {0}")]
    JsonError(#[from] serde_json::Error),
}

impl AssetLoader for McMetaAssetLoader {
    type Asset = McMetaAsset;
    type Settings = ();
    type Error = McMetaAssetLoaderError;
    fn load<'a>(
        &'a self,
        reader: &'a mut Reader,
        _settings: &'a (),
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            let contents = serde_json::from_slice::<McMetaAssetContents>(&bytes)?;
            // Get path of the base file it refers to by cutting off .mcmeta
            let texture_path = load_context.path().with_extension("");
            Ok(McMetaAsset {
                contents,
                texture: load_context.load(AssetPath::from_path(&texture_path)),
            })
        })
    }

    fn extensions(&self) -> &[&str] {
        &["mcmeta"]
    }
}
