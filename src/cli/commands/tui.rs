use crate::app::App;
use crate::cli::output::{Ctx, ExitCode};
use crate::core;
use crate::editor;
use crate::event::{self, AppEvent};
use crate::ui;
use color_eyre::eyre::{Result, bail};

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

    let mut events = event::EventRuntime::start();
    let mut app = App::new(events.sender(), loaded.manifest, home_dir, catalogue);
    app.start_probing();
    app.start_harness_load();

    let result = event_loop(&mut terminal, &mut app, &mut events).await;

    ratatui::restore();
    result.map(|()| ExitCode::Success)
}

async fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    events: &mut event::EventRuntime,
) -> Result<()> {
    terminal.draw(|frame| ui::draw(frame, app))?;

    while let Some(event) = events.recv().await {
        let mut outcome = app.handle_event(event);
        // Drain anything else already queued (e.g. a burst of probe results
        // landing back-to-back) before redrawing, so ~25 concurrent checks
        // don't trigger ~25 separate frames.
        while let Ok(event) = events.try_recv() {
            let next = app.handle_event(event);
            outcome.redraw |= next.redraw;
            outcome.launch_editor |= next.launch_editor;
            outcome.quit |= next.quit;
        }

        if outcome.quit {
            break;
        }

        if outcome.launch_editor
            && let Some(path) = app.pending_editor.take()
        {
            events.pause_input().await;
            let result = editor::launch(terminal, &path);
            while let Ok(event) = events.try_recv() {
                if !matches!(event, AppEvent::Term(_)) {
                    let next = app.handle_event(event);
                    outcome.redraw |= next.redraw;
                    outcome.quit |= next.quit;
                }
            }
            events.resume_input();
            app.on_editor_closed(result);
            outcome.redraw = true;
        }

        if outcome.redraw {
            terminal.draw(|frame| ui::draw(frame, app))?;
        }
    }

    Ok(())
}
