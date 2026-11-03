//! Terminal formatting and printing for `takopack_rust::cargo::Output`.
//!
//! [`super::spec`] renders RPM specs. This module renders the curated
//! [`Output`] itself for humans: sections, aligned fields, feature graphs with
//! `->` relations and colored labels.

use std::fmt::Write as _;
use std::io::{self, IsTerminal};

use clap::Args;
use nu_ansi_term::Color;
use semver::Version;
use takopack_core::errors::Result;
use takopack_core::util::calculate_compat_version;
use takopack_rust::cargo::source::crates_io::FromCratesIoBuilder;
use takopack_rust::cargo::{DependencyKind, Output, OutputDependency, OutputPackage, Process};
use tempfile::TempDir;

/// Arguments for `takopack output`.
#[derive(Debug, Clone, Args)]
pub struct OutputArgs {
    /// Name of the crate on crates.io.
    #[arg(value_name = "CRATE")]
    pub crate_name: String,
    /// Version to load; defaults to the latest stable release.
    #[arg(value_name = "VERSION")]
    pub version: Option<String>,
}

/// Resolve a crate, load its metadata and print the formatted `Output`.
pub fn run(args: OutputArgs) -> Result<()> {
    let tempdir = TempDir::with_prefix("takopack-output-")?;

    let mut builder = FromCratesIoBuilder::new(args.crate_name);
    if let Some(version) = args.version {
        builder.version(version);
    }
    let source = builder.resolve()?;

    let mut process = Process::from(source, tempdir.path());
    process.load()?;
    print(process.output()?);
    Ok(())
}

fn print(output: &Output) {
    let color = io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    print!("{}", render(output, color));
}

fn render(output: &Output, color: bool) -> String {
    let mut out = String::new();

    writeln!(out, "{}", bold("TakoPack Output", Color::Cyan, color)).unwrap();
    writeln!(out, "{}", paint(&"═".repeat(56), Color::DarkGray, color)).unwrap();
    writeln!(out).unwrap();

    writeln!(out, "{}", bold("SOURCE", Color::Cyan, color)).unwrap();
    source_field(&mut out, color, "archive", &output.source.id);
    source_field(&mut out, color, "sha256", &output.source.checksum);
    source_field(&mut out, color, "url", &output.source.url);
    source_field(
        &mut out,
        color,
        "root",
        &output.workspace_root.display().to_string(),
    );
    writeln!(out).unwrap();

    writeln!(
        out,
        "{}  {}",
        bold("PACKAGES", Color::Cyan, color),
        paint(&output.packages.len().to_string(), Color::Yellow, color)
    )
    .unwrap();

    for package in &output.packages {
        writeln!(out).unwrap();
        render_package(&mut out, color, package);
    }

    writeln!(out).unwrap();
    writeln!(
        out,
        "{}",
        paint(
            "legend  (feature) = Cargo feature   -> = relationship   pkg-1/feat = RPM capability",
            Color::DarkGray,
            color
        )
    )
    .unwrap();

    out
}

