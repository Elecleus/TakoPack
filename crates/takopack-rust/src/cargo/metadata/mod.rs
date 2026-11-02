//! Parsing of an already-obtained [`cargo_metadata::Metadata`].
//!
//! The module is closed: [`from_metadata`] takes nothing but `Metadata` and
//! returns a workspace-level view. It never invokes cargo, reads project
//! files, or needs configuration.
//!
//! # Workspace support
//!
//! `cargo metadata` run on a workspace root manifest already contains every
//! workspace member: [`Metadata::workspace_members`] identifies the members
//! and [`Metadata::packages`] carries their data. The parser filters `packages`
//! by `workspace_members`, in member order. This works for virtual workspace
//! manifests and for `cargo metadata --no-deps`.
//!
//! # What is parsed
//!
//! - Package identity, edition, rust-version, origin and manifest path.
//! - Raw package fields (`description`, license, URLs, readme, links), with
//!   blank strings normalized to `None`.
//! - Build targets (all cargo kinds preserved).
//! - Dependencies (all kinds and platforms), with `lower_bound`, an
//!   RPM-consumable `rpm_range` for stable requirements, and `enabled` for the
//!   Linux/x86_64 target.
//! - The feature graph, including the synthetic base node (`""`) and a
//!   `default` node.
//!
//! # What is not parsed
//!
//! RPM naming (`rust-...`), compat versions, summary/description splitting,
//! SPDX conversion, capability strings and checksums. Those depend on
//! configuration or on the source layer and belong to downstream modules.

mod dependency;
mod feature;
mod package;
mod platform;
mod target;

use std::collections::HashMap;
use std::path::PathBuf;

use cargo_metadata::{Metadata, Package};

pub use dependency::{Dependency, DependencyKind, VersionComparator, VersionOp, VersionRange};
pub use feature::FeatureClosure;
pub use package::PackageMetadata;

/// Parse every workspace member of `metadata`.
///
/// The returned packages follow `metadata.workspace_members` order. Dependency
/// packages are never returned, even when `metadata` was produced with a full
/// dependency resolution.
pub fn from_metadata(metadata: &Metadata) -> Result<WorkspaceMetadata, Error> {
    if metadata.workspace_members.is_empty() {
        return Err(Error::NoWorkspaceMembers);
    }

    let by_id: HashMap<&str, &Package> = metadata
        .packages
        .iter()
        .map(|package| (package.id.repr.as_str(), package))
        .collect();

    let mut packages = Vec::with_capacity(metadata.workspace_members.len());
    for id in &metadata.workspace_members {
        let package =
            by_id
                .get(id.repr.as_str())
                .copied()
                .ok_or_else(|| Error::MissingWorkspaceMember {
                    id: id.repr.clone(),
                })?;
        packages.push(PackageMetadata::parse(package));
    }

    Ok(WorkspaceMetadata {
        workspace_root: metadata.workspace_root.as_std_path().to_path_buf(),
        packages,
    })
}

/// Parsed metadata of a workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceMetadata {
    /// Workspace root as reported by cargo.
    pub workspace_root: PathBuf,
    /// Workspace members, in `workspace_members` order.
    pub packages: Vec<PackageMetadata>,
}

