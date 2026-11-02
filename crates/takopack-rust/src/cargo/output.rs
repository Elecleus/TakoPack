//! Curated, downstream-facing output of the loading pipeline.
//!
//! [`Output`] is the single type downstream packaging code consumes. It only
//! contains packaging-relevant data:
//!
//! - the source archive (identity, URL, checksum, local path);
//! - one entry per workspace package;
//! - per package: identity, packaging metadata, library/binary shape, the
//!   base dependency set and one precomputed closure per feature.
//!
//! Cargo-only details (`edition`, `origin`, target source paths, dev
//! dependencies, dependencies for other platforms) are intentionally not
//! exposed.
//!
//! RPM policy is intentionally left to the consumer: package names, compat
//! versions, summaries, SPDX conversion and `crate(...)` capability strings
//! are not generated here.

use std::path::PathBuf;

use semver::Version;

use crate::cargo::{
    metadata::{Dependency, FeatureClosure, PackageMetadata, WorkspaceMetadata},
    source::SourceArchive,
};

pub use crate::cargo::metadata::{DependencyKind, VersionComparator, VersionOp, VersionRange};

/// Everything the packaging layer needs for one crate source.
#[derive(Debug, Clone)]
pub struct Output {
    /// The source archive this output was built from.
    pub source: SourceArchive,
    /// Workspace root as reported by cargo.
    pub workspace_root: PathBuf,
    /// Packages of the workspace, in `workspace_members` order.
    pub packages: Vec<OutputPackage>,
}

impl Output {
    /// Build a downstream output from parsed metadata and its source archive.
    pub(crate) fn new(metadata: WorkspaceMetadata, source: SourceArchive) -> Self {
        let packages = metadata
            .packages
            .iter()
            .map(OutputPackage::from_metadata)
            .collect();

        Self {
            source,
            workspace_root: metadata.workspace_root,
            packages,
        }
    }

    /// Find a package by published name.
    pub fn package(&self, name: &str) -> Option<&OutputPackage> {
        self.packages.iter().find(|package| package.name == name)
    }
}

/// Downstream view of one workspace package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputPackage {
    /// Package name as published.
    pub name: String,
    /// Full SemVer version.
    pub version: Version,
    /// Minimum supported Rust version, when declared.
    pub rust_version: Option<Version>,
    /// Raw `description`, trimmed; blank becomes `None`.
    pub description: Option<String>,
    /// Raw `license`, trimmed; blank becomes `None`.
    pub license: Option<String>,
    /// `license-file`, relative to the manifest.
    pub license_file: Option<PathBuf>,
    /// `readme`, relative to the manifest.
    pub readme: Option<PathBuf>,
    /// `homepage` URL.
    pub homepage: Option<String>,
    /// `repository` URL.
    pub repository: Option<String>,
    /// `package.links`, when the package links a native library.
    pub links: Option<String>,
    /// Whether the package exposes a library target.
    pub is_lib: bool,
    /// Whether the package exposes a procedural macro target.
    pub is_proc_macro: bool,
    /// Binary target names, sorted and deduplicated.
    pub bins: Vec<String>,
    /// Dependencies that belong to the base (no default features) package.
    pub base_dependencies: Vec<OutputDependency>,
    /// Every feature, including `default`, with its precomputed closure.
    pub features: Vec<OutputFeature>,
}

