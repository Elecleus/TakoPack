use std::path::PathBuf;

use crate::cargo::cargo_dir::StructuredCargoDir;

/// A structured Cargo project that sources locally.
pub struct LocalCargoDir {
    path: PathBuf,
}

impl LocalCargoDir {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { path: dir.into() }
    }
}

impl From<LocalCargoDir> for StructuredCargoDir {
    fn from(value: LocalCargoDir) -> Self {
        return Self::from_local_dir(value.path);
    }
}
