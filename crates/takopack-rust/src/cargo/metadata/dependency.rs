//! Parsing of cargo dependency declarations.

use std::path::PathBuf;

use cargo_metadata::{
    Dependency as CargoDependency, DependencyKind as CargoDependencyKind, Package,
};
use semver::{BuildMetadata, Comparator, Op, Version, VersionReq};

use super::platform::TargetProfile;

/// Dependency kind as declared in `Cargo.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DependencyKind {
    /// `[dependencies]`.
    Normal,
    /// `[build-dependencies]`.
    Build,
    /// `[dev-dependencies]`.
    Dev,
}

/// A single parsed dependency declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    /// Key used in `Cargo.toml` (`rename`, or the package name). Feature
    /// strings refer to this name.
    pub toml_name: String,
    /// Actual package name, after renaming.
    pub package_name: String,
    /// Parsed version requirement.
    pub requirement: VersionReq,
    /// Approximate lower bound of `requirement`, when one can be derived.
    ///
    /// Only lower-bound comparators (`=`, `>=`, `~`, `^`) are considered;
    /// missing minor/patch components are filled with zero.
    pub lower_bound: Option<Version>,
    /// The requirement translated into RPM-consumable comparators.
    ///
    /// `None` when the translation would not be exact. Only stable versions
    /// are translated; requirements involving pre-release versions yield
    /// `None`.
    pub rpm_range: Option<VersionRange>,
    /// Dependency kind.
    pub kind: DependencyKind,
    /// Whether the dependency is optional.
    pub optional: bool,
    /// Whether the dependency's default features are enabled.
    pub default_features: bool,
    /// Features requested from the dependency, sorted and deduplicated.
    pub features: Vec<String>,
    /// Target restriction, as written by cargo (`cfg(...)` or a target name).
    pub platform: Option<String>,
    /// Whether the dependency applies to `x86_64-unknown-linux-gnu`.
    pub enabled: bool,
    /// Cargo source representation, when the dependency has one.
    pub source: Option<String>,
    /// Path of a path dependency, when the dependency has one.
    pub path: Option<PathBuf>,
}

impl Dependency {
    /// Whether this dependency belongs to the base feature node: non-optional,
    /// enabled on the target platform, and not dev-only.
    pub fn is_base(&self) -> bool {
        !self.optional && self.enabled && self.kind != DependencyKind::Dev
    }
}

/// All dependency declarations of a package, in deterministic order.
///
/// The order is `(package_name, kind, toml_name, platform)`, which makes
/// downstream iteration independent of cargo's serialization order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dependencies(Vec<Dependency>);

impl Dependencies {
    /// Iterate over all declarations.
    pub fn iter(&self) -> std::slice::Iter<'_, Dependency> {
        self.0.iter()
    }

    /// Number of declarations.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are no declarations.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Declarations that belong to the base feature node.
    pub fn base(&self) -> impl Iterator<Item = &Dependency> + '_ {
        self.0.iter().filter(|dependency| dependency.is_base())
    }

    /// Declarations of one kind.
    pub fn of_kind(&self, kind: DependencyKind) -> impl Iterator<Item = &Dependency> + '_ {
        self.0
            .iter()
            .filter(move |dependency| dependency.kind == kind)
    }

    /// Declarations filed under `toml_name`.
    ///
    /// There can be more than one declaration with the same key: the same
    /// crate may appear as a normal, build and dev dependency, or under
    /// different target platforms.
    pub fn by_toml_name<'a>(
        &'a self,
        toml_name: &str,
    ) -> impl Iterator<Item = &'a Dependency> + 'a {
        let toml_name = toml_name.to_owned();
        self.0
            .iter()
            .filter(move |dependency| dependency.toml_name == toml_name)
    }
}

impl FromIterator<Dependency> for Dependencies {
    fn from_iter<I: IntoIterator<Item = Dependency>>(dependencies: I) -> Self {
        let mut dependencies: Vec<_> = dependencies.into_iter().collect();
        dependencies.sort_by(|left, right| {
            left.package_name
                .cmp(&right.package_name)
                .then_with(|| left.kind.cmp(&right.kind))
                .then_with(|| left.toml_name.cmp(&right.toml_name))
                .then_with(|| left.platform.cmp(&right.platform))
        });
        Self(dependencies)
    }
}

