//! Target platform matching used to mark platform-specific dependencies.
//!
//! Cargo reports dependencies for every platform. The parsed model keeps all
//! of them and records [`crate::cargo::metadata::Dependency::enabled`] for the
//! openRuyi target only.
//!
//! The cfg list mirrors `rustc --print cfg --target x86_64-unknown-linux-gnu`
//! for the target itself. Build-profile cfgs (`debug_assertions`, `panic`) are
//! deliberately excluded: they are not properties of the target platform.

use cargo_metadata::cargo_platform::{Cfg, Platform};

/// The target platform a crate is parsed for.
#[derive(Debug, Clone)]
pub(crate) struct TargetProfile {
    triple: String,
    cfgs: Vec<Cfg>,
}

impl Default for TargetProfile {
    fn default() -> Self {
        Self::linux_x86_64()
    }
}

impl TargetProfile {
    fn linux_x86_64() -> Self {
        const TRIPLE: &str = "x86_64-unknown-linux-gnu";
        const CFGS: &[&str] = &[
            "target_abi = \"\"",
            "target_arch = \"x86_64\"",
            "target_endian = \"little\"",
            "target_env = \"gnu\"",
            "target_family = \"unix\"",
            "target_feature = \"fxsr\"",
            "target_feature = \"sse\"",
            "target_feature = \"sse2\"",
            "target_has_atomic = \"8\"",
            "target_has_atomic = \"16\"",
            "target_has_atomic = \"32\"",
            "target_has_atomic = \"64\"",
            "target_has_atomic = \"ptr\"",
            "target_os = \"linux\"",
            "target_pointer_width = \"64\"",
            "target_vendor = \"unknown\"",
            "unix",
        ];

        Self {
            triple: TRIPLE.to_owned(),
            cfgs: CFGS
                .iter()
                .map(|cfg| {
                    cfg.parse()
                        .expect("built-in target cfg should always parse")
                })
                .collect(),
        }
    }

    /// Whether a dependency's platform restriction matches this target.
    ///
    /// `None` (no restriction) always matches.
    pub(crate) fn matches(&self, platform: Option<&Platform>) -> bool {
        platform.is_none_or(|platform| platform.matches(&self.triple, &self.cfgs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn platform(value: &str) -> Platform {
        value.parse().unwrap()
    }

    #[test]
    fn matches_linux_cfgs() {
        let target = TargetProfile::linux_x86_64();
        assert!(target.matches(None));
        assert!(target.matches(Some(&platform("cfg(unix)"))));
        assert!(target.matches(Some(&platform("cfg(target_os = \"linux\")"))));
        assert!(target.matches(Some(&platform("cfg(any(unix, windows))"))));
        assert!(target.matches(Some(&platform(
            "cfg(all(unix, target_pointer_width = \"64\"))"
        ))));
        assert!(target.matches(Some(&platform("x86_64-unknown-linux-gnu"))));
    }

    #[test]
    fn rejects_other_platforms() {
        let target = TargetProfile::linux_x86_64();
        assert!(!target.matches(Some(&platform("cfg(windows)"))));
        assert!(!target.matches(Some(&platform("cfg(target_os = \"macos\")"))));
        assert!(!target.matches(Some(&platform("cfg(target_arch = \"aarch64\")"))));
        assert!(!target.matches(Some(&platform("x86_64-pc-windows-msvc"))));
    }

    #[test]
    fn build_profile_cfgs_are_not_target_facts() {
        let target = TargetProfile::linux_x86_64();
        assert!(!target.matches(Some(&platform("cfg(debug_assertions)"))));
        assert!(!target.matches(Some(&platform("cfg(panic = \"unwind\")"))));
    }
}
