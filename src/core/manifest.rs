//! The manifest: pinst's single source of truth for tools and configs.
//!
//! See `manifest.toml` for the authored file and `pinst schema manifest` for
//! the JSON Schema derived from these same types.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use color_eyre::eyre::{Context, Result, bail};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::core::platform::Platform;

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
    /// How to ask the tool to describe itself, when `<bin> --help` is not it.
    /// Run with `sh -c`, like `detect.command`. A tool with neither this nor
    /// `detect.bin` has no help to capture, which `pinst docs` reports rather
    /// than treating as an error.
    #[serde(default)]
    pub help_cmd: Option<String>,
    pub install: Install,
    /// Overrides the upgrade check derived from `install`.
    #[serde(default)]
    pub upgrade: Option<UpgradeSpec>,
    #[serde(default)]
    pub post_install: Vec<PostStep>,
    /// Per-platform overrides, keyed by [`Platform::key`] (`"linux"`,
    /// `"macos"`). A tool with no entry for the platform being resolved
    /// falls through to [`Platform::admits`]; a tool with an entry is
    /// explicitly accounted for on that platform, even if every field in
    /// the entry is `None` and it changes nothing.
    #[serde(default, rename = "platform")]
    pub platform_overrides: BTreeMap<String, PlatformOverride>,
}

/// One platform's differences from a tool's base definition. Every field is
/// optional and substitutes for the matching base field when present; a
/// field left `None` is inherited unchanged. `unsupported` is exclusive with
/// every substituting field — a tool this platform cannot install has
/// nothing else to override (checked in [`Manifest::validate`]).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct PlatformOverride {
    #[serde(default)]
    pub detect: Option<Detect>,
    #[serde(default)]
    pub install: Option<Install>,
    #[serde(default)]
    pub upgrade: Option<UpgradeSpec>,
    #[serde(default)]
    pub requires: Option<Vec<String>>,
    #[serde(default)]
    pub post_install: Option<Vec<PostStep>>,
    /// This platform has no automated path for the tool at all. Doctor
    /// reports it as `tool.unsupported.<name>` at info severity, carrying
    /// this note as the remediation.
    #[serde(default)]
    pub unsupported: Option<String>,
}

/// The fields of a [`Tool`] as they apply on a specific platform, borrowed
/// from either the base definition or its override — nothing is cloned.
#[derive(Debug, Clone, Copy)]
pub struct EffectiveTool<'a> {
    pub detect: &'a Detect,
    pub install: &'a Install,
    pub requires: &'a [String],
    pub post_install: &'a [PostStep],
}

impl EffectiveTool<'_> {
    /// True when this platform's effective install method offers no
    /// verifiable version pinning.
    pub fn is_unpinnable(&self) -> bool {
        install_is_unpinnable(self.install)
    }

    pub fn method_name(&self) -> &'static str {
        install_method_name(self.install)
    }
}

