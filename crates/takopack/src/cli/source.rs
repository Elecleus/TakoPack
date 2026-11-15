//! Source selection for the Rust CLI commands.
//!
//! Sources are represented as an enum: crates.io needs a crate name/version,
//! a remote archive needs a URL and optional checksum, and a local archive
//! needs a filesystem path.

use std::path::PathBuf;

use clap::Subcommand;

/// Source to load for `spec` and `output`.
#[derive(Debug, Clone, Subcommand)]
pub enum SourceCommand {
    /// Resolve a crate and version from crates.io.
    CratesIo {
        /// Name of the Rust crate.
        #[arg(value_name = "CRATE")]
        name: String,
        /// Version; defaults to the latest stable crates.io release.
        #[arg(value_name = "VERSION")]
        version: Option<String>,
    },
    /// Download a gzip-compressed tar archive.
    ///
    /// The archive may contain a single crate or a whole Cargo workspace,
    /// including an unmodified Git repository archive.
    Remote {
        /// Archive URL.
        #[arg(long, value_name = "URL")]
        url: String,
        /// Expected SHA-256 checksum of the archive.
        #[arg(long, value_name = "SHA256")]
        checksum: Option<String>,
    },
    /// Use a local archive file.
    Local {
        /// Path to the local gzip-compressed tar archive.
        #[arg(long, value_name = "PATH")]
        path: PathBuf,
    },
}