/// Failure while grouping `Metadata` into workspace packages.
#[derive(Debug)]
pub enum Error {
    /// `Metadata::workspace_members` is empty.
    NoWorkspaceMembers,
    /// A workspace member ID is not present in `Metadata::packages`.
    MissingWorkspaceMember {
        /// The missing package ID.
        id: String,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoWorkspaceMembers => {
                formatter.write_str("cargo metadata contains no workspace members")
            }
            Self::MissingWorkspaceMember { id } => {
                write!(formatter, "cargo metadata is missing workspace member {id}")
            }
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::package::{Edition, Origin};
    use super::target::TargetKind;
    use super::*;

    const APP_MANIFEST: &str = "/ws/app/Cargo.toml";
    const LIB_MANIFEST: &str = "/ws/lib/Cargo.toml";

    fn dependency(
        name: &str,
        requirement: &str,
        kind: Option<&str>,
        target: Option<&str>,
        optional: bool,
        rename: Option<&str>,
    ) -> Value {
        json!({
            "name": name,
            "source": null,
            "req": requirement,
            "kind": kind,
            "optional": optional,
            "uses_default_features": true,
            "features": [],
            "target": target,
            "rename": rename,
            "registry": null,
            "path": null
        })
    }

    fn path_dependency(name: &str, path: &str) -> Value {
        json!({
            "name": name,
            "source": null,
            "req": "*",
            "kind": null,
            "optional": false,
            "uses_default_features": true,
            "features": [],
            "target": null,
            "rename": null,
            "registry": null,
            "path": path
        })
    }

    fn target(name: &str, kinds: &[&str], src_path: &str) -> Value {
        json!({
            "kind": kinds,
            "crate_types": kinds,
            "name": name,
            "src_path": src_path,
            "edition": "2021",
            "doc": true,
            "doctest": true,
            "test": true
        })
    }

    fn package(
        name: &str,
        manifest: &str,
        dependencies: Value,
        features: Value,
        targets: Value,
    ) -> Value {
        json!({
            "name": name,
            "version": "1.2.3",
            "id": format!("path+file://{manifest}#1.2.3"),
            "source": null,
            "description": "A demo package.\n",
            "dependencies": dependencies,
            "license": "MIT",
            "license_file": null,
            "targets": targets,
            "features": features,
            "manifest_path": manifest,
            "metadata": null,
            "publish": null,
            "authors": [],
            "categories": [],
            "keywords": [],
            "readme": "README.md",
            "repository": "https://example.com/app",
            "homepage": null,
            "documentation": null,
            "edition": "2021",
            "links": null,
            "default_run": null,
            "rust_version": "1.70"
        })
    }

    fn metadata(packages: Vec<Value>, members: &[&str]) -> Metadata {
        serde_json::from_value(json!({
            "packages": packages,
            "workspace_members": members,
            "workspace_default_members": members,
            "resolve": null,
            "workspace_root": "/ws",
            "target_directory": "/ws/target",
            "build_directory": null,
            "version": 1,
            "metadata": null
        }))
        .expect("metadata fixture should deserialize")
    }

    fn workspace() -> Metadata {
        let app = package(
            "app",
            APP_MANIFEST,
            json!([
                dependency("dep", "^1.2.3", None, None, true, Some("renamed")),
                dependency("win", "3", None, Some("cfg(windows)"), false, None),
                dependency("bld", "1", Some("build"), None, false, None),
                path_dependency("local", "/ws/lib"),
            ]),
            json!({
                "default": ["extra"],
                "extra": ["dep:renamed"],
                "weak": ["renamed?/feat"]
            }),
            json!([
                target("app", &["lib"], "/ws/app/src/lib.rs"),
                target("app", &["bin"], "/ws/app/src/main.rs"),
            ]),
        );
        let lib = package(
            "lib",
            LIB_MANIFEST,
            json!([]),
            json!({}),
            json!([target("lib", &["lib"], "/ws/lib/src/lib.rs")]),
        );

        metadata(
            vec![app, lib],
            &[
                "path+file:///ws/app/Cargo.toml#1.2.3",
                "path+file:///ws/lib/Cargo.toml#1.2.3",
            ],
        )
    }

    fn app() -> PackageMetadata {
        // Parse through the public entry to keep the fixture close to real
        // metadata.
        let mut workspace = from_metadata(&workspace()).expect("workspace should parse");
        workspace.packages.remove(0)
    }

    #[test]
    fn parses_every_workspace_member_in_order() {
        let workspace = from_metadata(&workspace()).expect("workspace should parse");

        assert_eq!(workspace.workspace_root, PathBuf::from("/ws"));
        assert_eq!(workspace.packages.len(), 2);
        assert_eq!(workspace.packages[0].name, "app");
        assert_eq!(workspace.packages[1].name, "lib");

        let app = workspace
            .packages
            .iter()
            .find(|package| package.name == "app")
            .expect("app package");
        assert_eq!(app.manifest_path, PathBuf::from(APP_MANIFEST));
        assert!(
            workspace
                .packages
                .iter()
                .all(|package| package.manifest_path
                    != std::path::Path::new("/ws/missing/Cargo.toml"))
        );
    }

    #[test]
    fn package_fields_are_parsed() {
        let app = app();

        assert_eq!(app.version.to_string(), "1.2.3");
        assert_eq!(app.edition, Edition::Rust2021);
        assert_eq!(
            app.rust_version,
            Some(semver::Version::parse("1.70.0").unwrap())
        );
        assert_eq!(app.origin, Origin::Path);
        assert_eq!(app.manifest_path, PathBuf::from(APP_MANIFEST));
        assert_eq!(app.description.as_deref(), Some("A demo package."));
        assert_eq!(app.license.as_deref(), Some("MIT"));
        assert_eq!(app.readme, Some(PathBuf::from("README.md")));
        assert_eq!(app.repository.as_deref(), Some("https://example.com/app"));
        assert_eq!(app.homepage, None);
    }

    #[test]
    fn dependencies_are_parsed_and_classified() {
        let app = app();

        let names: Vec<&str> = app
            .dependencies
            .iter()
            .map(|dependency| dependency.package_name.as_str())
            .collect();
        assert_eq!(names, vec!["bld", "dep", "local", "win"]);

        let renamed = app.dependencies.by_toml_name("renamed").next().unwrap();
        assert_eq!(renamed.package_name, "dep");
        assert!(renamed.optional);
        assert!(renamed.enabled);
        assert_eq!(
            renamed.lower_bound,
            Some(semver::Version::parse("1.2.3").unwrap())
        );
        assert_eq!(
            renamed.rpm_range,
            Some(VersionRange::Comparators(vec![
                VersionComparator {
                    op: VersionOp::Ge,
                    version: semver::Version::parse("1.2.3").unwrap(),
                },
                VersionComparator {
                    op: VersionOp::Lt,
                    version: semver::Version::parse("2.0.0").unwrap(),
                },
            ]))
        );

        let windows = app.dependencies.by_toml_name("win").next().unwrap();
        assert_eq!(windows.platform.as_deref(), Some("cfg(windows)"));
        assert!(!windows.enabled);

        let build = app.dependencies.by_toml_name("bld").next().unwrap();
        assert_eq!(build.kind, DependencyKind::Build);

        let local = app.dependencies.by_toml_name("local").next().unwrap();
        assert_eq!(local.path, Some(PathBuf::from("/ws/lib")));
    }

    #[test]
    fn feature_graph_and_closures_are_parsed() {
        let app = app();

        for feature in ["", "default", "extra", "weak"] {
            assert!(
                app.features.iter().any(|(name, _)| name == feature),
                "missing {feature:?}"
            );
        }

        let closure = app.feature_closure("default").unwrap();
        let mut names: Vec<&str> = closure
            .dependencies
            .iter()
            .map(|resolved| resolved.dependency.package_name.as_str())
            .collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names, vec!["bld", "dep", "local"]);

        let closure = app.feature_closure("weak").unwrap();
        let names: Vec<&str> = closure
            .dependencies
            .iter()
            .map(|resolved| resolved.dependency.package_name.as_str())
            .collect();
        assert_eq!(names, vec!["bld", "local"]);

        let closure = app.feature_closure("default").unwrap();
        assert_eq!(
            closure.features,
            std::collections::BTreeSet::from([
                String::new(),
                "default".to_owned(),
                "extra".to_owned(),
            ])
        );
        assert!(app.feature_closure("missing").is_none());
    }

    #[test]
    fn targets_are_parsed() {
        let app = app();

        assert!(app.is_lib());
        assert!(!app.is_proc_macro());
        assert_eq!(app.bin_targets(), vec!["app"]);
        assert_eq!(app.targets.len(), 2);
        assert!(
            app.targets
                .iter()
                .any(|target| target.kinds.contains(&TargetKind::Lib))
        );
        assert!(
            app.targets
                .iter()
                .any(|target| target.kinds.contains(&TargetKind::Bin))
        );
    }

    #[test]
    fn empty_workspace_is_an_error() {
        let metadata = metadata(vec![], &[]);
        assert!(matches!(
            from_metadata(&metadata),
            Err(Error::NoWorkspaceMembers)
        ));
    }

    #[test]
    fn missing_member_is_an_error() {
        let metadata = metadata(vec![], &["path+file:///ws/missing#1.0.0"]);
        assert!(matches!(
            from_metadata(&metadata),
            Err(Error::MissingWorkspaceMember { .. })
        ));
    }
}
