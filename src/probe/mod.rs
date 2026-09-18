//! Concurrent, non-blocking probing of registry tools (PAT-001): one tokio
//! task per tool runs its `check_cmd`/`version_cmd`, streaming results back
//! over the shared event channel as they land instead of blocking the
//! render loop on ~20 sequential subprocess calls.

mod version;

use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::timeout;

use crate::event::AppEvent;
use crate::registry::ToolSpec;

const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub tool: String,
    pub installed: bool,
    pub version: Option<String>,
}

pub fn spawn_probe_tasks(registry: &[ToolSpec], tx: UnboundedSender<AppEvent>) {
    // cloned() is required, not just convenient: each spawned task must own
    // its ToolSpec to satisfy tokio::spawn's 'static bound.
    #[allow(clippy::unnecessary_to_owned)]
    for spec in registry.iter().cloned() {
        let tx = tx.clone();
        tokio::spawn(async move {
            let result = probe_tool(&spec).await;
            let _ = tx.send(AppEvent::Probe(result));
        });
    }
}

async fn probe_tool(spec: &ToolSpec) -> ProbeResult {
    let installed = run_check(&spec.check_cmd).await;
    let version = if installed {
        match &spec.version_cmd {
            Some(cmd) => run_capture(cmd)
                .await
                .and_then(|out| version::extract(&out, spec.version_regex.as_deref())),
            None => None,
        }
    } else {
        None
    };
    ProbeResult {
        tool: spec.name.clone(),
        installed,
        version,
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
