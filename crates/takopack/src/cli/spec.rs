//! First RPM spec integration for the new `cargo::Output` pipeline.
//!
//! The command resolves a crates.io crate, loads it through
//! `takopack_rust::cargo::Process` and renders a spec to stdout. Writing the
//! spec to disk and the remaining RPM packaging work come later.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use clap::Args;
use semver::Version;
use takopack_core::errors::Result;
use takopack_core::util::calculate_compat_version;
use takopack_rust::cargo::source::crates_io::FromCratesIoBuilder;
use takopack_rust::cargo::{Output, OutputDependency, OutputFeature, OutputPackage, Process};

use super::output::{dependency_capabilities, normalize_feature_name, normalize_name};
use tempfile::TempDir;

const SPDX_HEADER: &str = "\
# SPDX-FileCopyrightText: (C) 2026 Institute of Software, Chinese Academy of Sciences (ISCAS)
# SPDX-FileCopyrightText: (C) 2026 openRuyi Project Contributors
#
# SPDX-License-Identifier: MulanPSL-2.0
";

const SOURCE_LINE: &str = "Source:         https://static.crates.io/crates/%{crate_name}/%{full_version}/download#/%{name}-%{version}.tar.gz";
const BASE_REQUIRES: &str = "Requires:       crate(%{pkgname}) = %{version}";
const BASE_PROVIDES: &str = "Provides:       crate(%{pkgname}) = %{version}";
const FILES_SECTION: &str = "\
%files
%{_datadir}/cargo/registry/%{crate_name}-%{version}/";

/// Arguments for `takopack spec`.
#[derive(Debug, Clone, Args)]
pub struct SpecArgs {
    /// Name of the crate on crates.io.
    #[arg(value_name = "CRATE")]
    pub crate_name: String,
    /// Version to render; defaults to the latest stable release.
    #[arg(value_name = "VERSION")]
    pub version: Option<String>,
}

/// Resolve a crate, load its metadata and print the generated spec.
pub fn run(args: SpecArgs) -> Result<()> {
    let tempdir = TempDir::with_prefix("takopack-spec-")?;

    let mut builder = FromCratesIoBuilder::new(args.crate_name);
    if let Some(version) = args.version {
        builder.version(version);
    }
    let source = builder.resolve()?;

    let mut process = Process::from(source, tempdir.path());
    process.load()?;
    let output = process.output()?;
    let package = process.package()?;

    print!("{}", render_spec(output, package));
    Ok(())
}

