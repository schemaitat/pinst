//! The Harness tab: where the agent harness is installed, right now, at
//! both scopes, in view without moving the cursor (`REQ-004`, `REQ-005`).

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table};

use super::theme;
use crate::app::{App, HarnessModal, HarnessModalAction};
use crate::core::harness::install::receipt::ReceiptScope;
use crate::core::harness::install::state::AssetState;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    if !app.harness_ready {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme::muted_style())
            .title(" Harness ")
            .title_style(theme::accent_style());
        frame.render_widget(
            Paragraph::new("reading .agents/ and both scopes...")
                .style(theme::muted_style())
                .block(block),
            area,
        );
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(0)])
        .split(area);

    draw_banner(frame, chunks[0], app);
    draw_table(frame, chunks[1], app);

    if let Some(modal) = &app.harness_modal {
        draw_modal(frame, modal);
    }
}

/// Two fixed lines — "where is it installed" has exactly two possible
/// answers on this machine, and a reader should get both without scrolling.
fn draw_banner(frame: &mut Frame, area: Rect, app: &App) {
    let project = scope_line(
        "project",
        &app.harness_project_root.display().to_string(),
        &app.harness_project,
    );
    let global = scope_line(
        "global",
        &app.home_dir.display().to_string(),
        &app.harness_global,
    );
    let text = format!("{project}\n{global}");
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::muted_style())
        .title(" Harness install state ")
        .title_style(theme::accent_style());
    frame.render_widget(Paragraph::new(text).block(block), area);
}

fn scope_line(
    label: &str,
    root: &str,
    rows: &[crate::core::harness::install::state::AssetStatus],
) -> String {
    let installed = rows.iter().any(|r| r.state.satisfied());
    if !installed {
        return format!("{label:<8} {root}   not installed");
    }
    let skills = rows
        .iter()
        .filter(|r| r.kind == crate::core::harness::install::state::AssetKindLabel::Skill)
        .count();
    let commands = rows
        .iter()
        .filter(|r| r.kind == crate::core::harness::install::state::AssetKindLabel::Command)
        .count();
    format!("{label:<8} {root}   {skills} skills, {commands} commands")
}

fn draw_table(frame: &mut Frame, area: Rect, app: &App) {
    let header = Row::new(vec!["Scope", "Kind", "Name", "State"])
        .style(theme::title_style())
        .height(1);

    let filtered = app.filtered_harness();
    let rows: Vec<Row> = filtered
        .iter()
        .map(|(scope, row)| {
            let (label, style) = match row.state {
                AssetState::Linked | AssetState::Copied => ("ok", theme::ok_style()),
                AssetState::Missing => ("missing", theme::bad_style()),
                AssetState::Drifted | AssetState::Foreign => ("drifted", theme::warn_style()),
                AssetState::Unmanaged => ("unmanaged", theme::muted_style()),
            };
            Row::new(vec![
                Cell::from(scope.label()),
                Cell::from(row.kind.label()),
                Cell::from(row.name.clone()),
                Cell::from(label).style(style),
            ])
        })
        .collect();

    let title = if app.search_query.is_empty() {
        format!(" Assets ({}) ", filtered.len())
    } else {
        format!(" Assets ({} match) ", filtered.len())
    };

    let widths = [
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Min(20),
        Constraint::Length(10),
    ];
    let mut state = ratatui::widgets::TableState::default();
    if !filtered.is_empty() {
        state.select(Some(app.harness_selected.min(filtered.len() - 1)));
    }
    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme::muted_style())
                .title(title)
                .title_style(theme::accent_style()),
        )
        .row_highlight_style(theme::selected_style());
    frame.render_stateful_widget(table, area, &mut state);
}

fn draw_modal(frame: &mut Frame, modal: &HarnessModal) {
    let area = super::centered_rect(50, 20, frame.area());
    let scope_label = match modal.scope {
        ReceiptScope::Project => "project",
        ReceiptScope::Global => "global",
    };
    let verb = match modal.action {
        HarnessModalAction::Install => "install",
        HarnessModalAction::Uninstall => "uninstall",
    };
    let text = format!(
        "{verb} {} step(s) at {scope_label} scope?\n\n[Enter] confirm   [Esc] cancel",
        modal.steps
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::accent_style())
        .title(" Confirm ")
        .title_style(theme::title_style());
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(text).block(block), area);
}
