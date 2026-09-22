//! Upgrade detection: per-tool "what is the latest version" lookups
//! dispatched by the manifest's upgrade spec, behind an on-disk TTL cache so
//! repeated runs do not hammer rate-limited sources.

mod strategies;

use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::{Semaphore, mpsc::UnboundedSender};
use tokio::task::JoinSet;

use crate::core::manifest::{Tool, UpgradeSpec};
use crate::core::platform::Platform;
use crate::core::probe::ProbeResult;

const TTL_SECS: u64 = 24 * 60 * 60;
const MAX_CONCURRENT_LOOKUPS: usize = 4;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeCheckState {
    Cached,
    Checked,
    Unavailable,
    NotApplicable,
    Unsupported,
}

#[derive(Debug, Clone)]
pub struct UpgradeCheck {
    pub tool: String,
    pub result: Option<UpgradeResult>,
    pub state: UpgradeCheckState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    latest: Option<String>,
    checked_at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Cache(HashMap<String, CacheEntry>);

type Lookup =
    Arc<dyn Fn(UpgradeSpec) -> Pin<Box<dyn Future<Output = Option<String>> + Send>> + Send + Sync>;

struct CompletedCheck {
    check: UpgradeCheck,
    cache_entry: Option<CacheEntry>,
}

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
async fn check_one(
    tool: Tool,
    current: Option<String>,
    cached: Option<CacheEntry>,
    force: bool,
    platform: Platform,
    lookup: Lookup,
) -> CompletedCheck {
    let crate::core::manifest::Resolved::Supported(effective) = tool.resolve(platform) else {
        return CompletedCheck {
            check: UpgradeCheck {
                tool: tool.name,
                result: None,
                state: UpgradeCheckState::Unsupported,
            },
            cache_entry: None,
        };
    };
    let unpinnable = effective.is_unpinnable();
    let Some(spec) = tool.upgrade_spec_for(platform) else {
        return CompletedCheck {
            check: UpgradeCheck {
                tool: tool.name,
                result: None,
                state: UpgradeCheckState::Unsupported,
            },
            cache_entry: None,
        };
    };
    if matches!(spec, UpgradeSpec::None {}) {
        return CompletedCheck {
            check: UpgradeCheck {
                tool: tool.name.clone(),
                result: Some(compare(&tool, &spec, current, None, unpinnable)),
                state: UpgradeCheckState::NotApplicable,
            },
            cache_entry: None,
        };
    }

    let fresh = (!force)
        .then_some(cached)
        .flatten()
        .filter(|entry| now().saturating_sub(entry.checked_at) < TTL_SECS);

    if let Some(entry) = fresh {
        return CompletedCheck {
            check: UpgradeCheck {
                tool: tool.name.clone(),
                result: Some(compare(&tool, &spec, current, entry.latest, unpinnable)),
                state: UpgradeCheckState::Cached,
            },
            cache_entry: None,
        };
    }

    let latest = lookup(spec.clone()).await;
    let state = if latest.is_some() {
        UpgradeCheckState::Checked
    } else {
        UpgradeCheckState::Unavailable
    };
    let cache_entry = latest.as_ref().map(|_| CacheEntry {
        latest: latest.clone(),
        checked_at: now(),
    });
    CompletedCheck {
        check: UpgradeCheck {
            tool: tool.name.clone(),
            result: Some(compare(&tool, &spec, current, latest, unpinnable)),
            state,
        },
        cache_entry,
    }
}

fn lookup() -> Lookup {
    let client = strategies::client();
    Arc::new(move |spec| {
        let client = client.clone();
        Box::pin(async move {
            let client = client?;
            strategies::latest_version(&spec, &client).await
        })
    })
}

async fn run_checks_with(
    tools: Vec<Tool>,
    probes: BTreeMap<String, ProbeResult>,
    force: bool,
    platform: Platform,
    mut cache: Cache,
    tx: Option<UnboundedSender<Option<UpgradeCheck>>>,
    lookup: Lookup,
) -> (Vec<UpgradeCheck>, Cache) {
    let order: Vec<String> = tools.iter().map(|tool| tool.name.clone()).collect();
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_LOOKUPS));
    let mut set = JoinSet::new();

    for tool in tools {
        let current = probes
            .get(&tool.name)
            .and_then(|probe| probe.version.clone());
        let cached = cache.0.get(&tool.name).cloned();
        let permit = semaphore.clone();
        let lookup = lookup.clone();
        set.spawn(async move {
            let _permit = permit.acquire_owned().await.ok();
            check_one(tool, current, cached, force, platform, lookup).await
        });
    }

    let mut completed = BTreeMap::new();
    while let Some(joined) = set.join_next().await {
        let Ok(outcome) = joined else { continue };
        if let Some(entry) = outcome.cache_entry {
            cache.0.insert(outcome.check.tool.clone(), entry);
        }
        if let Some(tx) = &tx
            && tx.send(Some(outcome.check.clone())).is_err()
        {
            return (Vec::new(), cache);
        }
        completed.insert(outcome.check.tool.clone(), outcome.check);
    }

    let ordered = order
        .iter()
        .filter_map(|name| completed.remove(name))
        .collect();
    (ordered, cache)
}

