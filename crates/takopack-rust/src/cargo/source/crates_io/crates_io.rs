use std::cell::OnceCell;

use crate::cargo::source::{
    CargoSource, SourceArchive,
    crates_io::{builder::CrateInfo, error},
};

#[derive(Debug)]
pub struct FromCratesIo {
    info: CrateInfo,

    archive: OnceCell<SourceArchive>,
}

impl FromCratesIo {
    pub fn new(info: CrateInfo) -> Self {
        Self {
            info,
            archive: OnceCell::new(),
        }
    }
}

impl CargoSource for FromCratesIo {
    type Error = error::Error;

    fn load_archive(
        &self,
        dir: &impl AsRef<std::path::Path>,
    ) -> Result<&SourceArchive, Self::Error> {
        // Here we can use get_or_try_init, which is a nightly feature now.
        if let Some(archive) = self.archive.get() {
            Ok(archive)
        } else {
            // {name}-{version}.crate
            let filename = self.id().to_owned() + ".crate";
            // <path>/{name}-{version}.crate
            let path = dir.as_ref().join(&filename);
            let url = format!(
                "https://static.crates.io/crates/{}/{}",
                self.name(),
                filename
            );

            takopack_core::network::fetch_binary(&path, &url)?;

            let archive =
                SourceArchive::try_new_hashchecked(&self.id(), &path, &url, &self.info.cksum)?;
            self.archive.set(archive).unwrap();
            self.load_archive(dir)
        }
    }

    fn name(&self) -> &str {
        &self.info.name
    }

    fn version(&self) -> &str {
        &self.info.vers
    }
}