fn render_package(out: &mut String, color: bool, package: &OutputPackage) {
    let mut header = format!(
        "  {} {}",
        bold(&package.name, Color::Cyan, color),
        paint(&package.version.to_string(), Color::Yellow, color)
    );
    let badges = badges(color, package);
    if !badges.is_empty() {
        header.push_str("  ");
        header.push_str(&badges);
    }
    writeln!(out, "{header}").unwrap();

    if let Some(description) = &package.description {
        writeln!(
            out,
            "    {}",
            paint(&description.replace('\n', " "), Color::DarkGray, color)
        )
        .unwrap();
    }

    if let Some(rust_version) = &package.rust_version {
        package_field(out, color, "rust-version", &rust_version.to_string());
    }
    match &package.license {
        Some(license) => package_field(out, color, "license", license),
        None => package_field(out, color, "license", &missing(color)),
    }
    if let Some(homepage) = &package.homepage {
        package_field(out, color, "homepage", homepage);
    }
    if let Some(repository) = &package.repository {
        package_field(out, color, "repository", repository);
    }
    if let Some(readme) = &package.readme {
        package_field(out, color, "readme", &readme.display().to_string());
    }
    if let Some(license_file) = &package.license_file {
        package_field(
            out,
            color,
            "license-file",
            &license_file.display().to_string(),
        );
    }
    if let Some(links) = &package.links {
        package_field(out, color, "links", links);
    }
    let bins = if package.bins.is_empty() {
        missing(color)
    } else {
        paint(&package.bins.join(", "), Color::Green, color)
    };
    package_field(out, color, "binaries", &bins);

    writeln!(out).unwrap();
    writeln!(out, "    {}", bold("GRAPH", Color::Cyan, color)).unwrap();
    render_node(out, color, "base", &[], &package.base_dependencies);
    for feature in &package.features {
        let requires: Vec<String> = feature
            .features
            .iter()
            .filter(|name| !name.is_empty() && name.as_str() != feature.name)
            .cloned()
            .collect();
        render_node(out, color, &feature.name, &requires, &feature.dependencies);
    }
    if !package.features.is_empty() {
        writeln!(
            out,
            "      {}",
            paint("every feature also requires (base)", Color::DarkGray, color)
        )
        .unwrap();
    }
}

fn render_node(
    out: &mut String,
    color: bool,
    name: &str,
    requires: &[String],
    dependencies: &[OutputDependency],
) {
    let mut header = format!("      {}", feature_label(color, name));
    if !requires.is_empty() {
        header.push(' ');
        header.push_str(&paint("->", Color::Blue, color));
        header.push(' ');
        let requires: Vec<String> = requires
            .iter()
            .map(|required| feature_label(color, required))
            .collect();
        header.push_str(&requires.join(&paint(", ", Color::DarkGray, color)));
    }
    writeln!(out, "{header}").unwrap();

    if requires.is_empty() && dependencies.is_empty() && name != "base" {
        writeln!(
            out,
            "        {}",
            paint("(no extra dependencies)", Color::DarkGray, color)
        )
        .unwrap();
    }

    for dependency in dependencies {
        for capability in dependency_requirements(dependency) {
            writeln!(
                out,
                "        {} {}",
                paint("->", Color::Blue, color),
                paint(&capability, Color::Green, color)
            )
            .unwrap();
        }
    }
}

fn source_field(out: &mut String, color: bool, label: &str, value: &str) {
    writeln!(
        out,
        "  {}  {}",
        paint(&format!("{label:<10}"), Color::DarkGray, color),
        value
    )
    .unwrap();
}

fn package_field(out: &mut String, color: bool, label: &str, value: &str) {
    writeln!(
        out,
        "    {}  {}",
        paint(&format!("{label:<12}"), Color::DarkGray, color),
        value
    )
    .unwrap();
}

fn badges(color: bool, package: &OutputPackage) -> String {
    let mut badges = Vec::new();
    if package.is_lib {
        badges.push("[lib]".to_owned());
    }
    if package.is_proc_macro {
        badges.push("[proc-macro]".to_owned());
    }
    if !package.bins.is_empty() {
        badges.push(format!("[bins: {}]", package.bins.join(", ")));
    }
    if badges.is_empty() {
        return paint("[no targets]", Color::DarkGray, color);
    }
    paint(&badges.join(" "), Color::Green, color)
}

fn feature_label(color: bool, name: &str) -> String {
    let label = format!("({name})");
    match name {
        "base" => bold(&label, Color::Yellow, color),
        "default" => bold(&label, Color::Purple, color),
        _ => paint(&label, Color::Purple, color),
    }
}

fn missing(color: bool) -> String {
    paint("(none)", Color::DarkGray, color)
}

fn paint(text: &str, color: Color, enabled: bool) -> String {
    if enabled {
        color.paint(text).to_string()
    } else {
        text.to_owned()
    }
}

fn bold(text: &str, color: Color, enabled: bool) -> String {
    if enabled {
        color.bold().paint(text).to_string()
    } else {
        text.to_owned()
    }
}

// ---------------------------------------------------------------------------
// Shared Output -> RPM capability formatting (also used by `spec.rs`).
// ---------------------------------------------------------------------------

