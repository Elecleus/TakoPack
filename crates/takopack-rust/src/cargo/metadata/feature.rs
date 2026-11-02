//! Parsing of the cargo feature table into a feature graph.
//!
//! Cargo reports features as `name -> [value]`. A value is one of:
//!
//! | value           | meaning                                                    |
//! |-----------------|------------------------------------------------------------|
//! | `feature`       | another feature of the same crate                          |
//! | `dep:name`      | activate the optional dependency `name`                    |
//! | `name/feature`  | activate `name` and enable `feature` on it                 |
//! | `name?/feature` | enable `feature` on `name`, but only if `name` is active   |
//!
//! The graph adds two synthetic nodes: `""` (the base node, i.e. the crate
//! with no default features) and `"default"` when the manifest does not
//! declare it.

use std::collections::{BTreeMap, BTreeSet};

use super::dependency::{Dependencies, Dependency};

/// How a feature activates a dependency.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DependencyActivation {
    /// `dep:name`.
    Dependency {
        /// Dependency key in `Cargo.toml`.
        toml_name: String,
    },
    /// `name/feature`.
    Feature {
        /// Dependency key in `Cargo.toml`.
        toml_name: String,
        /// Feature requested from the dependency.
        feature: String,
    },
    /// `name?/feature`.
    WeakFeature {
        /// Dependency key in `Cargo.toml`.
        toml_name: String,
        /// Feature requested from the dependency.
        feature: String,
    },
}

impl DependencyActivation {
    /// The dependency key this activation refers to.
    pub fn toml_name(&self) -> &str {
        match self {
            Self::Dependency { toml_name }
            | Self::Feature { toml_name, .. }
            | Self::WeakFeature { toml_name, .. } => toml_name,
        }
    }

    /// The feature requested from the dependency, if any.
    pub fn feature(&self) -> Option<&str> {
        match self {
            Self::Dependency { .. } => None,
            Self::Feature { feature, .. } | Self::WeakFeature { feature, .. } => Some(feature),
        }
    }

    /// Whether this is a weak activation.
    pub fn is_weak(&self) -> bool {
        matches!(self, Self::WeakFeature { .. })
    }
}

/// One node in the feature graph.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FeatureNode {
    /// Other features enabled by this one.
    pub features: BTreeSet<String>,
    /// Dependencies activated by this one, sorted and deduplicated.
    pub dependencies: Vec<DependencyActivation>,
}

/// The feature graph of a package.
///
/// The node key [`FeatureGraph::BASE`] (`""`) is synthetic: it represents the
/// crate with no default features. [`FeatureGraph::DEFAULT`] (`"default"`)
/// always exists; it is synthesized as `default = [""]` when the manifest does
/// not declare it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FeatureGraph(BTreeMap<String, FeatureNode>);

impl FeatureGraph {
    /// Key of the synthetic base node.
    pub const BASE: &'static str = "";
    /// Key of the `default` feature.
    pub const DEFAULT: &'static str = "default";

    /// Iterate over feature nodes in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &FeatureNode)> + '_ {
        self.0
            .iter()
            .map(|(feature, node)| (feature.as_str(), node))
    }

    /// Compute the transitive closure of `feature`.
    ///
    /// The closure always includes the base node. Weak activations are applied
    /// only when their dependency is activated by a non-weak edge inside the
    /// same closure. Returns `None` when `feature` does not exist.
    pub fn closure<'a>(
        &'a self,
        feature: &str,
        dependencies: &'a Dependencies,
    ) -> Option<FeatureClosure<'a>> {
        self.0.get(feature)?;

        let features = self.reachable_features(feature);
        let mut activated_dependencies = BTreeSet::new();
        let mut weak_activations = Vec::new();
        let mut resolved = Vec::new();

        for feature in &features {
            let Some(node) = self.0.get(feature) else {
                continue;
            };
            for activation in &node.dependencies {
                if activation.is_weak() {
                    weak_activations.push(activation);
                } else {
                    activated_dependencies.insert(activation.toml_name());
                    resolved.extend(resolve_activation(activation, dependencies));
                }
            }
        }

        for activation in weak_activations {
            if activated_dependencies.contains(activation.toml_name()) {
                resolved.extend(resolve_activation(activation, dependencies));
            }
        }

        resolved.sort_by(|left, right| {
            left.dependency
                .toml_name
                .cmp(&right.dependency.toml_name)
                .then_with(|| left.dependency.kind.cmp(&right.dependency.kind))
                .then_with(|| left.dependency.platform.cmp(&right.dependency.platform))
                .then_with(|| left.feature.cmp(&right.feature))
        });
        resolved.dedup_by(|left, right| {
            left.dependency.toml_name == right.dependency.toml_name
                && left.dependency.kind == right.dependency.kind
                && left.dependency.platform == right.dependency.platform
                && left.feature == right.feature
        });

        Some(FeatureClosure {
            features,
            dependencies: resolved,
        })
    }

    fn reachable_features(&self, feature: &str) -> BTreeSet<String> {
        let mut reachable = BTreeSet::from([Self::BASE.to_owned()]);
        let mut pending = vec![feature.to_owned()];

        while let Some(current) = pending.pop() {
            if !reachable.insert(current.clone()) {
                continue;
            }
            let Some(node) = self.0.get(&current) else {
                continue;
            };
            pending.extend(
                node.features
                    .iter()
                    .filter(|feature| !reachable.contains(*feature))
                    .cloned(),
            );
        }

        reachable
    }
}

