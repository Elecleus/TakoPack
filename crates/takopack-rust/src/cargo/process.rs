//! Loading pipeline: source archive -> extracted manifest -> parsed metadata.
//!
//! [`Process`] owns a [`CargoSource`], loads and extracts it into the caller
//! provided base directory, asks cargo for metadata, parses that metadata with the internal metadata parser and projects it into a downstream-facing
//! [`Output`].

use std::{
    cell::OnceCell,
    path::{Path, PathBuf},
};

use crate::cargo::{
    cargo_dir::CargoDir,
    metadata,
    output::{Output, OutputPackage},
    source::CargoSource,
};

/// Loading pipeline for one crate source.
#[derive(Debug)]
pub struct Process<S>
where
    S: CargoSource,
{
    source: S,
    output: OnceCell<Output>,
    base_path: PathBuf,
}

impl<S> Process<S>
where
    S: CargoSource,
{
    /// Create a process that extracts `source` into `base_dir`.
    pub fn from(source: S, base_dir: impl AsRef<Path>) -> Self {
        Self {
            source,
            output: OnceCell::new(),
            base_path: base_dir.as_ref().to_path_buf(),
        }
    }

    /// The source this process loads.
    pub fn source(&self) -> &S {
        &self.source
    }

    /// Load the source, extract it, obtain cargo metadata, parse it and build
    /// the downstream [`Output`].
    ///
    /// The first successful call returns the output; later calls return the
    /// same output without doing the work again.
    pub fn load(&mut self) -> Result<&Output, Error>
    where
        S::Error: std::error::Error + Send + Sync + 'static,
    {
        if let Some(output) = self.output.get() {
            return Ok(output);
        }

        let archive = self
            .source
            .load_archive(&self.base_path)
            .map_err(|error| Error::Source(Box::new(error)))?
            .clone();

        let manifest_path = archive.extract()?;
        let cargo_dir = CargoDir::new(
            manifest_path
                .parent()
                .expect("SourceArchive::extract returns a manifest path"),
        );
        let raw_metadata = cargo_dir.get_metadata_no_deps()?;
        let metadata = metadata::from_metadata(&raw_metadata)?;
        let output = Output::new(metadata, archive);

        self.output.set(output).expect("output cell is empty");
        Ok(self.output.get().expect("output was just set"))
    }

    /// The downstream output produced by [`Self::load`].
    pub fn output(&self) -> Result<&Output, Error> {
        self.output.get().ok_or(Error::NotLoaded)
    }

    /// The package corresponding to this process's source.
    ///
    /// The package is selected by the source's crate name. When the extracted
    /// workspace contains exactly one package, that package is returned even
    /// if the names differ.
    pub fn package(&self) -> Result<&OutputPackage, Error> {
        let output = self.output()?;
        let name = self.source.name();

        output
            .package(name)
            .or_else(|| (output.packages.len() == 1).then(|| &output.packages[0]))
            .ok_or_else(|| Error::PackageNotFound {
                name: name.to_owned(),
            })
    }
}

/// Failure while loading or parsing a crate source.
#[derive(Debug)]
pub enum Error {
    /// [`Process::load`] has not completed successfully yet.
    NotLoaded,
    /// The source archive could not be loaded.
    Source(Box<dyn std::error::Error + Send + Sync + 'static>),
    /// `cargo metadata` could not be obtained.
    CargoDir(crate::cargo::cargo_dir::Error),
    /// Cargo metadata could not be parsed.
    Metadata(String),
    /// The source's package is not present in the extracted workspace.
    PackageNotFound {
        /// The crate name reported by the source.
        name: String,
    },
    /// A filesystem operation failed.
    Io(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotLoaded => formatter.write_str("process has not been loaded yet"),
            Self::Source(error) => write!(formatter, "failed to load source: {error}"),
            Self::CargoDir(error) => write!(formatter, "failed to read cargo metadata: {error:?}"),
            Self::Metadata(error) => write!(formatter, "failed to parse cargo metadata: {error}"),
            Self::PackageNotFound { name } => {
                write!(
                    formatter,
                    "no package named {name:?} in the loaded workspace"
                )
            }
            Self::Io(error) => write!(formatter, "i/o error: {error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error.as_ref()),
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<crate::cargo::cargo_dir::Error> for Error {
    fn from(error: crate::cargo::cargo_dir::Error) -> Self {
        Self::CargoDir(error)
    }
}

impl From<metadata::Error> for Error {
    fn from(error: metadata::Error) -> Self {
        Self::Metadata(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::process::Command;

    use flate2::Compression;
    use flate2::write::GzEncoder;
    use tar::{Builder, Header};

    use crate::cargo::source::SourceArchive;

    use super::*;

    #[derive(Debug)]
    struct FakeSource {
        archive: SourceArchive,
        name: String,
        version: String,
    }

    impl CargoSource for FakeSource {
        type Error = std::io::Error;

        fn name(&self) -> &str {
            &self.name
        }

        fn version(&self) -> &str {
            &self.version
        }

        fn load_archive(&self, _dir: &impl AsRef<Path>) -> Result<&SourceArchive, Self::Error> {
            Ok(&self.archive)
        }
    }

    fn cargo_available() -> bool {
        Command::new("cargo")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn make_crate(dir: &Path) -> PathBuf {
        const MANIFEST: &str = r#"
[package]
name = "demo"
version = "0.1.0"
edition = "2021"

[lib]
path = "lib.rs"

[workspace]
"#;

        let archive_path = dir.join("demo-0.1.0.crate");
        let file = File::create(&archive_path).unwrap();
        let encoder = GzEncoder::new(file, Compression::default());
        let mut builder = Builder::new(encoder);

        for (name, contents) in [("Cargo.toml", MANIFEST), ("lib.rs", "")] {
            let mut header = Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(
                    &mut header,
                    format!("demo-0.1.0/{name}"),
                    contents.as_bytes(),
                )
                .unwrap();
        }

        let encoder = builder.into_inner().unwrap();
        encoder.finish().unwrap();
        archive_path
    }

    #[test]
    fn load_returns_output_and_is_idempotent() {
        if !cargo_available() {
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let archive_path = make_crate(dir.path());
        let archive = SourceArchive::try_new(
            "demo-0.1.0",
            &archive_path,
            "https://example.com/demo-0.1.0.crate",
        )
        .unwrap();
        let source = FakeSource {
            archive,
            name: "demo".to_owned(),
            version: "0.1.0".to_owned(),
        };

        let mut process = Process::from(source, dir.path());
        assert!(matches!(process.package(), Err(Error::NotLoaded)));

        {
            let output = process.load().unwrap();
            assert_eq!(output.packages.len(), 1);
            assert_eq!(output.packages[0].name, "demo");
            assert_eq!(output.source.checksum.len(), 64);
            assert_eq!(process.package().unwrap().name, "demo");
            assert_eq!(process.output().unwrap().packages[0].name, "demo");
        }

        // Loading is idempotent.
        let first = process.load().unwrap() as *const Output;
        let second = process.load().unwrap() as *const Output;
        assert_eq!(first, second);
    }
}
