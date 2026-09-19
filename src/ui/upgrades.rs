use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table};

use super::theme;
use crate::app::App;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    if !app.upgrades_ever_run {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme::muted_style())
            .title(" Upgrades ")
            .title_style(theme::accent_style());
        frame.render_widget(
            Paragraph::new(
                "press r to check for upgrades (apt/cargo/GitHub releases/nvm, cached for 24h)",
            )
            .style(theme::muted_style())
            .block(block),
            area,
        );
        return;
    }

    let header = Row::new(vec!["Tool", "Current", "Latest", "Status"])
        .style(theme::title_style())
        .height(1);

    let filtered = app.filtered_registry();

    let rows: Vec<Row> = filtered
        .iter()
        .copied()
        .map(|spec| {
            let result = app.upgrades.get(&spec.name);
            let current = result
                .and_then(|r| r.current.clone())
                .unwrap_or_else(|| "-".to_string());
            let (latest, status_text, style) = match result {
                None => ("-".to_string(), "checking...", theme::muted_style()),
                Some(r) if r.upgrade_available => (
                    r.latest.clone().unwrap_or_else(|| "-".to_string()),
                    "upgrade available",
                    theme::warn_style(),
                ),
                Some(r) if r.latest.is_some() => (
                    r.latest.clone().unwrap_or_else(|| "-".to_string()),
                    "up to date",
                    theme::ok_style(),
                ),
                Some(_) => ("-".to_string(), "unknown", theme::muted_style()),
            };
            Row::new(vec![
                Cell::from(spec.name.clone()),
                Cell::from(current),
                Cell::from(latest),
                Cell::from(status_text).style(style),
            ])
        })
        .collect();

    let title = match (app.upgrades_loading, app.search_query.is_empty()) {
        (true, true) => format!(
            " Upgrades ({}/{} checked) ",
            app.upgrades.len(),
            app.registry.len()
        ),
        (true, false) => format!(
            " Upgrades ({} match{}, {}/{} checked) ",
            filtered.len(),
            if filtered.len() == 1 { "" } else { "es" },
            app.upgrades.len(),
            app.registry.len()
        ),
        (false, true) => " Upgrades ".to_string(),
        (false, false) => format!(
            " Upgrades ({} match{}) ",
            filtered.len(),
            if filtered.len() == 1 { "" } else { "es" }
        ),
    };

    let widths = [
        Constraint::Length(22),
        Constraint::Length(14),
        Constraint::Length(14),
        Constraint::Min(16),
    ];
    let table = Table::new(rows, widths).header(header).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme::muted_style())
            .title(title)
            .title_style(theme::accent_style()),
    );
    frame.render_widget(table, area);
}