impl<'a> IntoIterator for &'a Dependencies {
    type Item = &'a Dependency;
    type IntoIter = std::slice::Iter<'a, Dependency>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// A requirement translated into RPM-consumable comparison operators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionRange {
    /// No constraint (`*`).
    Any,
    /// Conjunction of comparators.
    Comparators(Vec<VersionComparator>),
}

/// One comparison of a version range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionComparator {
    /// Comparison operator.
    pub op: VersionOp,
    /// Compared version.
    pub version: Version,
}

/// Comparison operators understood by RPM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionOp {
    /// `=`.
    Eq,
    /// `>`.
    Gt,
    /// `>=`.
    Ge,
    /// `<`.
    Lt,
    /// `<=`.
    Le,
}

/// Parse every dependency declaration of a package.
pub(crate) fn parse(package: &Package, target: &TargetProfile) -> Dependencies {
    package
        .dependencies
        .iter()
        .map(|dependency| parse_one(dependency, target))
        .collect()
}

fn parse_one(dependency: &CargoDependency, target: &TargetProfile) -> Dependency {
    let mut features = dependency.features.clone();
    features.sort();
    features.dedup();

    Dependency {
        toml_name: dependency
            .rename
            .clone()
            .unwrap_or_else(|| dependency.name.clone()),
        package_name: dependency.name.clone(),
        requirement: dependency.req.clone(),
        lower_bound: lower_bound(&dependency.req),
        rpm_range: rpm_range(&dependency.req),
        kind: kind(dependency.kind),
        optional: dependency.optional,
        default_features: dependency.uses_default_features,
        features,
        platform: dependency
            .target
            .as_ref()
            .map(std::string::ToString::to_string),
        enabled: target.matches(dependency.target.as_ref()),
        source: dependency
            .source
            .as_ref()
            .map(std::string::ToString::to_string),
        path: dependency
            .path
            .as_ref()
            .map(|path| path.as_std_path().to_path_buf()),
    }
}

fn kind(kind: CargoDependencyKind) -> DependencyKind {
    match kind {
        CargoDependencyKind::Normal | CargoDependencyKind::Unknown => DependencyKind::Normal,
        CargoDependencyKind::Build => DependencyKind::Build,
        CargoDependencyKind::Development => DependencyKind::Dev,
    }
}

/// Approximate lower bound of a version requirement.
fn lower_bound(requirement: &VersionReq) -> Option<Version> {
    requirement
        .comparators
        .iter()
        .filter_map(comparator_lower_bound)
        .min()
}

fn comparator_lower_bound(comparator: &Comparator) -> Option<Version> {
    let is_lower_bound = matches!(
        comparator.op,
        Op::Exact | Op::GreaterEq | Op::Tilde | Op::Caret
    );
    if !is_lower_bound {
        return None;
    }

    Some(Version {
        major: comparator.major,
        minor: comparator.minor.unwrap_or(0),
        patch: comparator.patch.unwrap_or(0),
        pre: comparator.pre.clone(),
        build: BuildMetadata::EMPTY,
    })
}

/// Translate a requirement into RPM-consumable comparators.
///
/// Returns `None` for requirements that cannot be translated exactly, which
/// currently means requirements involving pre-release versions.
fn rpm_range(requirement: &VersionReq) -> Option<VersionRange> {
    if requirement.comparators.is_empty() {
        return Some(VersionRange::Any);
    }

    let mut comparators = Vec::new();
    for comparator in &requirement.comparators {
        comparators.extend(translate_comparator(comparator)?);
    }
    Some(VersionRange::Comparators(comparators))
}

