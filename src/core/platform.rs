//! The platform pinst is planning for: a value threaded as a parameter
//! through manifest resolution, planning, probing and doctor — never read
//! ambiently with `cfg!` at the point of use.
//!
//! Keeping it a parameter (ALT-003 in
//! `.ash/plans/260920-qcrrqs-macos-support/README.md`) is what makes
//! `--platform macos` possible: a Linux machine can render and inspect the
//! macOS answer without touching a Mac, which is the only way this plan's
//! later phases get developed and tested before Phase 5.

use std::fmt;
use std::str::FromStr;

use crate::core::manifest::Install;
use crate::core::usage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Platform {
    Linux,
    MacOS,
}

impl Platform {
    /// The platform this binary was compiled for. Read exactly once, at the
    /// bottom of the precedence chain in [`Platform::resolve`] — every other
    /// call site takes a `Platform` value, never `cfg!` directly.
    pub fn host() -> Platform {
        if cfg!(target_os = "macos") {
            Platform::MacOS
        } else {
            Platform::Linux
        }
    }

    /// The manifest key this platform's `[tool.platform.<key>]` overrides
    /// live under.
    pub fn key(self) -> &'static str {
        match self {
            Platform::Linux => "linux",
            Platform::MacOS => "macos",
        }
    }

    /// Resolves the platform to plan for: an explicit `--platform` flag
    /// wins, then `PINST_PLATFORM`, then the platform this binary was built
    /// for. An unrecognized `PINST_PLATFORM` value is a usage error rather
    /// than a silent fall-through to the host — a typo here would otherwise
    /// quietly plan for the wrong machine.
    pub fn resolve(explicit: Option<Platform>) -> color_eyre::eyre::Result<Platform> {
        if let Some(platform) = explicit {
            return Ok(platform);
        }
        match std::env::var("PINST_PLATFORM") {
            Ok(value) => value
                .parse()
                .map_err(|_| usage(format!("unrecognized PINST_PLATFORM '{value}'"))),
            Err(_) => Ok(Platform::host()),
        }
    }

    /// Whether an install method not overridden for this platform can still
    /// be attempted as written. `apt` and `brew` are the only methods that
    /// name a platform — Linux has no `brew`, macOS has no `apt` — and every
    /// other method (`cargo`, `curl_script`, `shell`, `github_release`,
    /// `nvm`, `git_clone`, `manual`) is portable by construction. A method
    /// being portable does not mean every *use* of it is (see `neovim`'s
    /// Linux-only release asset), which is why some tools carry an explicit
    /// override even though their method admits them.
    pub fn admits(self, install: &Install) -> bool {
        !matches!(
            (self, install),
            (Platform::MacOS, Install::Apt { .. }) | (Platform::Linux, Install::Brew { .. })
        )
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

impl FromStr for Platform {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "linux" => Ok(Platform::Linux),
            "macos" => Ok(Platform::MacOS),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_platform_wins_over_everything() {
        let _guard = crate::core::source::test_env_lock();
        unsafe {
            std::env::set_var("PINST_PLATFORM", "macos");
        }
        let resolved = Platform::resolve(Some(Platform::Linux)).unwrap();
        unsafe {
            std::env::remove_var("PINST_PLATFORM");
        }
        assert_eq!(resolved, Platform::Linux);
    }

    #[test]
    fn env_var_wins_over_host_when_no_explicit_flag() {
        let _guard = crate::core::source::test_env_lock();
        unsafe {
            std::env::set_var("PINST_PLATFORM", "macos");
        }
        let resolved = Platform::resolve(None).unwrap();
        unsafe {
            std::env::remove_var("PINST_PLATFORM");
        }
        assert_eq!(resolved, Platform::MacOS);
    }

    #[test]
    fn falls_back_to_host_with_nothing_set() {
        let _guard = crate::core::source::test_env_lock();
        unsafe {
            std::env::remove_var("PINST_PLATFORM");
        }
        let resolved = Platform::resolve(None).unwrap();
        assert_eq!(resolved, Platform::host());
    }

    #[test]
    fn unrecognized_env_var_is_a_usage_error() {
        let _guard = crate::core::source::test_env_lock();
        unsafe {
            std::env::set_var("PINST_PLATFORM", "solaris");
        }
        let err = Platform::resolve(None).unwrap_err();
        unsafe {
            std::env::remove_var("PINST_PLATFORM");
        }
        assert!(err.to_string().contains("PINST_PLATFORM"), "{err}");
    }

    #[test]
    fn apt_is_linux_only_everything_else_is_admitted() {
        let apt = Install::Apt {
            packages: vec!["x".into()],
        };
        assert!(Platform::Linux.admits(&apt));
        assert!(!Platform::MacOS.admits(&apt));

        let cargo = Install::Cargo {
            crate_name: "x".into(),
        };
        assert!(Platform::Linux.admits(&cargo));
        assert!(Platform::MacOS.admits(&cargo));
    }

    #[test]
    fn brew_is_macos_only() {
        let brew = Install::Brew {
            formulae: vec!["x".into()],
            cask: false,
        };
        assert!(!Platform::Linux.admits(&brew));
        assert!(Platform::MacOS.admits(&brew));
    }
}
