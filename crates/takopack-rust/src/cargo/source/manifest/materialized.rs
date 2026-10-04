use std::path::PathBuf;

use tempfile::TempDir;

use crate::cargo::cargo_dir::StructuredCargoDir;

pub struct MaterializedManifest {
    manifest: PathBuf,

    _tempdir: TempDir,
}

impl From<MaterializedManifest> for StructuredCargoDir {
    fn from(value: MaterializedManifest) -> Self {
        Self::from_tempdir(value._tempdir)
    }
}
