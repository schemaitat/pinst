mod app;
mod editor;
mod event;
mod health;
mod probe;
mod registry;
mod ui;
mod upgrade;

use std::path::PathBuf;

use color_eyre::eyre::Result;
use tokio::sync::mpsc::UnboundedReceiver;

use app::App;
use event::AppEvent;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let registry = registry::load_registry()?;

    let home_dir = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/root"));
    let dotfiles_dir = std::env::var_os("PINST_DOTFILES")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir.join("dotfiles"));

    // Call ratatui::init() only after color_eyre::install() so the
    // terminal-restoring panic hook ratatui installs runs before
    // color_eyre's pretty-printing hook (RISK-003).
    let mut terminal = ratatui::init();

    let (tx, mut rx) = event::start_event_loop();
    let mut app = App::new(tx, registry, dotfiles_dir, home_dir);
    app.start_probing();

    let result = run(&mut terminal, &mut app, &mut rx).await;

    ratatui::restore();
    result
}

async fn run(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    rx: &mut UnboundedReceiver<AppEvent>,
) -> Result<()> {
    terminal.draw(|frame| ui::draw(frame, app))?;

    while let Some(event) = rx.recv().await {
        app.handle_event(event);
        // Drain any other already-queued events (e.g. a burst of probe
        // results landing back-to-back) before redrawing, so ~20 concurrent
        // tool checks don't trigger ~20 separate frame redraws.
        while let Ok(event) = rx.try_recv() {
            app.handle_event(event);
        }

        if let Some(path) = app.pending_editor.take() {
            editor::launch(terminal, &path);
        }

        terminal.draw(|frame| ui::draw(frame, app))?;

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