impl From<BTreeMap<String, FeatureNode>> for FeatureGraph {
    fn from(graph: BTreeMap<String, FeatureNode>) -> Self {
        Self(graph)
    }
}

impl std::ops::Index<&str> for FeatureGraph {
    type Output = FeatureNode;

    fn index(&self, feature: &str) -> &Self::Output {
        self.0
            .get(feature)
            .unwrap_or_else(|| panic!("feature {feature:?} not found"))
    }
}

/// The transitive result of enabling one feature.
#[derive(Debug, Clone)]
pub struct FeatureClosure<'a> {
    /// Features reachable from the requested feature (always includes `""`).
    pub features: BTreeSet<String>,
    /// Activated dependencies, sorted and deduplicated.
    pub dependencies: Vec<ResolvedDependency<'a>>,
}

/// A dependency activated by a feature closure.
#[derive(Debug, Clone)]
pub struct ResolvedDependency<'a> {
    /// The declaration that was activated.
    pub dependency: &'a Dependency,
    /// Feature requested from the dependency, if the activation named one.
    pub feature: Option<String>,
}

/// Parse the feature table into a graph.
pub(crate) fn parse(
    features: &BTreeMap<String, Vec<String>>,
    dependencies: &Dependencies,
) -> FeatureGraph {
    let mut graph: BTreeMap<String, FeatureNode> = features
        .iter()
        .map(|(feature, values)| (feature.clone(), node(values)))
        .collect();

    graph.insert(FeatureGraph::BASE.to_owned(), base_node(dependencies));

    // Every feature implicitly depends on the base library unless it already
    // reaches it through other features.
    for (feature, node) in &mut graph {
        if !feature.is_empty() && node.features.is_empty() {
            node.features.insert(FeatureGraph::BASE.to_owned());
        }
    }

    // The implicit `default` feature.
    graph
        .entry(FeatureGraph::DEFAULT.to_owned())
        .or_insert_with(|| FeatureNode {
            features: BTreeSet::from([FeatureGraph::BASE.to_owned()]),
            dependencies: Vec::new(),
        });

    graph.into()
}

fn node(values: &[String]) -> FeatureNode {
    let mut node = FeatureNode::default();
    for value in values {
        match parse_value(value) {
            ParsedValue::Feature => {
                node.features.insert(value.clone());
            }
            ParsedValue::Dependency(activation) => {
                node.dependencies.push(activation);
            }
        }
    }
    node.dependencies.sort();
    node.dependencies.dedup();
    node
}

fn base_node(dependencies: &Dependencies) -> FeatureNode {
    let mut activations: Vec<_> = dependencies
        .base()
        .map(|dependency| DependencyActivation::Dependency {
            toml_name: dependency.toml_name.clone(),
        })
        .collect();
    activations.sort();
    activations.dedup();

    FeatureNode {
        features: BTreeSet::new(),
        dependencies: activations,
    }
}

enum ParsedValue {
    Feature,
    Dependency(DependencyActivation),
}

fn parse_value(value: &str) -> ParsedValue {
    if let Some(toml_name) = value.strip_prefix("dep:") {
        return ParsedValue::Dependency(DependencyActivation::Dependency {
            toml_name: toml_name.to_owned(),
        });
    }

    let Some((toml_name, feature)) = value.split_once('/') else {
        return ParsedValue::Feature;
    };

    let activation = match toml_name.strip_suffix('?') {
        Some(toml_name) => DependencyActivation::WeakFeature {
            toml_name: toml_name.to_owned(),
            feature: feature.to_owned(),
        },
        None => DependencyActivation::Feature {
            toml_name: toml_name.to_owned(),
            feature: feature.to_owned(),
        },
    };
    ParsedValue::Dependency(activation)
}

