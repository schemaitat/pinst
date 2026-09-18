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
