use std::path::PathBuf;

use tempfile::TempDir;

use crate::cargo::cargo_dir::StructuredCargoDir;

pub struct FromCratesIoExtracted {
    /// {name}-{version}
    id: String,

    extracted_dir: PathBuf,

    _tempdir: TempDir,
}

impl FromCratesIoExtracted {
    pub fn new(id: &str, extracted_dir: impl Into<PathBuf>, tempdir: TempDir) -> Self {
        Self {
            id: id.to_owned(),
            extracted_dir: extracted_dir.into(),
            _tempdir: tempdir,
        }
    }
}

impl From<FromCratesIoExtracted> for StructuredCargoDir {
    fn from(value: FromCratesIoExtracted) -> Self {
        Self::new(value.extracted_dir.join(value.id), Some(value._tempdir))
    }
}
