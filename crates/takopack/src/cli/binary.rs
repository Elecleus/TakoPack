//! Binary RPM spec framework for remote/workspace application sources.
//!
//! The registry spec in [`super::spec`] installs normalized crate sources.
//! This module instead targets workspace applications: one source RPM builds
//! the workspace and installs executable files from `target/release`.
//!
//! BuildRequires for the full resolved crates.io dependency graph are generated
//! from `cargo metadata`. Git and non-crates.io sources abort generation with a
//! vendoring hint. Project-specific assets, completions, man pages and build
//! flags remain manual.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use anyhow::{Result, anyhow, bail};
use semver::Version;
use takopack_core::util::calculate_compat_version;
use takopack_rust::cargo::source::remote::FromRemoteBuilder;
use takopack_rust::cargo::{DependencySource, Output, OutputPackage, Process};
use tempfile::TempDir;

use super::output::normalize_name;
use super::spec::{package_summary, rpm_version, source_line};

const SPDX_HEADER: &str = "\
# SPDX-FileCopyrightText: (C) 2026 Institute of Software, Chinese Academy of Sciences (ISCAS)
# SPDX-FileCopyrightText: (C) 2026 openRuyi Project Contributors
#
# SPDX-License-Identifier: MulanPSL-2.0
";

/// Load a remote workspace and print a binary RPM spec skeleton.
pub fn run(url: String, checksum: Option<String>, package: Option<String>) -> Result<()> {
    let tempdir = TempDir::with_prefix("takopack-binary-")?;

    let mut builder = FromRemoteBuilder::new(url);
    if let Some(checksum) = checksum {
        builder = builder.checksum(checksum);
    }

    let mut process = Process::from(builder.build(), tempdir.path());
    process.load()?;
    let output = process.output()?;
    let main = select_main(output, package.as_deref())?;
    let requirements = build_requires(output)?;

    print!("{}", render_binary_spec(output, main, &requirements)?);
    Ok(())
}

fn select_main<'a>(output: &'a Output, package: Option<&str>) -> Result<&'a OutputPackage> {
    let main = match package {
        Some(name) => output
            .package(name)
            .ok_or_else(|| anyhow!("package {name:?} is not in the workspace"))?,
        None => output
            .packages
            .iter()
            .find(|package| !package.bins.is_empty())
            .ok_or_else(|| anyhow!("workspace has no binary targets"))?,
    };

    if main.bins.is_empty() {
        bail!("package {:?} has no binary targets", main.name);
    }
    Ok(main)
}

fn workspace_bins(output: &Output) -> Result<Vec<&str>> {
    let mut bins: Vec<&str> = output
        .packages
        .iter()
        .flat_map(|package| package.bins.iter().map(String::as_str))
        .collect();
    bins.sort_unstable();

    if bins.is_empty() {
        bail!("workspace has no binary targets");
    }
    for pair in bins.windows(2) {
        if pair[0] == pair[1] {
            bail!("duplicate binary target {:?}", pair[0]);
        }
    }
    Ok(bins)
}

fn validate_output_sources(output: &Output) -> Result<()> {
    for package in &output.packages {
        let dependencies = package.base_dependencies.iter().chain(
            package
                .features
                .iter()
                .flat_map(|feature| feature.dependencies.iter()),
        );

        for dependency in dependencies {
            match &dependency.source {
                DependencySource::CratesIo | DependencySource::Path => {}
                DependencySource::Git => {
                    bail!(
                        "dependency {:?} comes from git; vendor it before generating the spec",
                        dependency.package_name
                    );
                }
                DependencySource::Registry { url } => {
                    bail!(
                        "dependency {:?} uses non-crates.io registry {url}; vendor it before generating the spec",
                        dependency.package_name
                    );
                }
                DependencySource::Other => {
                    bail!(
                        "dependency {:?} has an unsupported source; vendor it before generating the spec",
                        dependency.package_name
                    );
                }
            }
        }
    }

    Ok(())
}

fn build_requires(output: &Output) -> Result<Vec<String>> {
    validate_output_sources(output)?;
    validate_lock_sources(output)?;

    let mut command = cargo_metadata::MetadataCommand::new();
    command
        .manifest_path(output.workspace_root.join("Cargo.toml"))
        .current_dir(&output.workspace_root);
    let metadata = command.exec().map_err(|error| {
        anyhow!(
            "failed to resolve the full dependency graph; a git or other non-crates.io source may require vendoring: {error}"
        )
    })?;

    requirements_from_packages(metadata.packages.iter().map(|package| {
        (
            package.name.as_ref(),
            &package.version,
            package.source.as_ref().map(|source| source.repr.as_str()),
        )
    }))
}

