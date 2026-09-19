use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Row, Table, TableState};

use super::theme;
use crate::app::App;
use crate::core::probe::ProbeResult;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let header = Row::new(vec!["", "Tool", "Tags", "Status", "Version"])
        .style(theme::title_style())
        .height(1);

    let filtered = app.filtered_registry();

    let rows: Vec<Row> = filtered
        .iter()
        .copied()
        .map(|spec| {
            let probe = app.probes.get(&spec.name);
            let (icon, status_text, style) = match probe {
                None => ("...", "probing", theme::muted_style()),
                Some(ProbeResult {
                    installed: true, ..
                }) => ("[OK]", "installed", theme::ok_style()),
                Some(ProbeResult {
                    installed: false, ..
                }) => ("[--]", "missing", theme::bad_style()),
            };
            let version = probe
                .and_then(|p| p.version.clone())
                .unwrap_or_else(|| "-".to_string());
            Row::new(vec![
                Cell::from(icon).style(style),
                Cell::from(spec.name.clone()),
                Cell::from(spec.tags.join(",")).style(theme::muted_style()),
                Cell::from(status_text).style(style),
                Cell::from(version),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(5),
        Constraint::Length(22),
        Constraint::Length(16),
        Constraint::Length(11),
        Constraint::Min(10),
    ];

    let title = if app.search_query.is_empty() {
        format!(
            " Tools ({}/{} probed) ",
            app.probes_received, app.probes_expected
        )
    } else {
        format!(
            " Tools ({} match{}, {}/{} probed) ",
            filtered.len(),
            if filtered.len() == 1 { "" } else { "es" },
            app.probes_received,
            app.probes_expected
        )
    };

    let selected = app.overview_selected.min(filtered.len().saturating_sub(1));
    let mut state = TableState::default().with_selected((!filtered.is_empty()).then_some(selected));
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
        .row_highlight_style(theme::selected_style())
        .highlight_symbol("> ");

    frame.render_stateful_widget(table, area, &mut state);
}