fn translate_comparator(comparator: &Comparator) -> Option<Vec<VersionComparator>> {
    if !comparator.pre.is_empty() {
        return None;
    }

    let major = comparator.major;
    let minor = comparator.minor;
    let patch = comparator.patch;
    let lower = version(major, minor.unwrap_or(0), patch.unwrap_or(0));

    let comparators = match comparator.op {
        Op::Exact => match (minor, patch) {
            (Some(minor), Some(patch)) => {
                vec![cmp(VersionOp::Eq, version(major, minor, patch))]
            }
            (Some(minor), None) => vec![
                cmp(VersionOp::Ge, version(major, minor, 0)),
                cmp(VersionOp::Lt, version(major, minor + 1, 0)),
            ],
            (None, _) => vec![
                cmp(VersionOp::Ge, version(major, 0, 0)),
                cmp(VersionOp::Lt, version(major + 1, 0, 0)),
            ],
        },
        Op::GreaterEq => vec![cmp(VersionOp::Ge, lower)],
        Op::Greater => match (minor, patch) {
            (Some(minor), Some(patch)) => {
                vec![cmp(VersionOp::Gt, version(major, minor, patch))]
            }
            (Some(minor), None) => {
                vec![cmp(VersionOp::Ge, version(major, minor + 1, 0))]
            }
            (None, _) => vec![cmp(VersionOp::Ge, version(major + 1, 0, 0))],
        },
        Op::LessEq => match (minor, patch) {
            (Some(minor), Some(patch)) => {
                vec![cmp(VersionOp::Le, version(major, minor, patch))]
            }
            (Some(minor), None) => {
                vec![cmp(VersionOp::Lt, version(major, minor + 1, 0))]
            }
            (None, _) => vec![cmp(VersionOp::Lt, version(major + 1, 0, 0))],
        },
        Op::Less => vec![cmp(VersionOp::Lt, lower)],
        Op::Tilde => {
            let upper = match minor {
                Some(minor) => version(major, minor + 1, 0),
                None => version(major + 1, 0, 0),
            };
            vec![cmp(VersionOp::Ge, lower), cmp(VersionOp::Lt, upper)]
        }
        Op::Caret => {
            let upper = if major > 0 {
                version(major + 1, 0, 0)
            } else {
                match (minor, patch) {
                    (None, _) => version(1, 0, 0),
                    (Some(0), Some(patch)) => version(0, 0, patch + 1),
                    (Some(0), None) => version(0, 1, 0),
                    (Some(minor), _) => version(0, minor + 1, 0),
                }
            };
            vec![cmp(VersionOp::Ge, lower), cmp(VersionOp::Lt, upper)]
        }
        Op::Wildcard => {
            let upper = match minor {
                Some(minor) => version(major, minor + 1, 0),
                None => version(major + 1, 0, 0),
            };
            vec![cmp(VersionOp::Ge, lower), cmp(VersionOp::Lt, upper)]
        }
        _ => return None,
    };

    Some(comparators)
}

fn version(major: u64, minor: u64, patch: u64) -> Version {
    Version::new(major, minor, patch)
}

