//! Upgrade detection: dispatches a per-tool "what's the latest version"
//! lookup by install method (TASK-016), backed by an on-disk TTL cache so
//! repeated launches don't hammer rate-limited/slow network sources
//! (TASK-018, RISK-001).

mod strategies;

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use crate::event::AppEvent;
use crate::probe::ProbeResult;
use crate::registry::{InstallMethod, ToolSpec};

const TTL_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeStrategy {
    Apt,
    Cargo,
    GithubRelease,
    Nvm,
    /// No queryable "latest version" source for this tool's install method
    /// (e.g. a curl-script installer with no known GitHub releases feed).
    Unsupported,
}

pub fn strategy_for(spec: &ToolSpec) -> UpgradeStrategy {
    match spec.install_method {
        InstallMethod::Apt => UpgradeStrategy::Apt,
        InstallMethod::Cargo => UpgradeStrategy::Cargo,
        InstallMethod::Nvm => UpgradeStrategy::Nvm,
        InstallMethod::CurlScript => {
            if spec.github_repo.is_some() {
                UpgradeStrategy::GithubRelease
            } else {
                UpgradeStrategy::Unsupported
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct UpgradeResult {
    pub tool: String,
    pub current: Option<String>,
    pub latest: Option<String>,
    pub upgrade_available: bool,
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

/// Runs upgrade lookups for the whole registry on a blocking task (network +
/// subprocess calls), streaming one `AppEvent::Upgrade` per tool as results
/// land and a final `AppEvent::UpgradesDone` when the pass completes.
/// `force` bypasses the TTL cache (the Upgrades tab's manual refresh).
pub fn spawn_upgrade_check(
    registry: Vec<ToolSpec>,
    probes: BTreeMap<String, ProbeResult>,
    tx: UnboundedSender<AppEvent>,
    force: bool,
) {
    tokio::task::spawn_blocking(move || {
        let mut cache = load_cache();
        for spec in &registry {
            let strategy = strategy_for(spec);
            let current = probes.get(&spec.name).and_then(|p| p.version.clone());

            let cached_fresh = (!force)
                .then(|| cache.0.get(&spec.name))
                .flatten()
                .filter(|entry| now().saturating_sub(entry.checked_at) < TTL_SECS);

            let latest = match cached_fresh {
                Some(entry) => entry.latest.clone(),
                None => {
                    let v = strategies::latest_version(spec, strategy);
                    cache.0.insert(
                        spec.name.clone(),
                        CacheEntry {
                            latest: v.clone(),
                            checked_at: now(),
                        },
                    );
                    v
                }
            };

            // apt's Candidate version carries a distro revision suffix and
            // sometimes an epoch prefix (e.g. installed "5.9" vs. Candidate
            // "5.9-8ubuntu3", or "4:15.2.0-5ubuntu1") that a straight
            // inequality would misreport as an available upgrade on every
            // apt-managed tool. Treat the installed version appearing
            // anywhere in the candidate string as still up to date; only a
            // genuinely different upstream version counts as an upgrade.
            let upgrade_available = match (&current, &latest) {
                (Some(c), Some(l)) => !l.contains(c.as_str()),
                _ => false,
            };

            let result = UpgradeResult {
                tool: spec.name.clone(),
                current,
                latest,
                upgrade_available,
            };
            if tx.send(AppEvent::Upgrade(result)).is_err() {
                return;
            }
        }
        save_cache(&cache);
        let _ = tx.send(AppEvent::UpgradesDone);
    });
}
