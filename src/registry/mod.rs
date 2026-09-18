//! Tool registry: the declarative list of tools pinst knows about, seeded
//! from `registry.toml` (see that file's header for schema notes and why it
//! mirrors bootstrap.sh's own tool list rather than duplicating it).

use std::path::PathBuf;

use color_eyre::eyre::{Context, Result};
use serde::Deserialize;

const DEFAULT_REGISTRY_TOML: &str = include_str!("../../registry.toml");

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstallMethod {
    Apt,
    Cargo,
    CurlScript,
    Nvm,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub package: String,
    pub check_cmd: String,
    #[serde(default)]
    pub version_cmd: Option<String>,
    #[serde(default)]
    pub version_regex: Option<String>,
    #[serde(default)]
    pub bin_name: Option<String>,
    pub install_method: InstallMethod,
    #[serde(default)]
    pub apt_package: Option<String>,
    #[serde(default)]
    pub cargo_crate: Option<String>,
    #[serde(default)]
    pub github_repo: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RegistryFile {
    #[serde(rename = "tool", default)]
    tools: Vec<ToolSpec>,
}

/// Loads the tool registry, preferring an external override (env var,
/// `~/.config/pinst/registry.toml`, or `./registry.toml`) over the copy
/// embedded in the binary at compile time, so the registry can be edited
/// without recompiling pinst.
pub fn load_registry() -> Result<Vec<ToolSpec>> {
    let raw = match find_registry_source() {
        Some(path) => std::fs::read_to_string(&path)
            .with_context(|| format!("reading registry at {}", path.display()))?,
        None => DEFAULT_REGISTRY_TOML.to_string(),
    };
    let parsed: RegistryFile = toml::from_str(&raw).context("parsing registry.toml")?;
    Ok(parsed.tools)
}

fn find_registry_source() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("PINST_REGISTRY") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let config = PathBuf::from(home).join(".config/pinst/registry.toml");
        if config.is_file() {
            return Some(config);
        }
    }
    let cwd = PathBuf::from("registry.toml");
    if cwd.is_file() {
        return Some(cwd);
    }
    None
}
