use std::path::{Path, PathBuf};

use crate::cargo::source::CargoSource;

pub struct PreparedCrate {
    name: String,
    version: String,
    archive: PathBuf,
    cargo_dir: PathBuf,
}

impl PreparedCrate {
    pub fn new(name: String, version: String, archive: PathBuf, cargo_dir: PathBuf) -> Self {
        Self {
            name,
            version,
            archive,
            cargo_dir,
        }
    }
}

impl<'a> CargoSource for PreparedCrate {
    fn name_version(&self) -> (&str, &str) {
        let name = &self.name;
        let version = &self.version;
        (name, version)
    }

    fn archive(&self) -> &Path {
        &self.archive
    }

    fn cargo_dir(&self) -> &Path {
        &self.cargo_dir
    }
}
