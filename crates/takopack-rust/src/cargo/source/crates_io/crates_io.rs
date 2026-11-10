use std::path::Path;

use crate::cargo::source::{
    ArchiveMetadata, CargoSource, TarGzArchive,
    crates_io::{builder::CrateInfo, error},
};

#[derive(Debug)]
pub struct FromCratesIo {
    info: CrateInfo,
}

impl FromCratesIo {
    pub fn new(info: CrateInfo) -> Self {
        Self { info }
    }
}

impl CargoSource for FromCratesIo {
    type Error = error::Error;
    type Archive = TarGzArchive;

    fn fetch(&self, destination: &Path) -> Result<Self::Archive, Self::Error> {
        let filename = format!("{}-{}.crate", self.info.name, self.info.vers);
        let url = format!(
            "https://static.crates.io/crates/{}/{}",
            self.info.name, filename
        );

        takopack_core::network::fetch_binary(destination, &url)?;

        let metadata =
            ArchiveMetadata::try_new_hashchecked(destination, Some(url), &self.info.cksum)?;
        Ok(TarGzArchive::new(metadata))
    }
}
