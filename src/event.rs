//! Unified event stream feeding the TUI loop: terminal input, a UI tick, and
//! the background probe/health/upgrade results, all merged onto one `mpsc`
//! channel so the render loop only ever drains one queue.

use std::time::Duration;

use crossterm::event::{Event as CtEvent, EventStream};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio_stream::StreamExt;

use crate::core::configs::FileStatus;
use crate::core::doctor::Finding;
use crate::core::harness::install::state::AssetStatus;
use crate::core::probe::ProbeResult;
use crate::core::upgrade::UpgradeResult;

#[derive(Debug)]
pub enum AppEvent {
    Term(CtEvent),
    Tick,
    Probe(ProbeResult),
    Health {
        findings: Vec<Finding>,
        configs: Vec<FileStatus>,
    },
    Upgrade(UpgradeResult),
    UpgradesDone,
    Harness {
        project: Vec<AssetStatus>,
        global: Vec<AssetStatus>,
    },
}

/// Spawns the input-reading and tick-generating background tasks and returns
/// the shared sender (cloned into the background work) and the receiver the
/// main loop drains.
pub fn start_event_loop() -> (UnboundedSender<AppEvent>, UnboundedReceiver<AppEvent>) {
    let (tx, rx) = mpsc::unbounded_channel();

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
    });

    let tick_tx = tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(250));
        loop {
            interval.tick().await;
            if tick_tx.send(AppEvent::Tick).is_err() {
                break;
            }
        }
    });

    (tx, rx)
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

pub fn forward_upgrades(tx: UnboundedSender<AppEvent>) -> UnboundedSender<Option<UpgradeResult>> {
    let (up_tx, mut up_rx) = mpsc::unbounded_channel::<Option<UpgradeResult>>();
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
