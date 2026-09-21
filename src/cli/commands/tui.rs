use color_eyre::eyre::{Result, bail};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::app::App;
use crate::cli::output::{Ctx, ExitCode};
use crate::core;
use crate::editor;
use crate::event::{self, AppEvent};
use crate::ui;

pub async fn run(ctx: &Ctx) -> Result<ExitCode> {
    if ctx.json {
        bail!("the TUI cannot run in --json mode; use `pinst list`/`pinst doctor` instead");
    }

    let loaded = super::load_manifest(ctx)?;
    let home_dir = core::home_dir()?;
    let catalogue = core::docs::Catalogue::load()?;

    // Call ratatui::init() only after color_eyre is installed so the
    // terminal-restoring panic hook runs before color_eyre's pretty printer.
    let mut terminal = ratatui::init();

    let (tx, mut rx) = event::start_event_loop();
    let mut app = App::new(tx, loaded.manifest, home_dir, catalogue);
    app.start_probing();
    app.start_harness_load();

    let result = event_loop(&mut terminal, &mut app, &mut rx).await;

    ratatui::restore();
    result.map(|()| ExitCode::Success)
}

async fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    rx: &mut UnboundedReceiver<AppEvent>,
) -> Result<()> {
    terminal.draw(|frame| ui::draw(frame, app))?;

    while let Some(event) = rx.recv().await {
        app.handle_event(event);
        // Drain anything else already queued (e.g. a burst of probe results
        // landing back-to-back) before redrawing, so ~25 concurrent checks
        // don't trigger ~25 separate frames.
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
