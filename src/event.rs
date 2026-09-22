//! Unified event stream feeding the TUI loop: terminal input, a UI tick, and
//! the background probe/health/upgrade results, all merged onto one `mpsc`
//! channel so the render loop only ever drains one queue.

use crossterm::event::{Event as CtEvent, EventStream};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;

use crate::core::configs::FileStatus;
use crate::core::doctor::Finding;
use crate::core::harness::install::state::AssetStatus;
use crate::core::probe::ProbeResult;
use crate::core::upgrade::UpgradeCheck;

#[derive(Debug)]
pub enum AppEvent {
    Term(CtEvent),
    Probe(ProbeResult),
    Health {
        generation: u64,
        findings: Vec<Finding>,
        configs: Vec<FileStatus>,
    },
    Upgrade(UpgradeCheck),
    UpgradesDone,
    Harness {
        project: Vec<AssetStatus>,
        global: Vec<AssetStatus>,
    },
    HarnessPlan {
        generation: u64,
        scope: crate::core::harness::install::receipt::ReceiptScope,
        action: crate::app::HarnessModalAction,
        steps: Option<usize>,
    },
    HarnessScope {
        scope: crate::core::harness::install::receipt::ReceiptScope,
        rows: Vec<AssetStatus>,
    },
}

pub struct EventRuntime {
    tx: UnboundedSender<AppEvent>,
    rx: UnboundedReceiver<AppEvent>,
    input: Option<JoinHandle<()>>,
}

impl EventRuntime {
    pub fn start() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let input = Some(spawn_input(tx.clone()));
        Self { tx, rx, input }
    }

    pub fn sender(&self) -> UnboundedSender<AppEvent> {
        self.tx.clone()
    }

    pub async fn recv(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }

    pub fn try_recv(&mut self) -> Result<AppEvent, mpsc::error::TryRecvError> {
        self.rx.try_recv()
    }

    pub async fn pause_input(&mut self) {
        if let Some(input) = self.input.take() {
            input.abort();
            let _ = input.await;
        }
    }

    pub fn resume_input(&mut self) {
        if self.input.is_none() {
            self.input = Some(spawn_input(self.tx.clone()));
        }
    }

    #[cfg(test)]
    pub fn input_running(&self) -> bool {
        self.input.is_some()
    }
}

fn spawn_input(tx: UnboundedSender<AppEvent>) -> JoinHandle<()> {
    let input_tx = tx.clone();
    tokio::spawn(async move {
        let mut stream = EventStream::new();
        while let Some(event) = stream.next().await {
            if let Ok(event) = event
                && input_tx.send(AppEvent::Term(event)).is_err()
            {
                break;
            }
        }
    })
}

/// Bridges a core result stream onto the TUI's event enum, keeping `core`
/// free of any knowledge of the UI.
pub fn forward_probes(tx: UnboundedSender<AppEvent>) -> UnboundedSender<ProbeResult> {
    let (probe_tx, mut probe_rx) = mpsc::unbounded_channel::<ProbeResult>();
    tokio::spawn(async move {
        while let Some(result) = probe_rx.recv().await {
            if tx.send(AppEvent::Probe(result)).is_err() {
                break;
            }
        }
    });
    probe_tx
}

pub fn forward_upgrades(tx: UnboundedSender<AppEvent>) -> UnboundedSender<Option<UpgradeCheck>> {
    let (up_tx, mut up_rx) = mpsc::unbounded_channel::<Option<UpgradeCheck>>();
    tokio::spawn(async move {
        while let Some(message) = up_rx.recv().await {
            let event = match message {
                Some(result) => AppEvent::Upgrade(result),
                None => AppEvent::UpgradesDone,
            };
            if tx.send(event).is_err() {
                break;
            }
        }
    });
    up_tx
}

#[cfg(test)]
mod tests {
    use super::EventRuntime;

    #[tokio::test]
    async fn terminal_input_can_be_paused_and_resumed() {
        let mut runtime = EventRuntime::start();
        assert!(runtime.input_running());

        runtime.pause_input().await;
        assert!(!runtime.input_running());

        runtime.resume_input();
        assert!(runtime.input_running());
        runtime.pause_input().await;
    }
}
