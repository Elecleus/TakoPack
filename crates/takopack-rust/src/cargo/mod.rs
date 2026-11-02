//! Cargo-based metadata.
//!
//! [`Output`] is the downstream-facing result of the pipeline. Metadata
//! parsing lives in the crate-private `metadata` module; acquisition lives in
//! [`source`] and [`process`].

pub mod cargo_dir;
pub(crate) mod metadata;
pub mod output;
pub mod process;
pub mod source;

pub use output::{
    DependencyKind, DependencySource, Output, OutputDependency, OutputFeature, OutputPackage,
    VersionComparator, VersionOp, VersionRange,
};
pub use process::Process;
