//! The manifest: pinst's single source of truth for tools and configs.
//!
//! See `manifest.toml` for the authored file and `pinst schema manifest` for
//! the JSON Schema derived from these same types.

use std::collections::BTreeSet;
use std::path::PathBuf;

use color_eyre::eyre::{Context, Result, bail};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

const EMBEDDED_MANIFEST: &str = include_str!("../../manifest.toml");

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Manifest {
    pub meta: Meta,
    #[serde(default, rename = "tool")]
    pub tools: Vec<Tool>,
    #[serde(default, rename = "config")]
    pub configs: Vec<ConfigPackage>,
    #[serde(default)]
    pub profile: std::collections::BTreeMap<String, Profile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Meta {
    pub schema_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Tool {
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Tools that must be installed before this one. pinst topologically
    /// orders installs from these edges.
    #[serde(default)]
    pub requires: Vec<String>,
    pub detect: Detect,
    pub install: Install,
    /// Overrides the upgrade check derived from `install`.
    #[serde(default)]
    pub upgrade: Option<UpgradeSpec>,
    #[serde(default)]
    pub post_install: Vec<PostStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Detect {
    /// Run with `sh -c`; exit status 0 means installed.
    pub command: String,
    /// PATH binary name, when the tool has one.
    #[serde(default)]
    pub bin: Option<String>,
    #[serde(default)]
    pub version_cmd: Option<String>,
    /// Capture group 1 is the version. Defaults to a generic semver pattern.
    #[serde(default)]
    pub version_regex: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PostStep {
    #[serde(default)]
    pub description: String,
    pub command: String,
    /// When this `sh -c` check succeeds, the step is already done and is
    /// skipped — this is what keeps post-install steps idempotent.
    #[serde(default)]
    pub skip_if: Option<String>,
    /// Requires `--yes` (or an interactive confirmation) to run.
    #[serde(default)]
    pub confirm: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum Install {
    Apt {
        packages: Vec<String>,
    },
    Cargo {
        crate_name: String,
    },
    /// `curl -fsSL <url> | <shell> -s -- <args>`
    CurlScript {
        url: String,
        #[serde(default = "default_shell")]
        shell: String,
        #[serde(default)]
        args: Vec<String>,
        /// Enables GitHub-release upgrade checks for a curl-installed tool.
        #[serde(default)]
        github_repo: Option<String>,
    },
    /// Escape hatch for installers whose documented invocation does not fit
    /// the piped-curl shape. Run verbatim with `sh -c`.
    Shell {
        command: String,
    },
    GithubRelease {
        repo: String,
        /// Asset file name in the release (a `.tar.gz` is extracted).
        asset: String,
        /// Extraction destination, e.g. `/opt`.
        dest: String,
        #[serde(default)]
        confirm: bool,
    },
    Nvm {
        #[serde(default = "default_nvm_version")]
        version: String,
    },
    GitClone {
        url: String,
        dest: String,
        #[serde(default)]
        depth: Option<u32>,
    },
    /// No automated install path; doctor reports it with the note.
    Manual {
        note: String,
    },
}

fn default_shell() -> String {
    "sh".to_string()
}

fn default_nvm_version() -> String {
    "--lts".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum UpgradeSpec {
    Apt {
        package: String,
    },
    Cargo {
        crate_name: String,
    },
    GithubRelease {
        repo: String,
    },
    Nvm {},
    /// Explicitly opt out of upgrade checking (e.g. rustup self-manages).
    None {},
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConfigPackage {
    pub name: String,
    /// Subdirectory of `configs/` whose tree mirrors `$HOME`.
    pub source: String,
    #[serde(default)]
    pub summary: String,
    /// Paths within the package (relative, as they appear under `$HOME`)
    /// that get `${VAR}` substitution applied.
    #[serde(default)]
    pub templates: Vec<String>,
    #[serde(default)]
    pub requires_tool: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Profile {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl Tool {
    /// The upgrade check to run: the explicit override if present, otherwise
    /// derived from how the tool is installed.
    pub fn upgrade_spec(&self) -> UpgradeSpec {
        if let Some(explicit) = &self.upgrade {
            return explicit.clone();
        }
        match &self.install {
            Install::Apt { packages } => UpgradeSpec::Apt {
                package: packages
                    .first()
                    .cloned()
                    .unwrap_or_else(|| self.name.clone()),
            },
            Install::Cargo { crate_name } => UpgradeSpec::Cargo {
                crate_name: crate_name.clone(),
            },
            Install::CurlScript { github_repo, .. } => match github_repo {
                Some(repo) => UpgradeSpec::GithubRelease { repo: repo.clone() },
                None => UpgradeSpec::None {},
            },
            Install::GithubRelease { repo, .. } => {
                UpgradeSpec::GithubRelease { repo: repo.clone() }
            }
            Install::Nvm { .. } => UpgradeSpec::Nvm {},
            Install::Shell { .. } | Install::GitClone { .. } | Install::Manual { .. } => {
                UpgradeSpec::None {}
            }
        }
    }

    /// True when the install method offers no verifiable version pinning, so
    /// doctor reports it as unpinnable rather than implying a guarantee.
    pub fn is_unpinnable(&self) -> bool {
        matches!(
            self.install,
            Install::CurlScript { .. } | Install::Shell { .. }
        )
    }

    pub fn method_name(&self) -> &'static str {
        match self.install {
            Install::Apt { .. } => "apt",
            Install::Cargo { .. } => "cargo",
            Install::CurlScript { .. } => "curl_script",
            Install::Shell { .. } => "shell",
            Install::GithubRelease { .. } => "github_release",
            Install::Nvm { .. } => "nvm",
            Install::GitClone { .. } => "git_clone",
            Install::Manual { .. } => "manual",
        }
    }
}

impl Manifest {
    pub fn tool(&self, name: &str) -> Option<&Tool> {
        self.tools.iter().find(|t| t.name == name)
    }

    /// Structural checks the type system cannot express. Run on every load so
    /// a hand-edited or agent-authored manifest fails fast and specifically.
    pub fn validate(&self) -> Result<()> {
        if self.meta.schema_version != SCHEMA_VERSION {
            bail!(
                "manifest schema_version {} is not supported (this pinst speaks {})",
                self.meta.schema_version,
                SCHEMA_VERSION
            );
        }

        let mut seen = BTreeSet::new();
        for tool in &self.tools {
            if tool.name.trim().is_empty() {
                bail!("a tool entry has an empty name");
            }
            if !seen.insert(tool.name.as_str()) {
                bail!("duplicate tool '{}'", tool.name);
            }
        }

        for tool in &self.tools {
            for dep in &tool.requires {
                if !seen.contains(dep.as_str()) {
                    bail!("tool '{}' requires unknown tool '{}'", tool.name, dep);
                }
                if dep == &tool.name {
                    bail!("tool '{}' requires itself", tool.name);
                }
            }
        }

        let mut config_names = BTreeSet::new();
        for config in &self.configs {
            if !config_names.insert(config.name.as_str()) {
                bail!("duplicate config package '{}'", config.name);
            }
            if let Some(tool) = &config.requires_tool
                && !seen.contains(tool.as_str())
            {
                bail!("config '{}' requires unknown tool '{}'", config.name, tool);
            }
        }

        let known_tags: BTreeSet<&str> = self
            .tools
            .iter()
            .flat_map(|t| t.tags.iter().map(String::as_str))
            .collect();
        for (name, profile) in &self.profile {
            for tag in &profile.tags {
                if !known_tags.contains(tag.as_str()) {
                    bail!("profile '{name}' selects tag '{tag}' that no tool carries");
                }
            }
        }

        // Ordering is part of the contract, so a cycle is a load-time error
        // rather than something discovered mid-install.
        crate::core::graph::topo_order(&self.tools)?;
        Ok(())
    }
}

/// Where the manifest came from — reported by `pinst list --json` so an agent
/// can tell an override from the built-in copy.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ManifestSource {
    Embedded,
    Path(PathBuf),
}

impl std::fmt::Display for ManifestSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManifestSource::Embedded => write!(f, "<embedded>"),
            ManifestSource::Path(p) => write!(f, "{}", p.display()),
        }
    }
}

pub struct LoadedManifest {
    pub manifest: Manifest,
    pub source: ManifestSource,
}

/// Loads and validates the manifest, preferring an explicit path, then the
/// `PINST_MANIFEST` env var, then `~/.config/pinst/manifest.toml`, then
/// `./manifest.toml`, and finally the copy embedded in the binary — so the
/// binary stays self-contained but is still overridable without a rebuild.
pub fn load(explicit: Option<&PathBuf>) -> Result<LoadedManifest> {
    let source = match explicit {
        Some(path) => {
            if !path.is_file() {
                return Err(crate::core::usage(format!(
                    "manifest not found: {}",
                    path.display()
                )));
            }
            ManifestSource::Path(path.clone())
        }
        None => discover().unwrap_or(ManifestSource::Embedded),
    };

    let raw = match &source {
        ManifestSource::Embedded => EMBEDDED_MANIFEST.to_string(),
        ManifestSource::Path(path) => std::fs::read_to_string(path)
            .with_context(|| format!("reading manifest {}", path.display()))?,
    };

    // A manifest that does not parse or validate is a bad request, not a
    // runtime failure — exit 2 so an agent knows to fix its edit.
    let manifest: Manifest = toml::from_str(&raw)
        .map_err(|err| crate::core::usage(format!("parsing manifest from {source}: {err}")))?;
    manifest
        .validate()
        .map_err(|err| crate::core::usage(format!("invalid manifest {source}: {err:#}")))?;

    Ok(LoadedManifest { manifest, source })
}

fn discover() -> Option<ManifestSource> {
    if let Some(from_env) = std::env::var_os("PINST_MANIFEST") {
        let path = PathBuf::from(from_env);
        if path.is_file() {
            return Some(ManifestSource::Path(path));
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let path = PathBuf::from(home).join(".config/pinst/manifest.toml");
        if path.is_file() {
            return Some(ManifestSource::Path(path));
        }
    }
    let cwd = PathBuf::from("manifest.toml");
    if cwd.is_file() {
        return Some(ManifestSource::Path(cwd));
    }
    None
}

/// The embedded manifest, parsed. Used by tests to exercise the real data.
#[cfg(test)]
pub fn embedded() -> Result<Manifest> {
    let manifest: Manifest =
        toml::from_str(EMBEDDED_MANIFEST).context("parsing embedded manifest")?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Result<Manifest> {
        let m: Manifest = toml::from_str(src)?;
        m.validate()?;
        Ok(m)
    }

    #[test]
    fn embedded_manifest_is_valid() {
        let manifest = embedded().expect("embedded manifest parses");
        manifest.validate().expect("embedded manifest validates");
        assert!(manifest.tools.len() > 10);
        assert!(!manifest.configs.is_empty());
    }

    #[test]
    fn rejects_duplicate_tool() {
        let err = parse(
            r#"
            [meta]
            schema_version = 1
            [[tool]]
            name = "a"
            detect = { command = "true" }
            install = { method = "apt", packages = ["a"] }
            [[tool]]
            name = "a"
            detect = { command = "true" }
            install = { method = "apt", packages = ["a"] }
            "#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("duplicate tool"), "{err}");
    }

    #[test]
    fn rejects_unknown_dependency() {
        let err = parse(
            r#"
            [meta]
            schema_version = 1
            [[tool]]
            name = "a"
            requires = ["ghost"]
            detect = { command = "true" }
            install = { method = "apt", packages = ["a"] }
            "#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("unknown tool 'ghost'"), "{err}");
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let err = parse(
            r#"
            [meta]
            schema_version = 99
            "#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("schema_version"), "{err}");
    }

    #[test]
    fn derives_upgrade_spec_from_install_method() {
        let manifest = embedded().unwrap();
        let zsh = manifest.tool("zsh").unwrap();
        assert!(matches!(zsh.upgrade_spec(), UpgradeSpec::Apt { .. }));

        // curl-script installs only get a GitHub check when a repo is named.
        let claude = manifest.tool("claude").unwrap();
        assert!(matches!(claude.upgrade_spec(), UpgradeSpec::None {}));
        let uv = manifest.tool("uv").unwrap();
        assert!(matches!(
            uv.upgrade_spec(),
            UpgradeSpec::GithubRelease { .. }
        ));

        // An explicit override wins over the derived default.
        let rust = manifest.tool("rust").unwrap();
        assert!(matches!(rust.upgrade_spec(), UpgradeSpec::None {}));
    }
}