impl OutputPackage {
    pub(crate) fn from_metadata(package: &PackageMetadata) -> Self {
        let base: Vec<&Dependency> = package
            .base_dependencies()
            .filter(|dependency| !is_ignored(dependency))
            .collect();

        let base_dependencies = normalize(
            base.iter()
                .map(|dependency| output_dependency(dependency, None))
                .collect(),
        );

        let features = package
            .features
            .iter()
            .filter(|(name, _)| !name.is_empty())
            .map(|(name, _)| {
                let closure = package.feature_closure(name);
                OutputFeature {
                    name: name.to_owned(),
                    features: closure
                        .as_ref()
                        .map(|closure| {
                            closure
                                .features
                                .iter()
                                .filter(|candidate| *candidate != name)
                                .cloned()
                                .collect()
                        })
                        .unwrap_or_default(),
                    dependencies: closure
                        .as_ref()
                        .map(|closure| feature_dependencies(closure, &base))
                        .unwrap_or_default(),
                }
            })
            .collect();

        Self {
            name: package.name.clone(),
            version: package.version.clone(),
            rust_version: package.rust_version.clone(),
            description: package.description.clone(),
            license: package.license.clone(),
            license_file: package.license_file.clone(),
            readme: package.readme.clone(),
            homepage: package.homepage.clone(),
            repository: package.repository.clone(),
            links: package.links.clone(),
            is_lib: package.is_lib(),
            is_proc_macro: package.is_proc_macro(),
            bins: package
                .bin_targets()
                .into_iter()
                .map(str::to_owned)
                .collect(),
            base_dependencies,
            features,
        }
    }
}

/// One feature of an [`OutputPackage`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFeature {
    /// Feature name.
    pub name: String,
    /// Other features reachable from this feature, including `""` for the
    /// base package. The feature itself is not included.
    pub features: Vec<String>,
    /// External dependencies activated by this feature, excluding the ones
    /// already covered by [`OutputPackage::base_dependencies`].
    pub dependencies: Vec<OutputDependency>,
}

/// A dependency needed by the base package or by one feature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputDependency {
    /// Package name of the dependency.
    pub package_name: String,
    /// Features requested from the dependency: declared features plus the
    /// feature requested by the activating feature expression.
    pub features: Vec<String>,
    /// Whether the dependency's default features are enabled.
    pub default_features: bool,
    /// Approximate lower bound of the version requirement.
    pub lower_bound: Option<Version>,
    /// Version requirement translated into RPM-consumable comparators.
    pub rpm_range: Option<VersionRange>,
    /// Dependency kind. Dev dependencies are filtered out.
    pub kind: DependencyKind,
    /// Where the dependency comes from.
    pub source: DependencySource,
}

/// Where an [`OutputDependency`] comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DependencySource {
    /// A registry dependency.
    Registry,
    /// A path dependency (typically another workspace member).
    Path,
    /// A git dependency.
    Git,
    /// Any other source.
    Other,
}

fn feature_dependencies(
    closure: &FeatureClosure<'_>,
    base: &[&Dependency],
) -> Vec<OutputDependency> {
    let dependencies = closure
        .dependencies
        .iter()
        .filter(|resolved| {
            let dependency = resolved.dependency;
            !is_ignored(dependency)
                && !covered_by_base(dependency, resolved.feature.as_deref(), base)
        })
        .map(|resolved| output_dependency(resolved.dependency, resolved.feature.as_deref()))
        .collect();

    normalize(dependencies)
}

fn covered_by_base(
    dependency: &Dependency,
    requested_feature: Option<&str>,
    base: &[&Dependency],
) -> bool {
    base.iter().any(|base_dependency| {
        base_dependency.toml_name == dependency.toml_name
            && base_dependency.kind == dependency.kind
            && base_dependency.platform == dependency.platform
            && requested_feature.is_none_or(|feature| {
                base_dependency
                    .features
                    .iter()
                    .any(|base_feature| base_feature == feature)
            })
    })
}

fn output_dependency(dependency: &Dependency, requested_feature: Option<&str>) -> OutputDependency {
    let mut features = dependency.features.clone();
    if let Some(feature) = requested_feature {
        features.push(feature.to_owned());
    }
    features.sort();
    features.dedup();

    OutputDependency {
        package_name: dependency.package_name.clone(),
        features,
        default_features: dependency.default_features,
        lower_bound: dependency.lower_bound.clone(),
        rpm_range: dependency.rpm_range.clone(),
        kind: dependency.kind,
        source: dependency_source(dependency),
    }
}