fn validate_lock_sources(output: &Output) -> Result<()> {
    let lock_path = output.workspace_root.join("Cargo.lock");
    let lock = match std::fs::read_to_string(&lock_path) {
        Ok(lock) => lock,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let lock: toml::Value = toml::from_str(&lock)?;

    let Some(packages) = lock.get("package").and_then(toml::Value::as_array) else {
        return Ok(());
    };

    for package in packages {
        let Some(source) = package.get("source").and_then(toml::Value::as_str) else {
            continue;
        };
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .unwrap_or("<unknown>");

        if source.starts_with("git+") {
            bail!("dependency {name:?} comes from git; vendor it before generating the spec");
        }
        if !source.contains("crates.io") {
            bail!(
                "dependency {name:?} uses unsupported source {source}; vendor it before generating the spec"
            );
        }
    }

    Ok(())
}

fn requirements_from_packages<'a>(
    packages: impl Iterator<Item = (&'a str, &'a Version, Option<&'a str>)>,
) -> Result<Vec<String>> {
    let mut requirements = BTreeSet::new();

    for (name, version, source) in packages {
        let Some(source) = source else {
            continue;
        };

        if !source.contains("crates.io") {
            bail!(
                "dependency {name:?} uses unsupported source {source}; vendor it before generating the spec"
            );
        }

        let capability = format!(
            "crate({}-{})",
            normalize_name(name),
            calculate_compat_version(version)
        );
        requirements.insert(format!("{capability} >= {version}"));
    }

    Ok(requirements.into_iter().collect())
}

fn render_binary_spec(
    output: &Output,
    main: &OutputPackage,
    requirements: &[String],
) -> Result<String> {
    let bins = workspace_bins(output)?;

    let crate_name = normalize_name(&main.name);
    let summary = if main.description.is_some() {
        package_summary(main)
    } else {
        format!("Rust application \"{}\"", main.name)
    };
    let license = main.license.clone().unwrap_or_else(|| "FIXME".to_owned());
    let url = main
        .homepage
        .clone()
        .or_else(|| main.repository.clone())
        .unwrap_or_else(|| "FIXME".to_owned());
    let description = main.description.as_deref().unwrap_or("%{summary}").trim();

    let mut spec = String::new();
    spec.push_str(SPDX_HEADER);
    writeln!(spec).unwrap();
    writeln!(spec, "Name:           {crate_name}").unwrap();
    writeln!(spec, "Version:        {}", rpm_version(&main.version)).unwrap();
    writeln!(spec, "Release:        %autorelease").unwrap();
    writeln!(spec, "Summary:        {summary}").unwrap();
    writeln!(spec, "License:        {license}").unwrap();
    writeln!(spec, "URL:            {url}").unwrap();
    if let Some(repository) = &main.repository {
        let repository = repository
            .strip_prefix("git+")
            .or_else(|| repository.strip_prefix("git:"))
            .unwrap_or(repository);
        writeln!(spec, "VCS:            git:{repository}").unwrap();
    }
    writeln!(spec, "#!RemoteAsset:  sha256:{}", output.source.checksum).unwrap();
    writeln!(spec, "{}", source_line(output)).unwrap();
    writeln!(spec, "BuildSystem:    rust").unwrap();
    writeln!(spec).unwrap();
    writeln!(spec, "BuildRequires:  rust-rpm-macros").unwrap();
    writeln!(spec, "BuildRequires:  rust").unwrap();
    writeln!(spec, "BuildRequires:  cargo").unwrap();
    for requirement in requirements {
        writeln!(spec, "BuildRequires:  {requirement}").unwrap();
    }
    writeln!(spec).unwrap();
    writeln!(spec, "%description").unwrap();
    writeln!(spec, "{description}").unwrap();

    writeln!(spec).unwrap();
    writeln!(spec, "%build -p").unwrap();

    writeln!(spec).unwrap();
    writeln!(spec, "%install").unwrap();
    for bin in &bins {
        writeln!(
            spec,
            "install -Dm0755 target/release/{bin} %{{buildroot}}%{{_bindir}}/{bin}"
        )
        .unwrap();
    }

    writeln!(spec).unwrap();
    writeln!(spec, "%files").unwrap();
    if let Some(readme) = &main.readme {
        writeln!(spec, "%doc {}", readme.display()).unwrap();
    }
    if let Some(license_file) = &main.license_file {
        writeln!(spec, "%license {}", license_file.display()).unwrap();
    }
    for bin in &bins {
        writeln!(spec, "%{{_bindir}}/{bin}").unwrap();
    }

    writeln!(spec).unwrap();
    writeln!(spec, "%changelog").unwrap();
    writeln!(spec, "%autochangelog").unwrap();

    Ok(spec)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use semver::Version;
    use takopack_rust::cargo::source::ArchiveMetadata;
    use takopack_rust::cargo::{
        DependencyKind, DependencySource, OutputDependency, OutputFeature, OutputPackage,
    };

    use super::*;

    fn package(name: &str, bins: &[&str]) -> OutputPackage {
        OutputPackage {
            name: name.to_owned(),
            version: Version::parse("1.2.3").unwrap(),
            rust_version: None,
            description: Some("Blazing fast terminal file manager".to_owned()),
            license: Some("MIT".to_owned()),
            license_file: None,
            readme: Some(PathBuf::from("README.md")),
            homepage: Some("https://example.com/app".to_owned()),
            repository: Some("https://example.com/app.git".to_owned()),
            links: None,
            is_lib: true,
            is_proc_macro: false,
            bins: bins.iter().map(|bin| (*bin).to_owned()).collect(),
            base_dependencies: Vec::new(),
            features: Vec::<OutputFeature>::new(),
        }
    }

    fn dependency(name: &str, source: DependencySource) -> OutputDependency {
        OutputDependency {
            package_name: name.to_owned(),
            features: Vec::new(),
            default_features: false,
            lower_bound: Some(Version::parse("1.0.0").unwrap()),
            rpm_range: None,
            kind: DependencyKind::Normal,
            source,
        }
    }

    fn output() -> Output {
        Output {
            source: ArchiveMetadata {
                path: PathBuf::from("/tmp/source.tar.gz"),
                checksum: "deadbeef".to_owned(),
                url: Some("https://example.com/app-1.2.3.tar.gz".to_owned()),
            },
            workspace_root: PathBuf::from("/tmp/app"),
            packages: vec![package("yazi", &["yazi", "ya"])],
        }
    }

    #[test]
    fn renders_binary_spec_sections() {
        let output = output();
        let spec = render_binary_spec(&output, &output.packages[0], &[]).unwrap();

        assert!(spec.contains("Name:           yazi"));
        assert!(spec.contains("Version:        1.2.3"));
        assert!(spec.contains("BuildSystem:    rust"));
        assert!(spec.contains("Source:         https://example.com/app-1.2.3.tar.gz"));
        assert!(spec.contains("BuildRequires:  rust-rpm-macros"));
        assert!(spec.contains("install -Dm0755 target/release/yazi"));
        assert!(spec.contains("install -Dm0755 target/release/ya"));
        assert!(spec.contains("%{_bindir}/yazi"));
        assert!(spec.contains("%{_bindir}/ya"));
        assert!(
            !spec
                .lines()
                .any(|line| line.starts_with("BuildRequires:") && line.contains("crate("))
        );
        assert!(!spec.contains("cargo/registry"));
        assert!(spec.contains("%changelog"));
    }

    #[test]
    fn generates_crates_io_build_requires() {
        let version = Version::parse("1.0.0").unwrap();
        let requirements = requirements_from_packages(
            [(
                "serde_core",
                &version,
                Some("registry+https://github.com/rust-lang/crates.io-index"),
            )]
            .into_iter(),
        )
        .unwrap();

        assert_eq!(
            requirements,
            vec!["crate(serde-core-1) >= 1.0.0".to_owned()]
        );
    }

    #[test]
    fn rejects_git_and_foreign_registry_dependencies() {
        let mut output = output();
        output.packages[0].base_dependencies = vec![dependency("git-dep", DependencySource::Git)];
        assert!(validate_output_sources(&output).is_err());

        output.packages[0].base_dependencies = vec![dependency(
            "custom-registry-dep",
            DependencySource::Registry {
                url: "https://example.com/index".to_owned(),
            },
        )];
        assert!(validate_output_sources(&output).is_err());
    }

    #[test]
    fn ignores_internal_path_dependencies() {
        let mut output = output();
        output.packages[0].base_dependencies = vec![dependency("internal", DependencySource::Path)];

        let spec = render_binary_spec(&output, &output.packages[0], &[]).unwrap();
        assert!(!spec.contains("crate(internal-1)"));
    }

    #[test]
    fn rejects_git_packages_from_the_lockfile() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.lock"),
            r#"
version = 4

[[package]]
name = "git-dep"
version = "0.1.0"
source = "git+https://example.com/git-dep#abc"
"#,
        )
        .unwrap();

        let output = Output {
            workspace_root: dir.path().to_path_buf(),
            ..output()
        };
        assert!(validate_lock_sources(&output).is_err());
    }

    #[test]
    fn rejects_workspace_without_bins() {
        let output = Output {
            packages: vec![package("library-only", &[])],
            ..output()
        };

        assert!(select_main(&output, None).is_err());
        assert!(render_binary_spec(&output, &output.packages[0], &[]).is_err());
    }

    #[test]
    fn selected_package_controls_spec_identity() {
        let output = Output {
            packages: vec![
                package("first", &["first-bin"]),
                package("second", &["second-bin"]),
            ],
            ..output()
        };
        let selected = select_main(&output, Some("second")).unwrap();
        let spec = render_binary_spec(&output, selected, &[]).unwrap();

        assert!(spec.contains("Name:           second"));
        assert!(spec.contains("target/release/first-bin"));
        assert!(spec.contains("target/release/second-bin"));
    }
}