fn resolve_activation<'a>(
    activation: &DependencyActivation,
    dependencies: &'a Dependencies,
) -> Vec<ResolvedDependency<'a>> {
    let feature = activation.feature().map(str::to_owned);
    dependencies
        .by_toml_name(activation.toml_name())
        .map(|dependency| ResolvedDependency {
            dependency,
            feature: feature.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use semver::VersionReq;

    use super::*;
    use crate::cargo::metadata::dependency::{Dependency, DependencyKind};

    fn dependency(toml_name: &str, optional: bool, enabled: bool) -> Dependency {
        Dependency {
            toml_name: toml_name.to_owned(),
            package_name: toml_name.to_owned(),
            requirement: VersionReq::STAR,
            lower_bound: None,
            rpm_range: None,
            kind: DependencyKind::Normal,
            optional,
            default_features: true,
            features: Vec::new(),
            platform: None,
            enabled,
            source: None,
            path: None,
        }
    }

    fn build_dependencies(dependencies: Vec<Dependency>) -> Dependencies {
        dependencies.into_iter().collect()
    }

    fn features(entries: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
        entries
            .iter()
            .map(|(feature, values)| {
                (
                    (*feature).to_owned(),
                    values.iter().map(|value| (*value).to_owned()).collect(),
                )
            })
            .collect()
    }

    fn activation_names(node: &FeatureNode) -> Vec<&str> {
        node.dependencies
            .iter()
            .map(DependencyActivation::toml_name)
            .collect()
    }

    #[test]
    fn base_node_contains_only_base_dependencies() {
        let graph = parse(
            &features(&[]),
            &build_dependencies(vec![
                dependency("plain", false, true),
                dependency("optional", true, true),
                dependency("windows-only", false, false),
            ]),
        );

        assert_eq!(activation_names(&graph[FeatureGraph::BASE]), vec!["plain"]);
    }

    #[test]
    fn base_and_default_nodes_are_synthesized() {
        let graph = parse(
            &features(&[]),
            &build_dependencies(vec![dependency("plain", false, true)]),
        );

        assert!(graph.iter().any(|(name, _)| name == FeatureGraph::BASE));
        assert!(graph.iter().any(|(name, _)| name == FeatureGraph::DEFAULT));
        assert_eq!(
            graph[FeatureGraph::DEFAULT].features,
            BTreeSet::from([FeatureGraph::BASE.to_owned()])
        );
    }

    #[test]
    fn all_feature_value_forms_are_parsed() {
        let graph = parse(
            &features(&[("full", &["dep:opt", "plain/derive", "opt?/extra", "other"])]),
            &build_dependencies(vec![
                dependency("opt", true, true),
                dependency("plain", false, true),
                dependency("other", false, true),
            ]),
        );

        let node = &graph["full"];
        assert!(node.features.contains("other"));
        assert_eq!(
            node.dependencies,
            vec![
                DependencyActivation::Dependency {
                    toml_name: "opt".to_owned(),
                },
                DependencyActivation::Feature {
                    toml_name: "plain".to_owned(),
                    feature: "derive".to_owned(),
                },
                DependencyActivation::WeakFeature {
                    toml_name: "opt".to_owned(),
                    feature: "extra".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn empty_features_implicitly_depend_on_base() {
        let graph = parse(
            &features(&[("empty", &[]), ("default", &[])]),
            &build_dependencies(vec![dependency("plain", false, true)]),
        );

        for feature in ["empty", FeatureGraph::DEFAULT] {
            assert_eq!(
                graph[feature].features,
                BTreeSet::from([FeatureGraph::BASE.to_owned()]),
                "feature {feature:?} should depend on base"
            );
        }
    }

    #[test]
    fn closure_includes_base_and_follows_feature_edges() {
        let dependencies = build_dependencies(vec![
            dependency("plain", false, true),
            dependency("opt", true, true),
        ]);
        let graph = parse(
            &features(&[("default", &["extra"]), ("extra", &["dep:opt"])]),
            &dependencies,
        );

        let closure = graph.closure("default", &dependencies).unwrap();
        assert_eq!(
            closure.features,
            BTreeSet::from([String::new(), "default".to_owned(), "extra".to_owned(),])
        );
        let names: Vec<&str> = closure
            .dependencies
            .iter()
            .map(|resolved| resolved.dependency.toml_name.as_str())
            .collect();
        assert_eq!(names, vec!["opt", "plain"]);
    }

    #[test]
    fn weak_activation_only_applies_when_dependency_is_active() {
        let dependencies = build_dependencies(vec![
            dependency("plain", false, true),
            dependency("opt", true, true),
        ]);
        let graph = parse(&features(&[("weak", &["opt?/extra"])]), &dependencies);
        let closure = graph.closure("weak", &dependencies).unwrap();
        assert_eq!(
            closure
                .dependencies
                .iter()
                .map(|resolved| resolved.dependency.toml_name.as_str())
                .collect::<Vec<_>>(),
            vec!["plain"]
        );

        let dependencies = build_dependencies(vec![
            dependency("plain", false, true),
            dependency("opt", true, true),
        ]);
        let graph = parse(
            &features(&[("weak", &["dep:opt", "opt?/extra"])]),
            &dependencies,
        );
        let closure = graph.closure("weak", &dependencies).unwrap();
        let opt: Vec<Option<&str>> = closure
            .dependencies
            .iter()
            .filter(|resolved| resolved.dependency.toml_name == "opt")
            .map(|resolved| resolved.feature.as_deref())
            .collect();
        assert_eq!(opt, vec![None, Some("extra")]);
    }

    #[test]
    fn feature_cycles_terminate() {
        let dependencies = build_dependencies(vec![dependency("plain", false, true)]);
        let graph = parse(&features(&[("a", &["b"]), ("b", &["a"])]), &dependencies);

        let closure = graph.closure("a", &dependencies).unwrap();
        assert_eq!(
            closure.features,
            BTreeSet::from([String::new(), "a".to_owned(), "b".to_owned()])
        );
    }
}
