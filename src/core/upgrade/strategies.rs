//! Per-method "what is the latest version" lookups. Each returns `None` on
//! any failure (missing tool, offline, rate limited, unparseable response)
//! rather than erroring the whole check — an upgrade report is advisory, and
//! one unreachable source must not hide the other twenty answers.

use std::process::Stdio;
use std::time::Duration;

use reqwest::Client;
use tokio::process::Command;
use tokio::time::timeout;

use crate::core::manifest::UpgradeSpec;

const HTTP_TIMEOUT: Duration = Duration::from_secs(8);
const PROCESS_TIMEOUT: Duration = Duration::from_secs(8);

pub fn client() -> Option<Client> {
    Client::builder()
        .user_agent("pinst")
        .timeout(HTTP_TIMEOUT)
        .build()
        .ok()
}

pub async fn latest_version(spec: &UpgradeSpec, client: &Client) -> Option<String> {
    match spec {
        UpgradeSpec::Apt { package } => apt_latest(package).await,
        UpgradeSpec::Cargo { crate_name } => cargo_latest(crate_name).await,
        UpgradeSpec::GithubRelease { repo } => github_latest(client, repo).await,
        UpgradeSpec::Nvm {} => nvm_latest_lts(client).await,
        UpgradeSpec::Brew { formula } => brew_latest(formula).await,
        UpgradeSpec::None {} => None,
    }
}

async fn output(command: &mut Command, wait: Duration) -> Option<std::process::Output> {
    command.stdin(Stdio::null()).kill_on_drop(true);
    timeout(wait, command.output()).await.ok()?.ok()
}

async fn apt_latest(package: &str) -> Option<String> {
    let mut command = Command::new("apt-cache");
    command.arg("policy").arg(package);
    let output = output(&mut command, PROCESS_TIMEOUT).await?;
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

async fn cargo_latest(crate_name: &str) -> Option<String> {
    let mut command = Command::new("cargo");
    command
        .arg("search")
        .arg(crate_name)
        .arg("--limit")
        .arg("1");
    let output = output(&mut command, PROCESS_TIMEOUT).await?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    // Output line shape: `crate_name = "1.2.3"    # description`
    text.lines().next()?.split('"').nth(1).map(str::to_string)
}

async fn github_latest(client: &Client, repo: &str) -> Option<String> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    json.get("tag_name")?
        .as_str()
        .map(|s| s.trim_start_matches('v').to_string())
}

/// `brew info --json=v2` is the stable machine interface Homebrew documents;
/// parsing the human-readable `brew info` output would break on the next
/// release's formatting change.
async fn brew_latest(formula: &str) -> Option<String> {
    brew_latest_with("brew", formula).await
}

async fn brew_latest_with(program: &str, formula: &str) -> Option<String> {
    let mut command = Command::new(program);
    command
        .arg("info")
        .arg("--json=v2")
        .arg("--formula")
        .arg(formula);
    let output = output(&mut command, PROCESS_TIMEOUT).await?;
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

async fn nvm_latest_lts(client: &Client) -> Option<String> {
    let resp = client
        .get("https://nodejs.org/dist/index.json")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
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
    #[tokio::test]
    async fn brew_latest_degrades_to_none_without_brew_on_path() {
        let result = brew_latest_with("pinst-test-binary-that-does-not-exist", "git-delta").await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn subprocesses_are_killed_when_the_lookup_timeout_expires() {
        let mut command = Command::new("sh");
        command.arg("-c").arg("sleep 10; printf too-late");
        let result = output(&mut command, Duration::from_millis(20)).await;
        assert!(result.is_none());
    }
}