/// The result of resolving a [`Tool`] against a [`Platform`]. `Unsupported`
/// carries an owned string rather than a borrow: the common case (a tool
/// with no override at all, on a platform its method does not admit) has no
/// note written anywhere to borrow from, so one is generated on the spot.
#[derive(Debug, Clone)]
pub enum Resolved<'a> {
    Supported(EffectiveTool<'a>),
    /// The tool cannot be installed on this platform; the note is what
    /// doctor and the CLI report as the reason.
    Unsupported(String),
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
    /// `brew install [--cask] <formulae>`. Homebrew refuses to run as root,
    /// so this method's executor never prefixes `sudo`.
    Brew {
        formulae: Vec<String>,
        #[serde(default)]
        cask: bool,
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
    Brew {
        formula: String,
    },
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

/// Derives the upgrade check for a given install method and tool name —
/// factored out of [`Tool::upgrade_spec`] so a platform override's `install`
/// can go through the same derivation without a `Tool` to hang it off.
fn derive_upgrade_spec(name: &str, install: &Install) -> UpgradeSpec {
    match install {
        Install::Apt { packages } => UpgradeSpec::Apt {
            package: packages
                .first()
                .cloned()
                .unwrap_or_else(|| name.to_string()),
        },
        Install::Cargo { crate_name } => UpgradeSpec::Cargo {
            crate_name: crate_name.clone(),
        },
        Install::CurlScript { github_repo, .. } => match github_repo {
            Some(repo) => UpgradeSpec::GithubRelease { repo: repo.clone() },
            None => UpgradeSpec::None {},
        },
        Install::GithubRelease { repo, .. } => UpgradeSpec::GithubRelease { repo: repo.clone() },
        Install::Nvm { .. } => UpgradeSpec::Nvm {},
        Install::Brew { formulae, .. } => UpgradeSpec::Brew {
            // The formula that names the tool, not necessarily the whole
            // package: a brew install can list more than one formula, but
            // the upgrade check tracks the one the tool is named after.
            formula: formulae
                .first()
                .cloned()
                .unwrap_or_else(|| name.to_string()),
        },
        Install::Shell { .. } | Install::GitClone { .. } | Install::Manual { .. } => {
            UpgradeSpec::None {}
        }
    }
}

/// True when `install` offers no verifiable version pinning, so doctor
/// reports it as unpinnable rather than implying a guarantee. Brew is not
/// included: unlike a piped installer it reports an exact installed
/// version, which is what this is actually distinguishing.
fn install_is_unpinnable(install: &Install) -> bool {
    matches!(install, Install::CurlScript { .. } | Install::Shell { .. })
}

fn install_method_name(install: &Install) -> &'static str {
    match install {
        Install::Apt { .. } => "apt",
        Install::Cargo { .. } => "cargo",
        Install::CurlScript { .. } => "curl_script",
        Install::Shell { .. } => "shell",
        Install::GithubRelease { .. } => "github_release",
        Install::Nvm { .. } => "nvm",
        Install::GitClone { .. } => "git_clone",
        Install::Brew { .. } => "brew",
        Install::Manual { .. } => "manual",
    }
}

impl Tool {
    /// The install method's name on the platform this binary was built for.
    /// Every caller that must respect `--platform` resolves the tool first
    /// and calls [`EffectiveTool::method_name`] instead.
    pub fn method_name(&self) -> &'static str {
        install_method_name(&self.install)
    }

    /// Resolves this tool against `platform`: the effective view (base
    /// fields with the platform override's `Some` fields substituted), or
    /// why the platform cannot install it at all.
    ///
    /// A tool with no `[tool.platform.<key>]` entry for `platform` falls
    /// through to [`Platform::admits`] — apt tools are unsupported on macOS
    /// with a generated note, everything else resolves unchanged. A tool
    /// *with* an entry is explicitly accounted for on that platform: its
    /// `unsupported` note wins if set, otherwise every field the override
    /// left `None` is inherited from the base definition.
    pub fn resolve(&self, platform: Platform) -> Resolved<'_> {
        match self.platform_overrides.get(platform.key()) {
            Some(over) => {
                if let Some(note) = &over.unsupported {
                    return Resolved::Unsupported(note.clone());
                }
                Resolved::Supported(EffectiveTool {
                    detect: over.detect.as_ref().unwrap_or(&self.detect),
                    install: over.install.as_ref().unwrap_or(&self.install),
                    requires: over.requires.as_deref().unwrap_or(&self.requires),
                    post_install: over.post_install.as_deref().unwrap_or(&self.post_install),
                })
            }
            None => {
                if platform.admits(&self.install) {
                    Resolved::Supported(EffectiveTool {
                        detect: &self.detect,
                        install: &self.install,
                        requires: &self.requires,
                        post_install: &self.post_install,
                    })
                } else {
                    Resolved::Unsupported(format!(
                        "{} installs with {}, which {platform} does not have",
                        self.name,
                        install_method_name(&self.install)
                    ))
                }
            }
        }
    }

    /// The upgrade check for `platform`: an explicit override (on the
    /// override block, then the base tool) wins; otherwise it is derived
    /// from the platform's effective install method. Returns `None` when
    /// the tool is unsupported on `platform` — there is nothing to check.
    pub fn upgrade_spec_for(&self, platform: Platform) -> Option<UpgradeSpec> {
        let effective = match self.resolve(platform) {
            Resolved::Supported(effective) => effective,
            Resolved::Unsupported(_) => return None,
        };
        let over = self.platform_overrides.get(platform.key());
        if let Some(explicit) = over.and_then(|o| o.upgrade.as_ref()) {
            return Some(explicit.clone());
        }
        // The tool-level `upgrade` override was written for the *base*
        // install method. When this platform's override replaces `install`
        // without also restating `upgrade`, that base override no longer
        // describes the effective method — `delta`'s base `upgrade = apt`
        // must not leak into its macOS `brew` resolution — so it is only
        // inherited when this platform kept the base install unchanged.
        let install_overridden = over.is_some_and(|o| o.install.is_some());
        if !install_overridden && let Some(explicit) = &self.upgrade {
            return Some(explicit.clone());
        }
        Some(derive_upgrade_spec(&self.name, effective.install))
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

        for tool in &self.tools {
            for (key, over) in &tool.platform_overrides {
                if key.parse::<Platform>().is_err() {
                    bail!(
                        "tool '{}' has a platform override for unrecognized platform '{key}' \
                         (known: linux, macos)",
                        tool.name
                    );
                }
                let substitutes = over.detect.is_some()
                    || over.install.is_some()
                    || over.upgrade.is_some()
                    || over.requires.is_some()
                    || over.post_install.is_some();
                if over.unsupported.is_some() && substitutes {
                    bail!(
                        "tool '{}' platform override for '{key}' sets both 'unsupported' \
                         and a substituting field — a tool this platform cannot install has \
                         nothing else to override",
                        tool.name
                    );
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
        assert!(matches!(
            zsh.upgrade_spec_for(Platform::Linux).unwrap(),
            UpgradeSpec::Apt { .. }
        ));

        // curl-script installs only get a GitHub check when a repo is named.
        let claude = manifest.tool("claude").unwrap();
        assert!(matches!(
            claude.upgrade_spec_for(Platform::Linux).unwrap(),
            UpgradeSpec::None {}
        ));
        let uv = manifest.tool("uv").unwrap();
        assert!(matches!(
            uv.upgrade_spec_for(Platform::Linux).unwrap(),
            UpgradeSpec::GithubRelease { .. }
        ));

        // An explicit override wins over the derived default.
        let rust = manifest.tool("rust").unwrap();
        assert!(matches!(
            rust.upgrade_spec_for(Platform::Linux).unwrap(),
            UpgradeSpec::None {}
        ));
    }

    // TEST-002: Tool::resolve over an inline fixture — an override
    // substituting `install` only, leaving `detect` inherited; an explicit
    // `unsupported`; an apt tool with no override resolving to `Unsupported`
    // with the generated note; and a non-apt tool with no override
    // resolving unchanged on both platforms.
    #[test]
    fn resolve_substitutes_only_the_overridden_fields() {
        let manifest = parse(
            r#"
            [meta]
            schema_version = 1

            [[tool]]
            name = "widget"
            detect = { command = "command -v widget", bin = "widget" }
            install = { method = "apt", packages = ["widget"] }

              [tool.platform.macos]
              install = { method = "cargo", crate_name = "widget-cli" }
            "#,
        )
        .unwrap();
        let widget = manifest.tool("widget").unwrap();

        let Resolved::Supported(effective) = widget.resolve(Platform::MacOS) else {
            panic!("widget has a macos override and should resolve as supported");
        };
        assert!(matches!(effective.install, Install::Cargo { .. }));
        // detect was not overridden, so it is inherited from the base.
        assert_eq!(effective.detect.command, "command -v widget");
    }

    #[test]
    fn resolve_honors_an_explicit_unsupported_note() {
        let manifest = parse(
            r#"
            [meta]
            schema_version = 1

            [[tool]]
            name = "gizmo"
            detect = { command = "true" }
            install = { method = "shell", command = "echo hi" }

              [tool.platform.macos]
              unsupported = "no macOS build exists yet"
            "#,
        )
        .unwrap();
        let gizmo = manifest.tool("gizmo").unwrap();

        let Resolved::Unsupported(note) = gizmo.resolve(Platform::MacOS) else {
            panic!("gizmo declares itself unsupported on macos");
        };
        assert_eq!(note, "no macOS build exists yet");

        // Linux carries no override at all, so it resolves normally.
        assert!(matches!(
            gizmo.resolve(Platform::Linux),
            Resolved::Supported(_)
        ));
    }

    #[test]
    fn resolve_generates_a_note_for_an_apt_tool_with_no_override() {
        let manifest = embedded().unwrap();
        let zsh = manifest.tool("zsh").unwrap();

        let Resolved::Unsupported(note) = zsh.resolve(Platform::MacOS) else {
            panic!("zsh has no macos override and apt is not admitted there");
        };
        assert!(note.contains("zsh"), "{note}");
        assert!(note.contains("apt"), "{note}");

        // A non-apt tool with no override resolves unchanged on both
        // platforms.
        let rust = manifest.tool("rust").unwrap();
        assert!(matches!(
            rust.resolve(Platform::Linux),
            Resolved::Supported(_)
        ));
        assert!(matches!(
            rust.resolve(Platform::MacOS),
            Resolved::Supported(_)
        ));
    }

    // TEST-010: delta's macOS override changes `install` but not `upgrade`,
    // so the derived spec must follow the *effective* install (brew,
    // formula "git-delta") rather than either the tool's base apt upgrade
    // override or the tool's own name.
    #[test]
    fn upgrade_spec_follows_the_effective_install_not_the_base_override() {
        let manifest = embedded().unwrap();
        let delta = manifest.tool("delta").unwrap();

        assert!(matches!(
            delta.upgrade_spec_for(Platform::Linux).unwrap(),
            UpgradeSpec::Apt { .. }
        ));
        match delta.upgrade_spec_for(Platform::MacOS).unwrap() {
            UpgradeSpec::Brew { formula } => assert_eq!(formula, "git-delta"),
            other => panic!("expected UpgradeSpec::Brew, got {other:?}"),
        }
    }

    // TEST-003: validate() rejects an unrecognized platform key, and an
    // override that sets both `unsupported` and a substituting field.
    #[test]
    fn rejects_unrecognized_platform_key() {
        let err = parse(
            r#"
            [meta]
            schema_version = 1
            [[tool]]
            name = "a"
            detect = { command = "true" }
            install = { method = "apt", packages = ["a"] }

              [tool.platform.darwin]
              unsupported = "typo'd platform name"
            "#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("unrecognized platform"), "{err}");
        assert!(err.to_string().contains("'a'"), "{err}");
    }

    #[test]
    fn rejects_unsupported_combined_with_a_substituting_field() {
        let err = parse(
            r#"
            [meta]
            schema_version = 1
            [[tool]]
            name = "a"
            detect = { command = "true" }
            install = { method = "apt", packages = ["a"] }

              [tool.platform.macos]
              unsupported = "no macOS path"
              requires = ["b"]
            "#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("both 'unsupported'"), "{err}");
    }
}
