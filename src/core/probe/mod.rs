//! Concurrent, non-blocking detection of manifest tools: one task per tool
//! runs its `detect.command` (exit 0 = installed) and `detect.version_cmd`,
//! so probing ~25 tools costs one round of subprocesses rather than 25.

mod version;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use schemars::JsonSchema;
use serde::Serialize;
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::timeout;

use crate::core::manifest::{Resolved, Tool};
use crate::core::platform::Platform;

const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ProbeResult {
    pub tool: String,
    pub installed: bool,
    pub version: Option<String>,
    /// Where the tool's binary resolved on PATH, when it declares one.
    pub path: Option<PathBuf>,
}

/// Probes every tool concurrently and collects the results, using each
/// tool's effective `detect` for `platform`.
pub async fn probe_all(tools: &[&Tool], platform: Platform) -> BTreeMap<String, ProbeResult> {
    let mut set = tokio::task::JoinSet::new();
    for tool in tools {
        let tool = (*tool).clone();
        set.spawn(async move { probe_tool(&tool, platform).await });
    }

    let mut results = BTreeMap::new();
    while let Some(joined) = set.join_next().await {
        if let Ok(result) = joined {
            results.insert(result.tool.clone(), result);
        }
    }
    results
}

/// Streams results to `tx` as each probe lands. Used by the TUI, which
/// renders partial state rather than waiting for the whole set — always for
/// `Platform::host()`, since the TUI reports on the machine it is running
/// on rather than an overridable target.
pub fn spawn_streaming(tools: Vec<Tool>, tx: UnboundedSender<ProbeResult>, platform: Platform) {
    for tool in tools {
        let tx = tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(probe_tool(&tool, platform).await);
        });
    }
}

pub async fn probe_tool(tool: &Tool, platform: Platform) -> ProbeResult {
    // A tool `platform` cannot install has nothing to detect; callers are
    // expected to have already filtered these out via `graph::select`; this
    // is the fallback for one that slipped through some other path (e.g. a
    // direct `docs`/`schema` lookup) rather than a panic.
    let Resolved::Supported(effective) = tool.resolve(platform) else {
        return ProbeResult {
            tool: tool.name.clone(),
            installed: false,
            version: None,
            path: None,
        };
    };
    let installed = run_check(&effective.detect.command).await;
    let version = if installed {
        match &effective.detect.version_cmd {
            Some(cmd) => run_capture(cmd)
                .await
                .and_then(|out| version::extract(&out, effective.detect.version_regex.as_deref())),
            None => None,
        }
    } else {
        None
    };
    let path = effective
        .detect
        .bin
        .as_deref()
        .and_then(|bin| which::which(bin).ok());

    ProbeResult {
        tool: tool.name.clone(),
        installed,
        version,
        path,
    }
}

async fn run_check(cmd: &str) -> bool {
    let result = timeout(
        PROBE_TIMEOUT,
        Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status(),
    )
    .await;
    matches!(result, Ok(Ok(status)) if status.success())
}

async fn run_capture(cmd: &str) -> Option<String> {
    let result = timeout(
        PROBE_TIMEOUT,
        Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .stdin(Stdio::null())
            .output(),
    )
    .await;
    match result {
        Ok(Ok(output)) => {
            let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
            combined.push(' ');
            combined.push_str(&String::from_utf8_lossy(&output.stderr));
            Some(combined)
        }
        _ => None,
    }
}