/// Upgrade check over a whole tool set, using each tool's effective upgrade
/// spec for `platform`. Results retain manifest order for deterministic CLI
/// output even though stale lookups run concurrently.
pub async fn check_all(
    tools: &[&Tool],
    probes: &BTreeMap<String, ProbeResult>,
    force: bool,
    platform: Platform,
) -> Vec<UpgradeResult> {
    let owned: Vec<Tool> = tools.iter().map(|tool| (*tool).clone()).collect();
    let (checks, cache) = run_checks_with(
        owned,
        probes.clone(),
        force,
        platform,
        load_cache(),
        None,
        lookup(),
    )
    .await;
    save_cache(&cache);
    checks
        .iter()
        .filter_map(|check| check.result.clone())
        .collect()
}

/// Streams results to `tx` as they land, for the TUI's Upgrades tab — always
/// for `Platform::host()`, since the TUI checks the machine it is running on.
pub fn spawn_streaming(
    tools: Vec<Tool>,
    probes: BTreeMap<String, ProbeResult>,
    tx: UnboundedSender<Option<UpgradeCheck>>,
    force: bool,
    platform: Platform,
) {
    tokio::spawn(async move {
        let (checks, cache) = run_checks_with(
            tools,
            probes,
            force,
            platform,
            load_cache(),
            Some(tx.clone()),
            lookup(),
        )
        .await;
        if checks.is_empty() && tx.is_closed() {
            return;
        }
        save_cache(&cache);
        // None marks the end of the stream.
        let _ = tx.send(None);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::time::{Duration, sleep};

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

    #[tokio::test]
    async fn cached_checks_skip_lookup_while_forced_checks_replace_them() {
        let tool = tool();
        let mut cache = Cache::default();
        cache.0.insert(
            tool.name.clone(),
            CacheEntry {
                latest: Some("99.0.0".to_string()),
                checked_at: now(),
            },
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let lookup_calls = calls.clone();
        let lookup: Lookup = Arc::new(move |_| {
            let calls = lookup_calls.clone();
            Box::pin(async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Some("100.0.0".to_string())
            })
        });

        let (cached, _) = run_checks_with(
            vec![tool.clone()],
            BTreeMap::new(),
            false,
            Platform::Linux,
            cache.clone(),
            None,
            lookup.clone(),
        )
        .await;
        assert_eq!(cached[0].state, UpgradeCheckState::Cached);
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        let (forced, updated) = run_checks_with(
            vec![tool.clone()],
            BTreeMap::new(),
            true,
            Platform::Linux,
            cache,
            None,
            lookup,
        )
        .await;
        assert_eq!(forced[0].state, UpgradeCheckState::Checked);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(updated.0[&tool.name].latest.as_deref(), Some("100.0.0"));
    }

    #[tokio::test]
    async fn stale_lookups_are_bounded_and_results_keep_manifest_order() {
        let template = tool();
        let tools: Vec<Tool> = (0..8)
            .map(|index| {
                let mut tool = template.clone();
                tool.name = format!("tool-{index}");
                tool
            })
            .collect();
        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let maximum_seen = maximum.clone();
        let lookup: Lookup = Arc::new(move |_| {
            let active = active.clone();
            let maximum = maximum.clone();
            Box::pin(async move {
                let now_active = active.fetch_add(1, Ordering::SeqCst) + 1;
                maximum.fetch_max(now_active, Ordering::SeqCst);
                sleep(Duration::from_millis(20)).await;
                active.fetch_sub(1, Ordering::SeqCst);
                Some("1.0.0".to_string())
            })
        });

        let (checks, _) = run_checks_with(
            tools,
            BTreeMap::new(),
            true,
            Platform::Linux,
            Cache::default(),
            None,
            lookup,
        )
        .await;

        let names: Vec<&str> = checks.iter().map(|check| check.tool.as_str()).collect();
        assert_eq!(
            names,
            [
                "tool-0", "tool-1", "tool-2", "tool-3", "tool-4", "tool-5", "tool-6", "tool-7"
            ]
        );
        assert_eq!(maximum_seen.load(Ordering::SeqCst), MAX_CONCURRENT_LOOKUPS);
    }

    #[tokio::test]
    async fn unsupported_and_uncheckable_tools_reach_terminal_states() {
        let manifest = crate::core::manifest::embedded().unwrap();
        let tools = vec![
            manifest.tool("homebrew").unwrap().clone(),
            manifest.tool("rust").unwrap().clone(),
        ];
        let lookup: Lookup = Arc::new(|_| Box::pin(async { None }));
        let (checks, _) = run_checks_with(
            tools,
            BTreeMap::new(),
            true,
            Platform::Linux,
            Cache::default(),
            None,
            lookup,
        )
        .await;

        assert_eq!(checks[0].state, UpgradeCheckState::Unsupported);
        assert_eq!(checks[1].state, UpgradeCheckState::NotApplicable);
    }
}