fn render_spec(output: &Output, package: &OutputPackage) -> String {
    let crate_name = package.name.as_str();
    let normalized_name = normalize_name(crate_name);
    let compat = calculate_compat_version(&package.version);
    let pkgname = format!("{normalized_name}-{compat}");
    let rpm_name = format!("rust-{pkgname}");
    let full_version = package.version.to_string();
    let rpm_version = rpm_version(&package.version);
    let summary = format!("Rust crate \"{crate_name}\"");
    let license = package
        .license
        .clone()
        .unwrap_or_else(|| "FIXME".to_owned());
    let url = package
        .homepage
        .clone()
        .or_else(|| package.repository.clone())
        .unwrap_or_else(|| "FIXME".to_owned());
    let description = format!("Source code for takopackized Rust crate \"{crate_name}\"");
    let feature_summary = package_summary(package);

    let mut spec = String::new();
    spec.push_str(SPDX_HEADER);
    writeln!(spec).unwrap();
    writeln!(spec, "%global crate_name {crate_name}").unwrap();
    writeln!(spec, "%global full_version {full_version}").unwrap();
    writeln!(spec, "%global pkgname {pkgname}").unwrap();
    writeln!(spec).unwrap();
    writeln!(spec, "Name:           {rpm_name}").unwrap();
    writeln!(spec, "Version:        {rpm_version}").unwrap();
    writeln!(spec, "Release:        %autorelease").unwrap();
    writeln!(spec, "Summary:        {summary}").unwrap();
    writeln!(spec, "License:        {license}").unwrap();
    writeln!(spec, "URL:            {url}").unwrap();
    writeln!(spec, "#!RemoteAsset:  sha256:{}", output.source.checksum).unwrap();
    writeln!(spec, "{SOURCE_LINE}").unwrap();
    writeln!(spec, "BuildArch:      noarch").unwrap();
    writeln!(spec, "BuildSystem:    rustcrates").unwrap();
    writeln!(spec).unwrap();
    writeln!(spec, "BuildRequires:  rust-rpm-macros").unwrap();
    writeln!(spec).unwrap();

    for dependency in &package.base_dependencies {
        for line in requirement_lines(dependency) {
            writeln!(spec, "{line}").unwrap();
        }
    }
    writeln!(spec, "{BASE_PROVIDES}").unwrap();
    writeln!(spec).unwrap();
    writeln!(spec, "%description").unwrap();
    writeln!(spec, "{description}").unwrap();

    for feature in merge_features(&package.features) {
        let target = feature.name;
        let target_normalized = normalize_feature_name(target);
        let mut aliases: Vec<&str> = feature.provides.iter().skip(1).copied().collect();
        aliases.sort_by_key(|alias| normalize_feature_name(alias));
        aliases.dedup();

        writeln!(spec).unwrap();
        writeln!(spec, "%package     -n %{{name}}+{target_normalized}").unwrap();
        let more = if aliases.is_empty() {
            String::new()
        } else {
            format!(" and {} more", aliases.len())
        };
        writeln!(
            spec,
            "Summary:        {feature_summary} - feature \"{target}\"{more}"
        )
        .unwrap();
        writeln!(spec, "{BASE_REQUIRES}").unwrap();
        for required in &feature.requires {
            let required = normalize_feature_name(required);
            writeln!(
                spec,
                "Requires:       crate(%{{pkgname}}/{required}) = %{{version}}"
            )
            .unwrap();
        }
        for dependency in feature.dependencies {
            for line in requirement_lines(dependency) {
                writeln!(spec, "{line}").unwrap();
            }
        }
        let provides: BTreeSet<String> = feature
            .provides
            .iter()
            .map(|feature| normalize_feature_name(feature))
            .collect();
        for provide in provides {
            writeln!(
                spec,
                "Provides:       crate(%{{pkgname}}/{provide}) = %{{version}}"
            )
            .unwrap();
        }
        writeln!(spec).unwrap();
        writeln!(spec, "%description -n %{{name}}+{target_normalized}").unwrap();
        writeln!(
            spec,
            "This metapackage enables feature \"{target}\" for the Rust {crate_name} crate, by pulling in any additional dependencies needed by that feature."
        )
        .unwrap();
        if !aliases.is_empty() {
            writeln!(spec).unwrap();
            writeln!(
                spec,
                "Additionally, this package also provides {}.",
                alias_sentence(&aliases)
            )
            .unwrap();
        }
    }

    writeln!(spec).unwrap();
    writeln!(spec, "{FILES_SECTION}").unwrap();
    writeln!(spec).unwrap();
    writeln!(spec, "%changelog").unwrap();
    writeln!(spec, "%autochangelog").unwrap();

    spec
}

/// Derive the feature-subpackage summary prefix from the package description.
///
/// The RPM header keeps the generic `Rust crate "<name>"` summary; feature
/// subpackages use the upstream one-line description, matching the old
/// TakoPack spec output.
fn package_summary(package: &OutputPackage) -> String {
    let Some(description) = package.description.as_deref() else {
        return format!("Rust crate \"{}\"", package.name);
    };

    let mut text = description
        .replace("\n\n", "\r")
        .replace('\n', " ")
        .replace('\r', "\n")
        .trim()
        .to_owned();
    for article in ["a ", "an ", "the "] {
        if text
            .get(..article.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(article))
        {
            text = text[article.len()..].to_owned();
            break;
        }
    }

    let end = text.find('\n').into_iter().chain(text.find(". ")).min();
    if let Some(end) = end {
        text.truncate(end);
    }
    let summary = text.trim_end_matches('.').trim();
    if summary.is_empty() {
        return format!("Rust crate \"{}\"", package.name);
    }

    let mut chars = summary.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => format!("Rust crate \"{}\"", package.name),
    }
}

