use std::path::PathBuf;

use super::MaterializedManifest;

pub struct SingleManifestFile(PathBuf);

impl SingleManifestFile {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    pub fn materialize(&self) -> MaterializedManifest {
        todo!()
    }
}