pub(crate) fn normalize_name(name: &str) -> String {
    name.replace('_', "-").to_lowercase()
}

pub(crate) fn normalize_feature_name(feature: &str) -> String {
    feature
        .replace('_', "-")
        .to_lowercase()
        .trim_start_matches('-')
        .to_owned()
}

/// Capabilities required by one dependency, without the version constraint.
pub(crate) fn dependency_capabilities(dependency: &OutputDependency) -> Vec<String> {
    let base = capability_name(dependency);
    dependency_features(dependency)
        .into_iter()
        .map(|feature| match feature {
            Some(feature) => format!("{base}/{feature}"),
            None => base.clone(),
        })
        .collect()
}

/// Capabilities required by one dependency, including the version constraint.
fn dependency_requirements(dependency: &OutputDependency) -> Vec<String> {
    let version = dependency
        .lower_bound
        .as_ref()
        .map(|version| format!(" >= {version}"))
        .unwrap_or_default();

    dependency_capabilities(dependency)
        .into_iter()
        .map(|capability| format!("{capability}{version}"))
        .collect()
}

fn capability_name(dependency: &OutputDependency) -> String {
    let base = normalize_name(&dependency.package_name);
    match dependency.lower_bound.as_ref() {
        Some(version) => format!("{base}-{}", capability_suffix(version)),
        None => base,
    }
}

fn capability_suffix(version: &Version) -> String {
    if version.pre.is_empty() {
        calculate_compat_version(version)
    } else {
        format!(
            "{}.{}.{}-{}",
            version.major, version.minor, version.patch, version.pre
        )
    }
}

fn dependency_features(dependency: &OutputDependency) -> Vec<Option<String>> {
    let mut features = Vec::new();

    if dependency.kind == DependencyKind::Build {
        features.push(None);
        features.extend(
            dependency
                .features
                .iter()
                .map(|feature| Some(normalize_feature_name(feature))),
        );
    } else {
        if dependency.default_features {
            features.push(Some("default".to_owned()));
        }
        features.extend(
            dependency
                .features
                .iter()
                .map(|feature| Some(normalize_feature_name(feature))),
        );
        if features.is_empty() {
            features.push(None);
        }
    }

    features.sort();
    features.dedup();
    features
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use takopack_rust::cargo::source::SourceArchive;
    use takopack_rust::cargo::{DependencySource, OutputFeature};

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

    fn output() -> Output {
        let package = OutputPackage {
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
                    "std",
                    &[""],
                    vec![dependency(
                        "serde_core",
                        &["result", "std"],
                        false,
                        "1.0.229",
                    )],
                ),
            ],
        };

        Output {
            source: SourceArchive {
                id: "serde-1.0.229".to_owned(),
                path: PathBuf::from("/tmp/serde-1.0.229.crate"),
                checksum: "deadbeef".to_owned(),
                url: "https://static.crates.io/crates/serde/serde-1.0.229.crate".to_owned(),
            },
            workspace_root: PathBuf::from("/tmp/serde-1.0.229"),
            packages: vec![package],
        }
    }

    #[test]
    fn renders_sections_and_graph_without_color() {
        let output = output();
        let rendered = render(&output, false);

        assert!(rendered.contains("TakoPack Output"));
        assert!(rendered.contains("SOURCE"));
        assert!(rendered.contains("archive"));
        assert!(rendered.contains("serde-1.0.229"));
        assert!(rendered.contains("serde 1.0.229"));
        assert!(rendered.contains("[lib]"));
        assert!(rendered.contains("(base)"));
        assert!(rendered.contains("-> serde-core-1/result >= 1.0.229"));
        assert!(rendered.contains("(default) -> (std)"));
        assert!(rendered.contains("(derive) -> (serde_derive)"));
        assert!(rendered.contains("every feature also requires (base)"));
        assert!(rendered.contains("-> serde-derive-1/default >= 1.0.0"));
        assert!(!rendered.contains('\u{1b}'));
    }

    #[test]
    fn color_output_contains_ansi_escapes() {
        let rendered = render(&output(), true);
        assert!(rendered.contains('\u{1b}'));
    }
}