fn cmp(op: VersionOp, version: Version) -> VersionComparator {
    VersionComparator { op, version }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn dependency(value: Value) -> CargoDependency {
        serde_json::from_value(value).expect("dependency fixture should deserialize")
    }

    fn fixture(
        name: &str,
        requirement: &str,
        kind: Option<&str>,
        rename: Option<&str>,
        target: Option<&str>,
        optional: bool,
    ) -> CargoDependency {
        dependency(json!({
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
        }))
    }

    fn parse_one_linux(dependency: &CargoDependency) -> Dependency {
        parse_one(dependency, &TargetProfile::default())
    }

    fn range(requirement: &str) -> Option<VersionRange> {
        rpm_range(&VersionReq::parse(requirement).unwrap())
    }

    fn cmp(op: VersionOp, version: &str) -> VersionComparator {
        VersionComparator {
            op,
            version: Version::parse(version).unwrap(),
        }
    }

    #[test]
    fn rename_becomes_the_toml_name() {
        let dependency = fixture("renamed-dep", "^0.3", None, Some("optdep"), None, true);
        let parsed = parse_one_linux(&dependency);

        assert_eq!(parsed.toml_name, "optdep");
        assert_eq!(parsed.package_name, "renamed-dep");
        assert!(parsed.optional);
        assert_eq!(parsed.kind, DependencyKind::Normal);
    }

    #[test]
    fn kinds_platforms_and_paths_are_preserved() {
        let windows = fixture("win", "3", None, None, Some("cfg(windows)"), false);
        let windows = parse_one_linux(&windows);
        assert_eq!(windows.platform.as_deref(), Some("cfg(windows)"));
        assert!(!windows.enabled);

        let build = fixture("build", "1", Some("build"), None, None, false);
        assert_eq!(parse_one_linux(&build).kind, DependencyKind::Build);

        let dev = fixture("dev", "1", Some("dev"), None, None, false);
        assert_eq!(parse_one_linux(&dev).kind, DependencyKind::Dev);

        let path = dependency(json!({
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
        }));
        let path = parse_one_linux(&path);
        assert_eq!(path.path, Some(PathBuf::from("/ws/local")));
        assert!(path.enabled);
    }

    #[test]
    fn lower_bound_for_lower_bound_comparators() {
        let bound = |requirement: &str| lower_bound(&VersionReq::parse(requirement).unwrap());

        assert_eq!(bound("^1.2.3"), Some(Version::parse("1.2.3").unwrap()));
        assert_eq!(bound(">=1.2"), Some(Version::parse("1.2.0").unwrap()));
        assert_eq!(bound("~1.2.3"), Some(Version::parse("1.2.3").unwrap()));
        assert_eq!(bound(">=1.2, <2"), Some(Version::parse("1.2.0").unwrap()));
        assert_eq!(
            bound(">=1.2, >=1.3.1"),
            Some(Version::parse("1.2.0").unwrap())
        );
        assert_eq!(bound("*"), None);
        assert_eq!(bound("<2"), None);
        assert_eq!(bound(">1.2.3"), None);
    }

    #[test]
    fn caret_ranges_are_translated() {
        assert_eq!(
            range("^1.2.3"),
            Some(VersionRange::Comparators(vec![
                cmp(VersionOp::Ge, "1.2.3"),
                cmp(VersionOp::Lt, "2.0.0"),
            ]))
        );
        assert_eq!(
            range("^0.2.3"),
            Some(VersionRange::Comparators(vec![
                cmp(VersionOp::Ge, "0.2.3"),
                cmp(VersionOp::Lt, "0.3.0"),
            ]))
        );
        assert_eq!(
            range("^0.0.3"),
            Some(VersionRange::Comparators(vec![
                cmp(VersionOp::Ge, "0.0.3"),
                cmp(VersionOp::Lt, "0.0.4"),
            ]))
        );
    }

    #[test]
    fn tilde_and_exact_ranges_are_translated() {
        assert_eq!(
            range("~1.2.3"),
            Some(VersionRange::Comparators(vec![
                cmp(VersionOp::Ge, "1.2.3"),
                cmp(VersionOp::Lt, "1.3.0"),
            ]))
        );
        assert_eq!(
            range("=1.2.3"),
            Some(VersionRange::Comparators(vec![cmp(VersionOp::Eq, "1.2.3")]))
        );
        assert_eq!(
            range("=1"),
            Some(VersionRange::Comparators(vec![
                cmp(VersionOp::Ge, "1.0.0"),
                cmp(VersionOp::Lt, "2.0.0"),
            ]))
        );
    }

    #[test]
    fn simple_comparators_are_translated() {
        assert_eq!(
            range(">=1.2"),
            Some(VersionRange::Comparators(vec![cmp(VersionOp::Ge, "1.2.0")]))
        );
        assert_eq!(
            range(">1.2"),
            Some(VersionRange::Comparators(vec![cmp(VersionOp::Ge, "1.3.0")]))
        );
        assert_eq!(
            range("<2"),
            Some(VersionRange::Comparators(vec![cmp(VersionOp::Lt, "2.0.0")]))
        );
        assert_eq!(
            range("<=1.2"),
            Some(VersionRange::Comparators(vec![cmp(VersionOp::Lt, "1.3.0")]))
        );
    }

    #[test]
    fn wildcard_and_any_ranges_are_translated() {
        assert_eq!(range("*"), Some(VersionRange::Any));
        assert_eq!(
            range("1.*"),
            Some(VersionRange::Comparators(vec![
                cmp(VersionOp::Ge, "1.0.0"),
                cmp(VersionOp::Lt, "2.0.0"),
            ]))
        );
    }

    #[test]
    fn pre_release_ranges_are_not_translated() {
        assert_eq!(range("^0.26.0-beta.1"), None);
        assert_eq!(range("=1.2.3-rc.1"), None);
    }

    #[test]
    fn dependencies_are_sorted() {
        let dependencies: Dependencies = vec![
            parse_one_linux(&fixture("plain", "1", None, None, None, false)),
            parse_one_linux(&fixture("build", "1", Some("build"), None, None, false)),
        ]
        .into_iter()
        .collect();

        let names: Vec<&str> = dependencies
            .iter()
            .map(|dependency| dependency.package_name.as_str())
            .collect();
        assert_eq!(names, vec!["build", "plain"]);
    }
}