fn requirement_lines(dependency: &OutputDependency) -> Vec<String> {
    let version = dependency
        .lower_bound
        .as_ref()
        .map(|version| format!(" >= {version}"))
        .unwrap_or_default();
    dependency_capabilities(dependency)
        .into_iter()
        .map(|capability| format!("Requires:       crate({capability}){version}"))
        .collect()
}

fn rpm_version(version: &Version) -> String {
    let mut result = format!("{}.{}.{}", version.major, version.minor, version.patch);
    if !version.pre.is_empty() {
        result.push('-');
        result.push_str(version.pre.as_str());
    }
    result
}

fn alias_sentence(aliases: &[&str]) -> String {
    match aliases {
        [] => unreachable!("alias_sentence is only called with aliases"),
        [alias] => format!("the \"{alias}\" feature"),
        [rest @ .., last] => format!("the \"{}\", and \"{}\" features", rest.join("\", \""), last),
    }
}

struct FeatureSpec<'a> {
    name: &'a str,
    requires: Vec<&'a str>,
    dependencies: &'a [OutputDependency],
    provides: Vec<&'a str>,
    removed: bool,
}

/// Merge pure aliases and features with identical closures.
///
/// This mirrors the old TakoPack behavior: when a feature only forwards to
/// another feature, or two features have exactly the same closure, one package
/// provides both names. The data needed for this decision lives entirely in
/// [`OutputFeature`].
fn merge_features(features: &[OutputFeature]) -> Vec<FeatureSpec<'_>> {
    let mut specs: Vec<FeatureSpec<'_>> = features
        .iter()
        .map(|feature| FeatureSpec {
            name: &feature.name,
            requires: feature
                .features
                .iter()
                .filter(|name| name.as_str() != "" && name.as_str() != feature.name)
                .map(String::as_str)
                .collect(),
            dependencies: &feature.dependencies,
            provides: vec![&feature.name],
            removed: false,
        })
        .collect();

    // A feature that only forwards to one other feature and has the same
    // closure as that feature becomes a `Provides` entry on the target.
    for index in 0..specs.len() {
        if specs[index].removed || specs[index].requires.len() != 1 {
            continue;
        }
        let target_name = specs[index].requires[0];
        let Some(target) = specs
            .iter()
            .position(|spec| !spec.removed && spec.name == target_name)
        else {
            continue;
        };
        if specs[index].dependencies == specs[target].dependencies {
            let alias = specs[index].name;
            specs[target].provides.push(alias);
            specs[index].removed = true;
        }
    }

    // Features with identical closures are represented by one package.
    for left in 0..specs.len() {
        if specs[left].removed {
            continue;
        }
        for right in (left + 1)..specs.len() {
            if specs[right].removed {
                continue;
            }
            if specs[left].requires == specs[right].requires
                && specs[left].dependencies == specs[right].dependencies
            {
                let alias = specs[right].name;
                specs[left].provides.push(alias);
                specs[right].removed = true;
            }
        }
    }

    specs.retain(|spec| !spec.removed);
    specs
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use takopack_rust::cargo::source::SourceArchive;
    use takopack_rust::cargo::{DependencyKind, DependencySource};

    use super::*;

    fn version(value: &str) -> Version {
        Version::parse(value).unwrap()
    }

    fn dependency(
        package_name: &str,
        features: &[&str],
        default_features: bool,
        lower_bound: &str,
    ) -> OutputDependency {
        OutputDependency {
            package_name: package_name.to_owned(),
            features: features
                .iter()
                .map(|feature| (*feature).to_owned())
                .collect(),
            default_features,
            lower_bound: Some(version(lower_bound)),
            rpm_range: None,
            kind: DependencyKind::Normal,
            source: DependencySource::Registry,
        }
    }

    fn feature(
        name: &str,
        features: &[&str],
        dependencies: Vec<OutputDependency>,
    ) -> OutputFeature {
        OutputFeature {
            name: name.to_owned(),
            features: features
                .iter()
                .map(|feature| (*feature).to_owned())
                .collect(),
            dependencies,
        }
    }

    fn package() -> OutputPackage {
        OutputPackage {
            name: "serde".to_owned(),
            version: version("1.0.229"),
            rust_version: Some(version("1.56.0")),
            description: Some("A generic serialization/deserialization framework".to_owned()),
            license: Some("MIT OR Apache-2.0".to_owned()),
            license_file: None,
            readme: Some(PathBuf::from("crates-io.md")),
            homepage: Some("https://serde.rs".to_owned()),
            repository: Some("https://github.com/serde-rs/serde".to_owned()),
            links: None,
            is_lib: true,
            is_proc_macro: false,
            bins: Vec::new(),
            base_dependencies: vec![dependency("serde_core", &["result"], false, "1.0.229")],
            features: vec![
                feature(
                    "alloc",
                    &["", "alloc"],
                    vec![dependency(
                        "serde_core",
                        &["alloc", "result"],
                        false,
                        "1.0.229",
                    )],
                ),
                feature(
                    "default",
                    &["", "std"],
                    vec![dependency(
                        "serde_core",
                        &["result", "std"],
                        false,
                        "1.0.229",
                    )],
                ),
                feature(
                    "derive",
                    &["", "serde_derive"],
                    vec![dependency("serde_derive", &[], true, "1.0.0")],
                ),
                feature(
                    "rc",
                    &["", "rc"],
                    vec![dependency(
                        "serde_core",
                        &["rc", "result"],
                        false,
                        "1.0.229",
                    )],
                ),
                feature(
                    "serde_derive",
                    &[""],
                    vec![dependency("serde_derive", &[], true, "1.0.0")],
                ),
                feature(
                    "std",
                    &[""],
                    vec![dependency(
                        "serde_core",
                        &["result", "std"],
                        false,
                        "1.0.229",
                    )],
                ),
                feature(
                    "unstable",
                    &["", "unstable"],
                    vec![dependency(
                        "serde_core",
                        &["result", "unstable"],
                        false,
                        "1.0.229",
                    )],
                ),
            ],
        }
    }

    fn output() -> Output {
        Output {
            source: SourceArchive {
                id: "serde-1.0.229".to_owned(),
                path: PathBuf::from("/tmp/serde-1.0.229.crate"),
                checksum: "deadbeef".to_owned(),
                url: "https://static.crates.io/crates/serde/serde-1.0.229.crate".to_owned(),
            },
            workspace_root: PathBuf::from("/tmp/serde-1.0.229"),
            packages: vec![package()],
        }
    }

    #[test]
    fn renders_the_expected_shape() {
        let output = output();
        let spec = render_spec(&output, &output.packages[0]);

        assert!(spec.contains("Name:           rust-serde-1"));
        assert!(spec.contains("Version:        1.0.229"));
        assert!(spec.contains("URL:            https://serde.rs"));
        assert!(spec.contains("#!RemoteAsset:  sha256:deadbeef"));
        assert!(spec.contains("BuildRequires:  rust-rpm-macros"));

        assert!(spec.contains("Requires:       crate(serde-core-1/result) >= 1.0.229"));
        assert!(spec.contains("Provides:       crate(%{pkgname}) = %{version}"));

        assert!(spec.contains("%package     -n %{name}+alloc"));
        assert!(spec.contains("Requires:       crate(serde-core-1/alloc) >= 1.0.229"));
        assert!(spec.contains("Requires:       crate(serde-core-1/result) >= 1.0.229"));

        assert!(spec.contains("%package     -n %{name}+serde-derive"));
        assert!(spec.contains("Requires:       crate(serde-derive-1/default) >= 1.0.0"));
        assert!(spec.contains("Provides:       crate(%{pkgname}/derive) = %{version}"));
        assert!(spec.contains("Provides:       crate(%{pkgname}/serde-derive) = %{version}"));

        assert!(spec.contains("%package     -n %{name}+std"));
        assert!(spec.contains("Provides:       crate(%{pkgname}/default) = %{version}"));
        assert!(spec.contains("Provides:       crate(%{pkgname}/std) = %{version}"));

        assert!(!spec.contains("%package     -n %{name}+default"));
        assert!(!spec.contains("%package     -n %{name}+derive"));
        assert!(spec.contains("%files"));
        assert!(spec.contains("%autochangelog"));
    }
}
