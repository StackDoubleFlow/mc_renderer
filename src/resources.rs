use std::path::Path;
use std::fs::File;

use color_eyre::Result;
use zip::ZipArchive;

pub struct Resources {

}

impl Resources {
    pub fn init(path: &Path) -> Result<Self> {
        let zip_file = File::open(path)?;
        let zip = ZipArchive::new(zip_file)?;

        for file in zip.file_names() {
            tracing::info!("{}", file);
        }

        Ok(Self {})
    }
}

