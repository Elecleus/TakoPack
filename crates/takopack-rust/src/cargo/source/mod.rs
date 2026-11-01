pub mod crates_io;

use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

// [TODO] Rewrite SourceArchive into a trait, to allow generic SourceArchive.

#[derive(Debug, Clone)]
pub struct SourceArchive {
    pub id: String,
    pub path: PathBuf,
    /// sha256sum
    pub checksum: String,

    /// URL of the source archive.
    ///
    /// For example, "https://static.crates.io/.../..." for crate file.
    pub url: String,
}

impl SourceArchive {
    pub fn try_new(id: &str, path: &impl AsRef<Path>, url: &str) -> Result<Self, io::Error> {
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();

        std::io::copy(&mut file, &mut hasher)?;
        let checksum = format!("{:x}", hasher.finalize());
        Ok(Self {
            id: id.to_string(),
            path: path.as_ref().to_path_buf(),
            checksum,
            url: url.to_string(),
        })
    }

    pub fn try_new_hashchecked(
        id: &str,
        path: &impl AsRef<Path>,
        url: &str,
        expected_cksum: &str,
    ) -> Result<Self, Error> {
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();

        std::io::copy(&mut file, &mut hasher)?;
        let checksum = format!("{:x}", hasher.finalize());

        if checksum == expected_cksum {
            Ok(Self {
                id: id.to_string(),
                path: path.as_ref().to_path_buf(),
                checksum,
                url: url.to_string(),
            })
        } else {
            Err(Error::ChecksumMismatch {
                got: checksum,
                expected: expected_cksum.to_string(),
            })
        }
    }

    /// Load/Extract the source cargo dir, returning the path of `Cargo.toml`.
    pub fn extract(&self) -> Result<PathBuf, std::io::Error> {
        // [TODO] Learning from cargo needed.
        // [TODO] Rewrite into `extract()
        let archive_path = &self.path;
        let extract_dir = self.path.parent().unwrap();

        takopack_core::tarball::extract_tar_gz(archive_path, extract_dir)?;

        let subdir = extract_dir.join(&self.id);
        let cargo_toml_path = subdir.join("Cargo.toml");
        if cargo_toml_path.exists() {
            Ok(cargo_toml_path)
        } else {
            todo!()
        }
    }
}

pub enum Error {
    Io(std::io::Error),
    ChecksumMismatch { got: String, expected: String },
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

pub trait CargoSource: std::fmt::Debug {
    type Error;

    fn name(&self) -> &str;
    fn version(&self) -> &str;

    /// Load source archive file at `<dir>/`,
    /// then return the path and the checksum of the source archive.
    fn load_archive(&self, dir: &impl AsRef<Path>) -> Result<&SourceArchive, Self::Error>;

    fn id(&self) -> String {
        format!("{}-{}", self.name(), self.version())
    }
}
