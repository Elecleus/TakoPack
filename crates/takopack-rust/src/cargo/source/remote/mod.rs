//! Generic remote archive source.
//!
//! Given a URL and an optional SHA-256 checksum, this module downloads a
//! gzip-compressed tar archive (`.crate` or `.tar.gz`) and feeds it into the
//! same [`crate::cargo::source::CargoSource`] pipeline as the crates.io
//! source. The archive may contain a single crate or a complete Cargo
//! workspace, including an unmodified Git repository archive. Extraction and
//! metadata parsing remain in [`crate::cargo::source::Archive`] and
//! [`crate::cargo::process::Process`].

mod archive;
mod builder;
mod error;

pub use archive::FromRemote;
pub use builder::FromRemoteBuilder;
pub use error::Error;
