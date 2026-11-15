//! Local gzip-compressed tar archive source.
//!
//! This is the same archive pipeline as the remote source, but the archive is
//! copied from a local path instead of downloaded.

use std::path::{Path, PathBuf};

use crate::cargo::source::{ArchiveMetadata, CargoSource, TarGzArchive};

/// A [`CargoSource`] backed by a local archive file.
#[derive(Debug)]
pub struct FromLocalArchive {
    path: PathBuf,
}

impl FromLocalArchive {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl CargoSource for FromLocalArchive {
    type Error = crate::cargo::source::Error;
    type Archive = TarGzArchive;

    fn fetch(&self, destination: &Path) -> Result<Self::Archive, Self::Error> {
        if self.path != destination {
            std::fs::copy(&self.path, destination)?;
        }

        let metadata = ArchiveMetadata::try_new(destination, None)?;
        Ok(TarGzArchive::new(metadata))
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use flate2::Compression;
    use flate2::write::GzEncoder;
    use tar::{Builder, Header};

    use crate::cargo::source::Archive;

    use super::*;

    #[test]
    fn copies_checks_and_extracts_a_local_archive() {
        let dir = tempfile::tempdir().unwrap();
        let source_path = dir.path().join("demo.tar.gz");
        let file = File::create(&source_path).unwrap();
        let encoder = GzEncoder::new(file, Compression::default());
        let mut builder = Builder::new(encoder);

        for (name, contents) in [
            ("demo-0.1.0/Cargo.toml", "[package]\nname = \"demo\"\n"),
            ("demo-0.1.0/lib.rs", ""),
        ] {
            let mut header = Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, name, contents.as_bytes())
                .unwrap();
        }
        let encoder = builder.into_inner().unwrap();
        encoder.finish().unwrap();

        let destination = dir.path().join("source.archive");
        let source = FromLocalArchive::new(&source_path);
        let archive = source.fetch(&destination).unwrap();

        assert_eq!(archive.metadata().path, destination);
        assert_eq!(archive.metadata().url, None);
        assert!(archive.metadata().checksum.len() == 64);

        let extracted = archive.extract(&dir.path().join("extracted")).unwrap();
        assert_eq!(extracted.root, dir.path().join("extracted/demo-0.1.0"));
    }
}
