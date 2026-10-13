use std::{
    cell::OnceCell,
    path::{Path, PathBuf},
};

use cargo_metadata::{CargoOpt, Metadata};

use crate::{
    cargo::{cargo_dir::CargoDir, source::CargoSource},
    pkgdata::PkgdataBuilder,
};

/// A process for a source.
pub struct Process<S>
where
    S: CargoSource,
{
    source: Box<S>,
    /// {name}-{version}
    id: OnceCell<String>,

    cargo_dir: OnceCell<CargoDir>,

    metadata: OnceCell<Metadata>,
    pkgdata: PkgdataBuilder,

    base_dir: PathBuf,
}

impl<S> Process<S>
where
    S: CargoSource,
{
    pub fn from(source: Box<S>, base_dir: impl AsRef<Path>) -> Self {
        Self {
            source,
            id: OnceCell::new(),
            cargo_dir: OnceCell::new(),
            metadata: OnceCell::new(),
            pkgdata: PkgdataBuilder::new(),
            base_dir: base_dir.as_ref().into(),
        }
    }

    pub fn load(&mut self) -> Result<(), Error> {
        // Set self.id
        self.id
            .set({
                let (name, version) = self.source.name_version();
                format!("{}-{}", name, version)
            })
            .map_err(|_| Error::DoubleInit)?;

        let cargo_dir = CargoDir::new(self.source.cargo_dir());
        self.cargo_dir
            .set(cargo_dir)
            .map_err(|_| Error::DoubleInit)?;

        self.pkgdata.checksum(self.source.checksum()?);

        let metadata = self
            .cargo_dir
            .get()
            .unwrap()
            .get_metadata(CargoOpt::AllFeatures)?;
        self.metadata.set(metadata).map_err(|_| Error::DoubleInit)?;

        Ok(())
    }
}

#[derive(Debug)]
pub enum Error {
    DoubleInit,
    GetBeforeInit,
    CargoDir(crate::cargo::cargo_dir::Error),
    Io(std::io::Error),
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<crate::cargo::cargo_dir::Error> for Error {
    fn from(value: crate::cargo::cargo_dir::Error) -> Self {
        Error::CargoDir(value)
    }
}
