//! Parsing of package-level fields.

use std::path::PathBuf;

use cargo_metadata::{Package, Source};

use super::{
    dependency::{self, Dependencies, Dependency},
    feature::{self, FeatureClosure, FeatureGraph},
    platform::TargetProfile,
    target::{self, Target, TargetKind},
};

/// Rust edition declared by a package.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Edition {
    /// Edition 2015.
    Rust2015,
    /// Edition 2018.
    Rust2018,
    /// Edition 2021.
    Rust2021,
    /// Edition 2024.
    Rust2024,
    /// An edition this parser does not know about.
    Other(String),
}

impl Edition {
    /// The edition as written in `Cargo.toml`.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Rust2015 => "2015",
            Self::Rust2018 => "2018",
            Self::Rust2021 => "2021",
            Self::Rust2024 => "2024",
            Self::Other(edition) => edition,
        }
    }
}

impl std::fmt::Display for Edition {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl From<&str> for Edition {
    fn from(edition: &str) -> Self {
        match edition {
            "2015" => Self::Rust2015,
            "2018" => Self::Rust2018,
            "2021" => Self::Rust2021,
            "2024" => Self::Rust2024,
            other => Self::Other(other.to_owned()),
        }
    }
}

/// Where a package was loaded from, as reported by cargo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// The crates.io registry.
    CratesIo,
    /// Another registry, as reported by cargo (`registry+...`).
    Registry(String),
    /// A local path.
    Path,
    /// A git source, as reported by cargo (`git+...`).
    Git(String),
    /// Any other source representation.
    Other(String),
}

/// Parsed metadata of one workspace package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageMetadata {
    /// Package name as published.
    pub name: String,
    /// Full SemVer version, including pre-release and build metadata.
    pub version: semver::Version,
    /// Rust edition.
    pub edition: Edition,
    /// Minimum supported Rust version, when declared.
    pub rust_version: Option<semver::Version>,
    /// Where the package was loaded from.
    pub origin: Origin,
    /// Manifest path as reported by cargo.
    pub manifest_path: PathBuf,

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
    /// `documentation` URL.
    pub documentation: Option<String>,
    /// `package.links`, when the package links a native library.
    pub links: Option<String>,

    /// Build targets, sorted by name.
    pub targets: Vec<Target>,
    /// Dependency declarations, sorted deterministically.
    pub dependencies: Dependencies,
    /// Feature graph, always containing the base and default nodes.
    pub features: FeatureGraph,
}

impl PackageMetadata {
    /// Parse one cargo package.
    pub(crate) fn parse(package: &Package) -> Self {
        let dependencies = dependency::parse(package, &TargetProfile::default());

        Self {
            name: package.name.to_string(),
            version: package.version.clone(),
            edition: Edition::from(package.edition.as_str()),
            rust_version: package.rust_version.clone(),
            origin: origin(package.source.as_ref()),
            manifest_path: package.manifest_path.as_std_path().to_path_buf(),

            description: trimmed(package.description.as_deref()),
            license: trimmed(package.license.as_deref()),
            license_file: package
                .license_file
                .as_ref()
                .map(|path| path.as_std_path().to_path_buf()),
            readme: package
                .readme
                .as_ref()
                .map(|path| path.as_std_path().to_path_buf()),
            homepage: trimmed(package.homepage.as_deref()),
            repository: trimmed(package.repository.as_deref()),
            documentation: trimmed(package.documentation.as_deref()),
            links: trimmed(package.links.as_deref()),

            targets: target::parse(package),
            features: feature::parse(&package.features, &dependencies),
            dependencies,
        }
    }

    /// Whether the package exposes a library target.
    pub fn is_lib(&self) -> bool {
        self.targets.iter().any(Target::is_library)
    }

    /// Whether the package exposes a procedural macro target.
    pub fn is_proc_macro(&self) -> bool {
        self.targets
            .iter()
            .any(|target| target.kinds.contains(&TargetKind::ProcMacro))
    }

    /// Binary target names, sorted and deduplicated.
    pub fn bin_targets(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .targets
            .iter()
            .filter(|target| target.kinds.contains(&TargetKind::Bin))
            .map(|target| target.name.as_str())
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// Dependencies that belong to the base feature node.
    pub fn base_dependencies(&self) -> impl Iterator<Item = &Dependency> {
        self.dependencies.base()
    }

    /// Compute the transitive closure of one feature.
    pub fn feature_closure(&self, feature: &str) -> Option<FeatureClosure<'_>> {
        self.features.closure(feature, &self.dependencies)
    }
}

fn trimmed(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn origin(source: Option<&Source>) -> Origin {
    match source {
        None => Origin::Path,
        Some(source) if source.is_crates_io() => Origin::CratesIo,
        Some(source) => {
            let repr = source.repr.clone();
            if repr.starts_with("registry+") {
                Origin::Registry(repr)
            } else if repr.starts_with("git+") {
                Origin::Git(repr)
            } else {
                Origin::Other(repr)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(repr: &str) -> Source {
        Source {
            repr: repr.to_owned(),
        }
    }

    #[test]
    fn editions_round_trip() {
        assert_eq!(Edition::from("2021"), Edition::Rust2021);
        assert_eq!(Edition::Rust2021.to_string(), "2021");
        assert_eq!(Edition::from("2030"), Edition::Other("2030".to_owned()));
    }

    #[test]
    fn crates_io_is_recognized() {
        assert_eq!(
            origin(Some(&source(
                "registry+https://github.com/rust-lang/crates.io-index"
            ))),
            Origin::CratesIo
        );
    }

    #[test]
    fn other_origins_keep_their_representation() {
        assert_eq!(origin(None), Origin::Path);
        assert_eq!(
            origin(Some(&source("registry+https://example.com/index"))),
            Origin::Registry("registry+https://example.com/index".to_owned())
        );
        assert_eq!(
            origin(Some(&source("git+https://example.com/repo#abc"))),
            Origin::Git("git+https://example.com/repo#abc".to_owned())
        );
        assert_eq!(
            origin(Some(&source("sparse+https://example.com/index"))),
            Origin::Other("sparse+https://example.com/index".to_owned())
        );
    }

    #[test]
    fn blank_strings_become_none() {
        assert_eq!(trimmed(None), None);
        assert_eq!(trimmed(Some("  ")), None);
        assert_eq!(trimmed(Some(" MIT ")), Some("MIT".to_owned()));
    }
}