fn dependency_source(dependency: &Dependency) -> DependencySource {
    if dependency.path.is_some() {
        return DependencySource::Path;
    }

    match dependency.source.as_deref() {
        Some(source) if source.starts_with("registry+") => DependencySource::Registry,
        Some(source) if source.starts_with("git+") => DependencySource::Git,
        Some(_) => DependencySource::Other,
        None => DependencySource::Other,
    }
}

/// Dev dependencies, dependencies for other platforms and rustc's internal
/// workspace crates are not useful for RPM packaging.
fn is_ignored(dependency: &Dependency) -> bool {
    !dependency.enabled
        || dependency.kind == DependencyKind::Dev
        || is_special_crate(&dependency.package_name)
}

fn is_special_crate(package_name: &str) -> bool {
    matches!(
        package_name.replace('_', "-").as_str(),
        "rustc-std-workspace-core" | "rustc-std-workspace-alloc" | "rustc-std-workspace-std"
    )
}

fn normalize(mut dependencies: Vec<OutputDependency>) -> Vec<OutputDependency> {
    for dependency in &mut dependencies {
        dependency.features.sort();
        dependency.features.dedup();
    }

    dependencies.sort_by(|left, right| {
        left.package_name
            .cmp(&right.package_name)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.source.cmp(&right.source))
            .then_with(|| left.lower_bound.cmp(&right.lower_bound))
            .then_with(|| left.features.cmp(&right.features))
    });
    dependencies.dedup();
    dependencies
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::cargo::metadata::from_metadata;

    use super::*;

    fn dependency(
        name: &str,
        requirement: &str,
        kind: Option<&str>,
        target: Option<&str>,
        optional: bool,
        source: Option<&str>,
    ) -> Value {
        json!({
            "name": name,
            "source": source,
            "req": requirement,
            "kind": kind,
            "optional": optional,
            "uses_default_features": true,
            "features": [],
            "target": target,
            "rename": null,
            "registry": null,
            "path": null
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

    fn workspace_metadata() -> WorkspaceMetadata {
        let registry = "registry+https://github.com/rust-lang/crates.io-index";
        let package = json!({
            "name": "demo",
            "version": "1.2.3",
            "id": "path+file:///ws/demo/Cargo.toml#1.2.3",
            "source": null,
            "description": "A demo package.",
            "dependencies": [
                dependency("plain", "2", None, None, false, Some(registry)),
                dependency("bld", "1", Some("build"), None, false, Some(registry)),
                dependency("dev", "1", Some("dev"), None, false, Some(registry)),
                dependency("win", "3", None, Some("cfg(windows)"), false, Some(registry)),
                dependency("opt", "1.0", None, None, true, Some(registry)),
                dependency(
                    "rustc-std-workspace-core",
                    "1",
                    None,
                    None,
                    false,
                    Some(registry)
                ),
                json!({
                    "name": "local",
                    "source": null,
                    "req": "*",
                    "kind": null,
                    "optional": false,
                    "uses_default_features": true,
                    "features": [],
                    "target": null,
                    "rename": null,
                    "registry": null,
                    "path": "/ws/local"
                })
            ],
            "license": "MIT",
            "license_file": null,
            "targets": [
                target("demo", &["lib"], "/ws/demo/src/lib.rs"),
                target("demo", &["bin"], "/ws/demo/src/main.rs")
            ],
            "features": {
                "default": ["extra"],
                "extra": ["dep:opt"],
                "weak": ["opt?/feat"]
            },
            "manifest_path": "/ws/demo/Cargo.toml",
            "metadata": null,
            "publish": null,
            "authors": [],
            "categories": [],
            "keywords": [],
            "readme": "README.md",
            "repository": "https://example.com/demo",
            "homepage": null,
            "documentation": null,
            "edition": "2021",
            "links": null,
            "default_run": null,
            "rust_version": "1.70"
        });

        let metadata = json!({
            "packages": [package],
            "workspace_members": ["path+file:///ws/demo/Cargo.toml#1.2.3"],
            "workspace_default_members": ["path+file:///ws/demo/Cargo.toml#1.2.3"],
            "resolve": null,
            "workspace_root": "/ws",
            "target_directory": "/ws/target",
            "build_directory": null,
            "version": 1,
            "metadata": null
        });

        let metadata: cargo_metadata::Metadata =
            serde_json::from_value(metadata).expect("metadata fixture should deserialize");
        from_metadata(&metadata).expect("workspace should parse")
    }

    fn source_archive() -> SourceArchive {
        SourceArchive {
            id: "demo-1.2.3".to_owned(),
            path: PathBuf::from("/ws/demo-1.2.3.crate"),
            checksum: "a".repeat(64),
            url: "https://static.crates.io/crates/demo/demo-1.2.3.crate".to_owned(),
        }
    }

    #[test]
    fn base_dependencies_are_filtered_and_classified() {
        let output = Output::new(workspace_metadata(), source_archive());
        let package = &output.packages[0];

        let names: Vec<&str> = package
            .base_dependencies
            .iter()
            .map(|dependency| dependency.package_name.as_str())
            .collect();
        assert_eq!(names, vec!["bld", "local", "plain"]);

        assert_eq!(package.base_dependencies[0].kind, DependencyKind::Build);
        assert_eq!(package.base_dependencies[1].source, DependencySource::Path);
        assert_eq!(
            package.base_dependencies[2].source,
            DependencySource::Registry
        );
        assert!(
            package
                .base_dependencies
                .iter()
                .all(|dependency| dependency.package_name != "dev"
                    && dependency.package_name != "win"
                    && dependency.package_name != "opt"
                    && dependency.package_name != "rustc-std-workspace-core")
        );
    }

    #[test]
    fn features_carry_precomputed_closures() {
        let output = Output::new(workspace_metadata(), source_archive());
        let package = &output.packages[0];

        let default = package
            .features
            .iter()
            .find(|feature| feature.name == "default")
            .expect("default feature");
        assert_eq!(default.features, vec!["".to_owned(), "extra".to_owned()]);
        assert_eq!(default.dependencies.len(), 1);
        assert_eq!(default.dependencies[0].package_name, "opt");

        let extra = package
            .features
            .iter()
            .find(|feature| feature.name == "extra")
            .expect("extra feature");
        assert_eq!(extra.dependencies[0].package_name, "opt");

        let weak = package
            .features
            .iter()
            .find(|feature| feature.name == "weak")
            .expect("weak feature");
        assert!(weak.dependencies.is_empty());
    }

    #[test]
    fn package_shape_and_metadata_are_curated() {
        let output = Output::new(workspace_metadata(), source_archive());
        let package = &output.packages[0];

        assert_eq!(&output.workspace_root, &PathBuf::from("/ws"));
        assert_eq!(package.name, "demo");
        assert_eq!(package.version.to_string(), "1.2.3");
        assert_eq!(package.rust_version, Some(Version::new(1, 70, 0)));
        assert_eq!(package.description.as_deref(), Some("A demo package."));
        assert_eq!(package.license.as_deref(), Some("MIT"));
        assert_eq!(package.readme, Some(PathBuf::from("README.md")));
        assert_eq!(
            package.repository.as_deref(),
            Some("https://example.com/demo")
        );
        assert!(package.is_lib);
        assert!(!package.is_proc_macro);
        assert_eq!(package.bins, vec!["demo"]);
        assert_eq!(
            output.package("demo").map(|package| package.name.as_str()),
            Some("demo")
        );
        assert!(output.package("missing").is_none());
        assert_eq!(output.source.checksum, "a".repeat(64));
    }
}
