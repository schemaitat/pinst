use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, TableState};

use super::theme;
use crate::app::App;
use crate::core::upgrade::UpgradeCheckState;

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
            let check = app.upgrades.get(&spec.name);
            let result = check.and_then(|check| check.result.as_ref());
            let current = result
                .and_then(|result| result.current.as_deref())
                .unwrap_or("-");
            let (latest, status_text, style) = match check {
                None if app.upgrades_loading => ("-", "checking...", theme::muted_style()),
                None => ("-", "unavailable", theme::muted_style()),
                Some(check) if check.state == UpgradeCheckState::Unsupported => {
                    ("-", "unsupported", theme::muted_style())
                }
                Some(check) if check.state == UpgradeCheckState::NotApplicable => {
                    ("-", "not checkable", theme::muted_style())
                }
                Some(check) if check.state == UpgradeCheckState::Unavailable => {
                    ("-", "unavailable", theme::muted_style())
                }
                Some(check) => {
                    let result = check.result.as_ref().expect("completed check has a result");
                    let latest = result.latest.as_deref().unwrap_or("-");
                    let cached = check.state == UpgradeCheckState::Cached;
                    if result.upgrade_available {
                        (
                            latest,
                            if cached {
                                "cached upgrade"
                            } else {
                                "upgrade available"
                            },
                            theme::warn_style(),
                        )
                    } else {
                        (
                            latest,
                            if cached {
                                "cached current"
                            } else {
                                "up to date"
                            },
                            theme::ok_style(),
                        )
                    }
                }
            };
            Row::new(vec![
                Cell::from(spec.name.as_str()),
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
        Constraint::Min(18),
    ];
    let selected = app.upgrades_selected.min(filtered.len().saturating_sub(1));
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
