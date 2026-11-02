//! Parsing of cargo build targets.

use std::path::PathBuf;

use cargo_metadata::{Package, Target as CargoTarget, TargetKind as CargoTargetKind};

/// Kind of a build target, mirroring cargo's representation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TargetKind {
    /// `bench` target.
    Bench,
    /// `bin` target.
    Bin,
    /// `build.rs` target.
    CustomBuild,
    /// `cdylib` target.
    CDyLib,
    /// `dylib` target.
    DyLib,
    /// `example` target.
    Example,
    /// `lib` target.
    Lib,
    /// `proc-macro` target.
    ProcMacro,
    /// `rlib` target.
    RLib,
    /// `staticlib` target.
    StaticLib,
    /// A kind this parser does not know about.
    Unknown(String),
}

impl TargetKind {
    /// Whether the kind produces a library artifact.
    pub fn is_library(&self) -> bool {
        matches!(
            self,
            Self::Lib | Self::ProcMacro | Self::RLib | Self::DyLib | Self::CDyLib | Self::StaticLib
        )
    }
}

/// A parsed build target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// Target name as reported by cargo.
    pub name: String,
    /// All target kinds reported by cargo, in order.
    pub kinds: Vec<TargetKind>,
    /// Cargo crate types (`lib`, `rlib`, `proc-macro`, ...).
    pub crate_types: Vec<String>,
    /// Features required to build this target.
    pub required_features: Vec<String>,
    /// Source path as reported by cargo (absolute in normal metadata).
    pub src_path: PathBuf,
}

impl Target {
    /// Whether this target produces a library artifact.
    pub fn is_library(&self) -> bool {
        self.kinds.iter().any(TargetKind::is_library)
    }
}

/// Parse every target of `package`, sorted by name.
pub(crate) fn parse(package: &Package) -> Vec<Target> {
    let mut targets: Vec<Target> = package.targets.iter().map(parse_one).collect();
    targets.sort_by(|left, right| left.name.cmp(&right.name));
    targets
}

fn parse_one(target: &CargoTarget) -> Target {
    Target {
        name: target.name.clone(),
        kinds: target.kind.iter().map(kind).collect(),
        crate_types: target
            .crate_types
            .iter()
            .map(std::string::ToString::to_string)
            .collect(),
        required_features: target.required_features.clone(),
        src_path: target.src_path.as_std_path().to_path_buf(),
    }
}

fn kind(kind: &CargoTargetKind) -> TargetKind {
    match kind {
        CargoTargetKind::Bench => TargetKind::Bench,
        CargoTargetKind::Bin => TargetKind::Bin,
        CargoTargetKind::CustomBuild => TargetKind::CustomBuild,
        CargoTargetKind::CDyLib => TargetKind::CDyLib,
        CargoTargetKind::DyLib => TargetKind::DyLib,
        CargoTargetKind::Example => TargetKind::Example,
        CargoTargetKind::Lib => TargetKind::Lib,
        CargoTargetKind::ProcMacro => TargetKind::ProcMacro,
        CargoTargetKind::RLib => TargetKind::RLib,
        CargoTargetKind::StaticLib => TargetKind::StaticLib,
        CargoTargetKind::Unknown(kind) => TargetKind::Unknown(kind.clone()),
        other => TargetKind::Unknown(format!("{other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn target(value: Value) -> CargoTarget {
        serde_json::from_value(value).expect("target fixture should deserialize")
    }

    fn fixture(name: &str, kinds: &[&str], src_path: &str) -> CargoTarget {
        target(json!({
            "kind": kinds,
            "crate_types": kinds,
            "name": name,
            "src_path": src_path,
            "edition": "2021",
            "doc": true,
            "doctest": true,
            "test": true
        }))
    }

    #[test]
    fn kinds_and_crate_types_are_preserved() {
        let parsed = parse_one(&fixture("demo", &["cdylib", "rlib"], "/ws/demo/src/lib.rs"));

        assert_eq!(parsed.name, "demo");
        assert_eq!(parsed.kinds, vec![TargetKind::CDyLib, TargetKind::RLib]);
        assert_eq!(parsed.crate_types, vec!["cdylib", "rlib"]);
        assert_eq!(parsed.src_path, PathBuf::from("/ws/demo/src/lib.rs"));
        assert!(parsed.is_library());
    }

    #[test]
    fn proc_macro_is_a_library_kind() {
        let parsed = parse_one(&fixture("demo", &["proc-macro"], "/ws/demo/src/lib.rs"));
        assert_eq!(parsed.kinds, vec![TargetKind::ProcMacro]);
        assert!(parsed.is_library());
    }

    #[test]
    fn binary_is_not_a_library_kind() {
        let parsed = parse_one(&fixture("demo", &["bin"], "/ws/demo/src/main.rs"));
        assert_eq!(parsed.kinds, vec![TargetKind::Bin]);
        assert!(!parsed.is_library());
    }
}
