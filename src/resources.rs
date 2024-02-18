use std::cell::RefCell;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use color_eyre::Result;
use minecraft_assets::api::{
    AssetPack, EnumerateResources, LoadResource, ResourceIdentifier, ResourceKind, ResourcePath,
};
use zip::ZipArchive;

struct ZipArchiveResourceProvider(RefCell<ZipArchive<File>>);

impl EnumerateResources for ZipArchiveResourceProvider {
    fn enumerate_resources(
        &self,
        namespace: &str,
        kind: ResourceKind,
    ) -> Result<Vec<ResourceIdentifier<'static>>, io::Error> {
        let mut zip = self.0.borrow_mut();
        let dir = ResourcePath::for_kind("", namespace, kind);

        let mut res = Vec::new();
        for i in 0..zip.len() {
            let file = zip.by_index(i)?;
            if let Some(id) = dir
                .to_str()
                .and_then(|dir| file.name().strip_prefix(dir))
                .and_then(|s| s.strip_suffix(kind.extension()))
            {
                res.push(ResourceIdentifier::new_owned(
                    kind,
                    format!("{namespace}{id}"),
                ))
            }
        }

        Ok(res)
    }
}

impl LoadResource for ZipArchiveResourceProvider {
    fn load_resource(&self, id: &ResourceIdentifier) -> Result<Vec<u8>, io::Error> {
        let path = ResourcePath::for_resource("", id);
        let mut zip = self.0.borrow_mut();
        let mut file = zip
            .by_name(path.to_str().unwrap())
            .map_err(|e| io::Error::new(io::ErrorKind::NotFound, e))?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        Ok(buf)
    }
}

pub struct Resources {}

impl Resources {
    pub fn init(path: &Path) -> Result<Self> {
        let zip_file = File::open(path)?;
        let zip = ZipArchive::new(zip_file)?;
        let resource_provider = ZipArchiveResourceProvider(RefCell::new(zip));
        let assets = AssetPack::new(resource_provider);

        let states = assets.load_blockstates("oak_planks").unwrap();
        dbg!(assets
            .load_block_model_recursive("block/oak_planks")
            .unwrap());
        Ok(Resources {})
    }
}
