//! Unified event stream feeding the main loop: terminal input, a UI tick,
//! and the background probe/health/upgrade results, all merged onto one
//! `mpsc` channel (PAT-002) so `main.rs` only ever has to drain one queue.

use std::time::Duration;

use crossterm::event::{Event as CtEvent, EventStream};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio_stream::StreamExt;

use crate::health::{FileHealth, ToolHealth};
use crate::probe::ProbeResult;
use crate::upgrade::UpgradeResult;

#[derive(Debug)]
pub enum AppEvent {
    Term(CtEvent),
    Tick,
    Probe(ProbeResult),
    Health {
        files: Vec<FileHealth>,
        tools: Vec<ToolHealth>,
    },
    Upgrade(UpgradeResult),
    UpgradesDone,
}

/// Spawns the input-reading and tick-generating background tasks and
/// returns the shared sender (cloned into probe/health/upgrade tasks) and
/// the receiver the main loop drains.
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
