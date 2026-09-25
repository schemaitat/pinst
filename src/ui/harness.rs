//! The Harness tab: where the agent harness is installed, right now, at
//! both scopes, grouped so project and global cannot be confused, with each
//! asset's install path in view (`REQ-004`, `REQ-005`, `REQ-001`, `REQ-002`).

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph};

use super::theme;
use crate::app::{App, HarnessModal, HarnessModalAction, HarnessScopeStat};
use crate::core::harness::install::receipt::ReceiptScope;
use crate::core::harness::install::state::{AssetState, AssetStatus};

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

    if area.height < 10 {
        draw_assets(frame, area, app);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(4), Constraint::Min(0)])
            .split(area);
        draw_scope_summary(frame, chunks[0], app);
        draw_assets(frame, chunks[1], app);
    }

    if let Some(modal) = &app.harness_modal {
        draw_modal(frame, modal);
    }
}

/// One digest line per scope: label, install root, and the state counts. Two
/// scopes means both fit without scrolling, which is the point of the tab.
fn draw_scope_summary(frame: &mut Frame, area: Rect, app: &App) {
    let lines: Vec<Line> = app.harness_scopes().iter().map(scope_line).collect();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::muted_style())
        .title(" Harness install state ")
        .title_style(theme::accent_style());
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn scope_line(stat: &HarnessScopeStat) -> Line<'static> {
    let installed_style = if stat.total > 0 && stat.installed == stat.total {
        theme::ok_style()
    } else {
        theme::muted_style()
    };
    let mut spans = vec![
        Span::styled(format!("{:<8}", stat.scope.label()), theme::title_style()),
        Span::styled(stat.root.display().to_string(), theme::muted_style()),
        Span::raw("   "),
        Span::styled(
            format!("{}/{} installed", stat.installed, stat.total),
            installed_style,
        ),
    ];
    for (count, label, style) in [
        (stat.missing, "missing", theme::bad_style()),
        (stat.drifted, "drifted", theme::warn_style()),
        (stat.unmanaged, "unmanaged", theme::muted_style()),
    ] {
        if count > 0 {
            spans.push(Span::raw("   "));
            spans.push(Span::styled(format!("{count} {label}"), style));
        }
    }
    Line::from(spans)
}

/// The grouped asset list. Section headers are display-only lines; the
/// selection still indexes `filtered_harness()` so `i`/`u` keep working, and
/// is mapped to a display row here by counting the headers it skips.
fn draw_assets(frame: &mut Frame, area: Rect, app: &App) {
    let filtered = app.filtered_harness();
    let title = if app.search_query.is_empty() {
        format!(" Assets ({}) ", filtered.len())
    } else {
        format!(" Assets ({} match) ", filtered.len())
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::muted_style())
        .title(title)
        .title_style(theme::accent_style());

    if filtered.is_empty() {
        let message = if app.harness_project.is_empty() && app.harness_global.is_empty() {
            "no harness assets found in .agents/"
        } else {
            "nothing matches this search"
        };
        frame.render_widget(
            Paragraph::new(message)
                .style(theme::muted_style())
                .block(block),
            area,
        );
        return;
    }

    let selected = app.harness_selected.min(filtered.len() - 1);
    let mut items: Vec<ListItem> = Vec::new();
    let mut display_selected = None;
    let mut current: Option<ReceiptScope> = None;
    for (index, (scope, row)) in filtered.into_iter().enumerate() {
        if current != Some(scope) {
            let stat = app.harness_scope_stat(scope);
            items.push(ListItem::new(section_line(&stat)));
            current = Some(scope);
        }
        if index == selected {
            display_selected = Some(items.len());
        }
        items.push(ListItem::new(asset_line(row)));
    }

    let mut state = ListState::default();
    state.select(display_selected);
    let list = List::new(items)
        .block(block)
        .highlight_style(theme::selected_style())
        .highlight_symbol("> ");
    frame.render_stateful_widget(list, area, &mut state);
}

fn section_line(stat: &HarnessScopeStat) -> Line<'static> {
    Line::from(Span::styled(
        format!(
            "── {} · {} · {}/{} installed",
            stat.scope.label(),
            stat.root.display(),
            stat.installed,
            stat.total
        ),
        theme::accent_style(),
    ))
}

/// One asset, its state, and the path it is installed at.
fn asset_line(row: &AssetStatus) -> Line<'static> {
    let (label, style) = state_label(row.state);
    Line::from(vec![
        Span::styled(format!("{:<7} ", row.kind.label()), theme::muted_style()),
        Span::styled(format!("{:<20} ", row.name), theme::title_style()),
        Span::styled(format!("{label:<10} "), style),
        Span::styled(row.target.display().to_string(), theme::muted_style()),
    ])
}

fn state_label(state: AssetState) -> (&'static str, Style) {
    match state {
        AssetState::Linked | AssetState::Copied => ("ok", theme::ok_style()),
        AssetState::Missing => ("missing", theme::bad_style()),
        AssetState::Drifted | AssetState::Foreign => ("drifted", theme::warn_style()),
        AssetState::Unmanaged => ("unmanaged", theme::muted_style()),
    }
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
    let text = match modal.steps {
        Some(steps) => format!(
            "{verb} {steps} step(s) at {scope_label} scope?\n\n[Enter] confirm   [Esc] cancel"
        ),
        None => format!("calculating {verb} plan for {scope_label} scope...\n\n[Esc] cancel"),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::accent_style())
        .title(" Confirm ")
        .title_style(theme::title_style());
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(text).block(block), area);
}
