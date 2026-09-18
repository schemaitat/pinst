//! Per-install-method "what's the latest version" lookups (TASK-017). Each
//! function returns `None` on any failure (missing tool, offline, rate
//! limited, unparseable response) rather than erroring the whole Upgrades
//! view — RISK-001.

use std::process::Command;
use std::time::Duration;

use super::UpgradeStrategy;
use crate::registry::ToolSpec;

const HTTP_TIMEOUT: Duration = Duration::from_secs(8);

pub fn latest_version(spec: &ToolSpec, strategy: UpgradeStrategy) -> Option<String> {
    match strategy {
        UpgradeStrategy::Apt => apt_latest(spec.apt_package.as_deref().unwrap_or(&spec.name)),
        UpgradeStrategy::Cargo => {
            cargo_latest(spec.cargo_crate.as_deref().unwrap_or(&spec.name))
        }
        UpgradeStrategy::GithubRelease => spec.github_repo.as_deref().and_then(github_latest),
        UpgradeStrategy::Nvm => nvm_latest_lts(),
        UpgradeStrategy::Unsupported => None,
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
        let line = line.trim();
        let rest = line.strip_prefix("Candidate:")?;
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
    let first_line = text.lines().next()?;
    first_line.split('"').nth(1).map(str::to_string)
}

fn github_latest(repo: &str) -> Option<String> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let client = reqwest::blocking::Client::builder()
        .user_agent("pinst-dotfiles-tui")
        .timeout(HTTP_TIMEOUT)
        .build()
        .ok()?;
    let resp = client.get(&url).send().ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().ok()?;
    json.get("tag_name")?
        .as_str()
        .map(|s| s.trim_start_matches('v').to_string())
}

fn nvm_latest_lts() -> Option<String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("pinst-dotfiles-tui")
        .timeout(HTTP_TIMEOUT)
        .build()
        .ok()?;
    let resp = client
        .get("https://nodejs.org/dist/index.json")
        .send()
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().ok()?;
    json.as_array()?
        .iter()
        .find(|entry| entry.get("lts").map(|v| v.as_bool() != Some(false)).unwrap_or(false))
        .and_then(|entry| entry.get("version"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim_start_matches('v').to_string())
}
