use std::path::{Path, PathBuf};

use cargo_metadata::{CargoOpt, Metadata, MetadataCommand};

#[derive(Debug)]
pub struct CargoDir {
    dir_path: PathBuf,
    manifest_path: PathBuf,
}

impl CargoDir {
    pub fn new(dir: impl AsRef<Path>) -> Self {
        let manifest_path = dir.as_ref().join("Cargo.toml");

        Self {
            dir_path: dir.as_ref().into(),
            manifest_path: manifest_path,
        }
    }

    pub fn get_metadata(&self, features: CargoOpt) -> Result<Metadata, Error> {
        let mut cmd = MetadataCommand::new();
        cmd.manifest_path(&self.manifest_path).features(features);

        Ok(cmd.exec()?)
    }
}

#[derive(Debug)]
pub enum Error {
    CargoMetadata(cargo_metadata::Error),
}

impl From<cargo_metadata::Error> for Error {
    fn from(value: cargo_metadata::Error) -> Self {
        Self::CargoMetadata(value)
    }
}
