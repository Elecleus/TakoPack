//! Source selection for the Rust CLI commands.
//!
//! The two sources are represented as an enum instead of optional flags:
//! crates.io needs a crate name/version, while a remote archive needs a URL
//! and an optional checksum.

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
}
