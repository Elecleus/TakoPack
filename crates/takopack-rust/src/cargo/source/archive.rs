use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::cargo::source::Error;

/// Data about a fetched archive that downstream code may need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveMetadata {
    /// Path to the downloaded archive file.
    pub path: PathBuf,
    /// SHA-256 checksum of the archive.
    pub checksum: String,
    /// URL the archive was downloaded from, when known.
    pub url: Option<String>,
}

impl ArchiveMetadata {
    /// Compute the checksum of `path`.
    pub fn try_new(path: &Path, url: Option<String>) -> Result<Self, Error> {
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();
        io::copy(&mut file, &mut hasher)?;

        Ok(Self {
            path: path.to_path_buf(),
            checksum: format!("{:x}", hasher.finalize()),
            url,
        })
    }

    /// Compute the checksum and verify it against `expected`.
    pub fn try_new_hashchecked(
        path: &Path,
        url: Option<String>,
        expected: &str,
    ) -> Result<Self, Error> {
        let metadata = Self::try_new(path, url)?;
        if metadata.checksum == expected {
            Ok(metadata)
        } else {
            Err(Error::ChecksumMismatch {
                got: metadata.checksum,
                expected: expected.to_owned(),
            })
        }
    }
}

/// A source root extracted from an archive.
#[derive(Debug, Clone)]
pub struct ExtractedSource {
    /// Directory containing the root `Cargo.toml`.
    pub root: PathBuf,
    /// Path to the root `Cargo.toml`.
    pub manifest: PathBuf,
}

/// An archive whose extraction strategy is source-specific.
pub trait Archive {
    type Error: std::error::Error + Send + Sync + 'static;

    fn metadata(&self) -> &ArchiveMetadata;

    /// Extract into `destination` and locate the Cargo workspace root.
    fn extract(&self, destination: &Path) -> Result<ExtractedSource, Self::Error>;
}

/// A gzip-compressed tar archive, used by crates.io and generic remote sources.
#[derive(Debug)]
pub struct TarGzArchive {
    metadata: ArchiveMetadata,
}

impl TarGzArchive {
    pub fn new(metadata: ArchiveMetadata) -> Self {
        Self { metadata }
    }
}

impl Archive for TarGzArchive {
    type Error = io::Error;

    fn metadata(&self) -> &ArchiveMetadata {
        &self.metadata
    }

    fn extract(&self, destination: &Path) -> Result<ExtractedSource, Self::Error> {
        std::fs::create_dir_all(destination)?;
        takopack_core::tarball::extract_tar_gz(&self.metadata.path, destination)?;

        let manifest = find_manifest(destination)?;
        let root = manifest.parent().unwrap_or(destination).to_path_buf();
        Ok(ExtractedSource { root, manifest })
    }
}

fn find_manifest(extract_dir: &Path) -> Result<PathBuf, io::Error> {
    let root = extract_dir.join("Cargo.toml");
    if root.is_file() {
        return Ok(root);
    }

    let mut candidates = Vec::new();
    for entry in std::fs::read_dir(extract_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let manifest = path.join("Cargo.toml");
        if manifest.is_file() {
            candidates.push(manifest);
        }
    }
    candidates.sort();

    match candidates.len() {
        1 => Ok(candidates.remove(0)),
        0 => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "no Cargo.toml found in extracted archive {}",
                extract_dir.display()
            ),
        )),
        _ => {
            let candidates = candidates
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "multiple Cargo.toml candidates in extracted archive {}: {candidates}",
                    extract_dir.display()
                ),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use tar::{Builder, Header};

    use super::*;

    fn write_archive(path: &Path, entries: &[(&str, &str)]) {
        let file = File::create(path).unwrap();
        let encoder = GzEncoder::new(file, Compression::default());
        let mut builder = Builder::new(encoder);

        for (name, contents) in entries {
            let mut header = Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, *name, contents.as_bytes())
                .unwrap();
        }

        let encoder = builder.into_inner().unwrap();
        encoder.finish().unwrap();
    }

    #[test]
    fn extracts_a_single_top_level_dir() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("demo.tar.gz");
        write_archive(
            &archive_path,
            &[
                ("demo-0.1.0/Cargo.toml", "[package]\nname = \"demo\"\n"),
                ("demo-0.1.0/lib.rs", ""),
            ],
        );

        let metadata = ArchiveMetadata::try_new(
            &archive_path,
            Some("https://example.com/demo.tar.gz".into()),
        )
        .unwrap();
        let archive = TarGzArchive::new(metadata);
        let destination = dir.path().join("extracted");
        let source = archive.extract(&destination).unwrap();

        assert_eq!(source.root, destination.join("demo-0.1.0"));
        assert_eq!(source.manifest, source.root.join("Cargo.toml"));
    }

    #[test]
    fn rejects_ambiguous_manifests() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("demo.tar.gz");
        write_archive(
            &archive_path,
            &[
                ("one/Cargo.toml", "[package]\nname = \"one\"\n"),
                ("two/Cargo.toml", "[package]\nname = \"two\"\n"),
            ],
        );

        let metadata = ArchiveMetadata::try_new(
            &archive_path,
            Some("https://example.com/demo.tar.gz".into()),
        )
        .unwrap();
        let archive = TarGzArchive::new(metadata);
        let error = archive.extract(&dir.path().join("extracted")).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}
