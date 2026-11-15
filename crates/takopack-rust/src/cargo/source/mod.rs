pub mod archive;
pub mod crates_io;
pub mod local;
pub mod remote;

pub use archive::{Archive, ArchiveMetadata, ExtractedSource, TarGzArchive};
pub use local::FromLocalArchive;

use std::{io, path::Path};

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    ChecksumMismatch { got: String, expected: String },
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "i/o error: {error}"),
            Self::ChecksumMismatch { got, expected } => {
                write!(
                    formatter,
                    "checksum mismatch, expected {expected}, got {got}"
                )
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::ChecksumMismatch { .. } => None,
        }
    }
}

pub trait CargoSource: std::fmt::Debug {
    type Error: std::error::Error + Send + Sync + 'static;
    type Archive: Archive;

    /// Fetch the source archive into `destination`.
    fn fetch(&self, destination: &Path) -> Result<Self::Archive, Self::Error>;
}
