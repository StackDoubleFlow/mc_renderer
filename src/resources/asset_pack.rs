use color_eyre::Result;
use minecraft_assets::api::AssetPack;

pub fn load_asset_pack() -> Result<AssetPack> {
    Ok(AssetPack::at_path("."))
}
