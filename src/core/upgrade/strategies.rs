//! Per-method "what is the latest version" lookups. Each returns `None` on
//! any failure (missing tool, offline, rate limited, unparseable response)
//! rather than erroring the whole check — an upgrade report is advisory, and
//! one unreachable source must not hide the other twenty answers.

use std::process::Command;
use std::time::Duration;

use crate::core::manifest::UpgradeSpec;

const HTTP_TIMEOUT: Duration = Duration::from_secs(8);

pub fn latest_version(spec: &UpgradeSpec) -> Option<String> {
    match spec {
        UpgradeSpec::Apt { package } => apt_latest(package),
        UpgradeSpec::Cargo { crate_name } => cargo_latest(crate_name),
        UpgradeSpec::GithubRelease { repo } => github_latest(repo),
        UpgradeSpec::Nvm {} => nvm_latest_lts(),
        UpgradeSpec::Brew { formula } => brew_latest(formula),
        UpgradeSpec::None {} => None,
    }
}

fn apt_latest(package: &str) -> Option<String> {
    let output = Command::new("apt-cache")
        .arg("policy")
        .arg(package)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("Candidate:")?;
        let v = rest.trim();
        (!v.is_empty() && v != "(none)").then(|| v.to_string())
    })
}

fn cargo_latest(crate_name: &str) -> Option<String> {
    let output = Command::new("cargo")
        .arg("search")
        .arg(crate_name)
        .arg("--limit")
        .arg("1")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    // Output line shape: `crate_name = "1.2.3"    # description`
    text.lines().next()?.split('"').nth(1).map(str::to_string)
}

fn client() -> Option<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent("pinst")
        .timeout(HTTP_TIMEOUT)
        .build()
        .ok()
}

fn github_latest(repo: &str) -> Option<String> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let resp = client()?.get(&url).send().ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().ok()?;
    json.get("tag_name")?
        .as_str()
        .map(|s| s.trim_start_matches('v').to_string())
}

/// `brew info --json=v2` is the stable machine interface Homebrew documents;
/// parsing the human-readable `brew info` output would break on the next
/// release's formatting change.
fn brew_latest(formula: &str) -> Option<String> {
    let output = Command::new("brew")
        .arg("info")
        .arg("--json=v2")
        .arg("--formula")
        .arg(formula)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    json.get("formulae")?
        .as_array()?
        .first()?
        .get("versions")?
        .get("stable")?
        .as_str()
        .map(str::to_string)
}

fn nvm_latest_lts() -> Option<String> {
    let resp = client()?
        .get("https://nodejs.org/dist/index.json")
        .send()
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().ok()?;
    json.as_array()?
        .iter()
        .find(|entry| {
            entry
                .get("lts")
                .map(|v| v.as_bool() != Some(false))
                .unwrap_or(false)
        })
        .and_then(|entry| entry.get("version"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim_start_matches('v').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // TEST-013: brew_latest degrades to None when `brew` is not on PATH,
    // rather than erroring the whole upgrade check (LESSON-029: the
    // documented degradation is the test specification). PATH is process-
    // wide, so this is serialized against every other env-var test.
    #[test]
    fn brew_latest_degrades_to_none_without_brew_on_path() {
        let _guard = crate::core::source::test_env_lock();
        let original = std::env::var_os("PATH");

        // An empty PATH means `Command::new("brew")` cannot resolve the
        // binary at all — the actual "brew absent" case, not merely "this
        // particular brew failed".
        unsafe {
            std::env::set_var("PATH", "");
        }
        let result = brew_latest("git-delta");
        match original {
            Some(path) => unsafe { std::env::set_var("PATH", path) },
            None => unsafe { std::env::remove_var("PATH") },
        }

        assert_eq!(result, None);
    }
}
