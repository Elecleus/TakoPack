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
            manifest_path,
        }
    }

    pub fn get_metadata(&self, features: CargoOpt) -> Result<Metadata, Error> {
        let mut command = self.command();
        command.features(features);
        Ok(command.exec()?)
    }

    /// Run `cargo metadata --no-deps`.
    ///
    /// This is offline, does not resolve dependencies and does not create
    /// `Cargo.lock` or `target/` artifacts. `Package` still carries the full
    /// dependency declarations, targets and feature table.
    pub fn get_metadata_no_deps(&self) -> Result<Metadata, Error> {
        let mut command = self.command();
        command.no_deps();
        Ok(command.exec()?)
    }

    fn command(&self) -> MetadataCommand {
        let mut command = MetadataCommand::new();
        command
            .manifest_path(&self.manifest_path)
            .current_dir(&self.dir_path);
        command
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
