//! Upgrade detection: per-tool "what is the latest version" lookups
//! dispatched by the manifest's upgrade spec, behind an on-disk TTL cache so
//! repeated runs do not hammer rate-limited sources.

mod strategies;

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use crate::core::manifest::{Tool, UpgradeSpec};
use crate::core::platform::Platform;
use crate::core::probe::ProbeResult;

const TTL_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct UpgradeResult {
    pub tool: String,
    pub current: Option<String>,
    pub latest: Option<String>,
    pub upgrade_available: bool,
    /// True when the install method cannot be version-pinned or verified
    /// (curl-piped installers), so "up to date" is a best effort.
    pub unpinnable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    latest: Option<String>,
    checked_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Cache(HashMap<String, CacheEntry>);

fn cache_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join(".cache")
            .join("pinst")
            .join("upgrade_cache.json"),
    )
}

fn load_cache() -> Cache {
    cache_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_cache(cache: &Cache) {
    let Some(path) = cache_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(cache) {
        let _ = std::fs::write(path, json);
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// apt candidates look like `4:15.2.0-5ubuntu1`: an optional epoch, the
/// upstream version, then the distro revision. Only the middle part is
/// comparable with what `--version` reports.
fn upstream_of(candidate: &str) -> &str {
    let without_epoch = candidate
        .split_once(':')
        .map(|(_, rest)| rest)
        .unwrap_or(candidate);
    without_epoch.split('-').next().unwrap_or(without_epoch)
}

/// Homebrew decorates a formula's version with a revision suffix when it
/// rebuilds the same upstream release — `14.1.0_1` — the way apt decorates
/// with an epoch and a distro revision. Only the part before `_` is
/// comparable with what `--version` reports; stripping the whole string
/// unconditionally would be wrong for a formula whose upstream version
/// itself contains an underscore, so this only strips a *trailing*
/// `_<digits>` revision marker.
fn brew_upstream_of(candidate: &str) -> &str {
    match candidate.rsplit_once('_') {
        Some((base, revision))
            if !revision.is_empty() && revision.bytes().all(|b| b.is_ascii_digit()) =>
        {
            base
        }
        _ => candidate,
    }
}

fn compare(
    tool: &Tool,
    spec: &UpgradeSpec,
    current: Option<String>,
    latest: Option<String>,
    unpinnable: bool,
) -> UpgradeResult {
    // Only apt and brew carry this kind of packaging decoration. Stripping
    // it from a GitHub tag would turn `1.2.0-rc1` into `1.2.0` and invent an
    // upgrade.
    let upgrade_available = match (&current, &latest) {
        (Some(c), Some(l)) => match spec {
            UpgradeSpec::Apt { .. } => upstream_of(l) != c,
            UpgradeSpec::Brew { .. } => brew_upstream_of(l) != c,
            _ => l != c,
        },
        _ => false,
    };
    UpgradeResult {
        tool: tool.name.clone(),
        current,
        latest,
        upgrade_available,
        unpinnable,
    }
}

/// Looks up the latest version for one tool, using the cache unless `force`.
/// Returns `None` when the tool is unsupported on `platform` — there is
/// nothing to check.
fn check_one(
    tool: &Tool,
    current: Option<String>,
    cache: &mut Cache,
    force: bool,
    platform: Platform,
) -> Option<UpgradeResult> {
    let crate::core::manifest::Resolved::Supported(effective) = tool.resolve(platform) else {
        return None;
    };
    let unpinnable = effective.is_unpinnable();
    let spec = tool.upgrade_spec_for(platform)?;
    if matches!(spec, UpgradeSpec::None {}) {
        return Some(compare(tool, &spec, current, None, unpinnable));
    }

    let fresh = (!force)
        .then(|| cache.0.get(&tool.name))
        .flatten()
        .filter(|entry| now().saturating_sub(entry.checked_at) < TTL_SECS);

    let latest = match fresh {
        Some(entry) => entry.latest.clone(),
        None => {
            let looked_up = strategies::latest_version(&spec);
            // Only cache an answer we actually got: caching a network failure
            // would pin "no version information" for the full TTL after a
            // single offline run.
            if looked_up.is_some() {
                cache.0.insert(
                    tool.name.clone(),
                    CacheEntry {
                        latest: looked_up.clone(),
                        checked_at: now(),
                    },
                );
            }
            looked_up
        }
    };

    Some(compare(tool, &spec, current, latest, unpinnable))
}

/// Blocking upgrade check over a whole tool set, using each tool's effective
/// upgrade spec for `platform`. Callers on an async runtime must wrap this in
/// `spawn_blocking` — the lookups shell out and hit the network.
pub fn check_all(
    tools: &[&Tool],
    probes: &BTreeMap<String, ProbeResult>,
    force: bool,
    platform: Platform,
) -> Vec<UpgradeResult> {
    let mut cache = load_cache();
    let results: Vec<UpgradeResult> = tools
        .iter()
        .filter_map(|tool| {
            let current = probes.get(&tool.name).and_then(|p| p.version.clone());
            check_one(tool, current, &mut cache, force, platform)
        })
        .collect();
    save_cache(&cache);
    results
}

/// Streams results to `tx` as they land, for the TUI's Upgrades tab — always
/// for `Platform::host()`, since the TUI checks the machine it is running on.
pub fn spawn_streaming(
    tools: Vec<Tool>,
    probes: BTreeMap<String, ProbeResult>,
    tx: UnboundedSender<Option<UpgradeResult>>,
    force: bool,
    platform: Platform,
) {
    tokio::task::spawn_blocking(move || {
        let mut cache = load_cache();
        for tool in &tools {
            let current = probes.get(&tool.name).and_then(|p| p.version.clone());
            if let Some(result) = check_one(tool, current, &mut cache, force, platform)
                && tx.send(Some(result)).is_err()
            {
                return;
            }
        }
        save_cache(&cache);
        // None marks the end of the stream.
        let _ = tx.send(None);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool() -> Tool {
        crate::core::manifest::embedded()
            .unwrap()
            .tool("curl")
            .unwrap()
            .clone()
    }

    // Sits beside the brew case below: apt's epoch/revision decoration was
    // already handled by `upstream_of`, just never pinned by a test of its
    // own until this one.
    #[test]
    fn apt_epoch_and_revision_decoration_does_not_invent_an_upgrade() {
        let spec = UpgradeSpec::Apt {
            package: "curl".to_string(),
        };
        let result = compare(
            &tool(),
            &spec,
            Some("7.81.0".to_string()),
            Some("4:7.81.0-1ubuntu1.20".to_string()),
            false,
        );
        assert!(!result.upgrade_available);
    }

    // TEST-009 / RISK-003: a brew revision suffix must not read as a newer
    // version than the one already installed.
    #[test]
    fn brew_revision_suffix_does_not_invent_an_upgrade() {
        let spec = UpgradeSpec::Brew {
            formula: "git-delta".to_string(),
        };
        let result = compare(
            &tool(),
            &spec,
            Some("14.1.0".to_string()),
            Some("14.1.0_1".to_string()),
            false,
        );
        assert!(!result.upgrade_available);
    }

    #[test]
    fn a_genuinely_newer_brew_version_is_still_an_upgrade() {
        let spec = UpgradeSpec::Brew {
            formula: "git-delta".to_string(),
        };
        let result = compare(
            &tool(),
            &spec,
            Some("14.1.0".to_string()),
            Some("14.2.0".to_string()),
            false,
        );
        assert!(result.upgrade_available);
    }
}
