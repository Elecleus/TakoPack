use std::path::PathBuf;

use takopack_core::{error::Result, tarball::extract_tar_gz};
use tempfile::TempDir;

use crate::cargo::source::crates_io::extracted::FromCratesIoExtracted;

pub struct FromCratesIo {
    /// {name}-{version}
    id: String,

    cratefile: PathBuf,

    _tempdir: TempDir,
}

impl FromCratesIo {
    pub fn new(id: &str, cratefile: impl Into<PathBuf>, tempdir: TempDir) -> Self {
        Self {
            id: id.to_owned(),

            cratefile: cratefile.into(),

            _tempdir: tempdir,
        }
    }

    pub fn extract(self) -> Result<FromCratesIoExtracted> {
        let extract_dir = self._tempdir.path().to_owned();

        extract_tar_gz(&self.cratefile, &extract_dir)?;

        Ok(FromCratesIoExtracted::new(
            &self.id,
            extract_dir,
            self._tempdir,
        ))
    }
}
