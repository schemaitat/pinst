//! The Harness tab: where the agent harness is installed, right now, at
//! both scopes, grouped so project and global cannot be confused, with each
//! asset's install path in view (`REQ-004`, `REQ-005`, `REQ-001`, `REQ-002`).

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};

use super::theme;
use crate::app::{App, HarnessModal, HarnessModalAction, HarnessScopeStat};
use crate::core::harness::asset::{self, AssetKind};
use crate::core::harness::install::receipt::ReceiptScope;
use crate::core::harness::install::state::{AssetKindLabel, AssetState, AssetStatus};
use crate::core::harness::preview::{MAX_PREVIEW_BYTES, Preview};
use crate::core::source::Source;

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

    if area.height < 12 {
        // Not enough room for a preview: the grouped list alone still answers
        // "what is installed where" on a short terminal.
        draw_assets(frame, area, app);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(4), Constraint::Min(0)])
            .split(area);
        draw_scope_summary(frame, chunks[0], app);

        let body = chunks[1];
        if body.width >= 72 && body.height >= 6 {
            let columns = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                .split(body);
            draw_assets(frame, columns[0], app);
            draw_preview(frame, columns[1], app);
        } else {
            draw_assets(frame, body, app);
        }
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
/// selection still indexes `filtered_harness()` so install-all/uninstall-all
/// (`i`/`u`) and the per-row `s`/`x` keys keep working, and is mapped to a
/// display row here by counting the headers it skips.
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

/// One asset, its vendor, its state, and the path it is installed at.
fn asset_line(row: &AssetStatus) -> Line<'static> {
    let (label, style) = state_label(row.state);
    Line::from(vec![
        Span::styled(format!("{:<7} ", row.kind.label()), theme::muted_style()),
        Span::styled(format!("{:<8} ", row.vendor.label()), theme::muted_style()),
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

/// The selected asset's source and installed target, rendered as wrapped,
/// scrollable text. Nothing is executed and nothing large is dumped: binary
/// and oversized content reach here already classified by `preview`.
fn draw_preview(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::muted_style())
        .title(selected_title(app))
        .title_style(theme::accent_style());

    let Some((scope, row)) = app.selected_harness() else {
        frame.render_widget(
            Paragraph::new("select an asset to preview its content")
                .style(theme::muted_style())
                .block(block),
            area,
        );
        return;
    };

    frame.render_widget(
        Paragraph::new(preview_lines(app, scope, row))
            .wrap(Wrap { trim: false })
            .scroll((app.harness_scroll, 0))
            .block(block),
        area,
    );
}

fn selected_title(app: &App) -> String {
    match app.selected_harness() {
        Some((_, row)) => format!(" {} ", row.name),
        None => " Preview ".to_string(),
    }
}

fn preview_lines(app: &App, scope: ReceiptScope, row: &AssetStatus) -> Vec<Line<'static>> {
    let (label, style) = state_label(row.state);
    let mut lines = vec![Line::from(vec![
        Span::styled(format!("{} · ", scope.label()), theme::muted_style()),
        Span::styled(format!("{} ", row.vendor.label()), theme::muted_style()),
        Span::styled(format!("{} ", row.kind.label()), theme::muted_style()),
        Span::styled(row.name.clone(), theme::title_style()),
        Span::raw("  "),
        Span::styled(label.to_string(), style),
    ])];
    lines.push(Line::from(""));

    if row.state == AssetState::Unmanaged {
        lines.push(Line::from(Span::styled(
            "unmanaged · this entry is in the vendor directory but names no .agents/ asset",
            theme::warn_style(),
        )));
    } else {
        let kind: AssetKind = row.kind.into();
        let relative = asset::entry_relative(kind, &row.name);
        let (path, preview) = match &app.harness_source {
            Source::Tree(root) => {
                let path = root.join(&relative);
                (path.display().to_string(), Preview::read(&path))
            }
            Source::Embedded => (
                format!(".agents/{} (embedded in binary)", relative.display()),
                Preview::from_bytes(asset::content(&app.harness_source, &relative)),
            ),
        };
        lines.extend(preview_section("source", path, &preview));
    }

    // A skill's target is its directory; its content lives in SKILL.md. A
    // command is the single file itself.
    let target_entry = match row.kind {
        AssetKindLabel::Skill => row.target.join("SKILL.md"),
        AssetKindLabel::Command => row.target.clone(),
    };
    let target_preview = Preview::read(&target_entry);
    lines.extend(preview_section(
        "target",
        target_entry.display().to_string(),
        &target_preview,
    ));

    lines
}

fn preview_section(label: &str, path: String, preview: &Preview) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        Span::styled(format!("{label}: "), theme::accent_style()),
        Span::styled(path, theme::muted_style()),
    ])];
    match preview {
        Preview::Text {
            body,
            total_bytes,
            truncated,
        } => {
            for line in body.lines() {
                lines.push(Line::from(line.to_string()));
            }
            if *truncated {
                lines.push(Line::from(Span::styled(
                    format!(
                        "… truncated: showing first {MAX_PREVIEW_BYTES} of {total_bytes} bytes"
                    ),
                    theme::warn_style(),
                )));
            }
        }
        Preview::Empty => lines.push(Line::from(Span::styled(
            "(empty file)",
            theme::muted_style(),
        ))),
        Preview::Missing => lines.push(Line::from(Span::styled(
            "(not installed — nothing at this path)",
            theme::bad_style(),
        ))),
        Preview::Binary { bytes } => lines.push(Line::from(Span::styled(
            format!("(binary content, {bytes} bytes)"),
            theme::warn_style(),
        ))),
        Preview::Error(error) => lines.push(Line::from(Span::styled(
            format!("(could not read: {error})"),
            theme::bad_style(),
        ))),
    }
    lines.push(Line::from(""));
    lines
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
    let target = if modal.selected.is_some() {
        " selected asset"
    } else {
        " all assets"
    };
    let text = match modal.steps {
        Some(steps) => format!(
            "{verb}{target} at {scope_label} scope: {steps} step(s)\n\
             drift: overwrite with backup\n\n[Enter] confirm   [Esc] cancel"
        ),
        None => format!("calculating {verb}{target} for {scope_label} scope...\n\n[Esc] cancel"),
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
