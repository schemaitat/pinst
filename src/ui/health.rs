use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table};

use super::theme;
use crate::app::App;
use crate::core::configs::FileState;
use crate::core::doctor::Severity;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    if !app.health_ready {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme::muted_style())
            .title(" Health ")
            .title_style(theme::accent_style());
        frame.render_widget(
            Paragraph::new("waiting for tool probing to finish before diagnosing...")
                .style(theme::muted_style())
                .block(block),
            area,
        );
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    draw_findings(frame, chunks[0], app);
    draw_configs(frame, chunks[1], app);
}

fn draw_findings(frame: &mut Frame, area: Rect, app: &App) {
    let header = Row::new(vec!["Severity", "Finding"])
        .style(theme::title_style())
        .height(1);

    let filtered = app.filtered_findings();
    let rows: Vec<Row> = filtered
        .iter()
        .copied()
        .map(|finding| {
            let (label, style) = match finding.severity {
                Severity::Error => ("error", theme::bad_style()),
                Severity::Warning => ("warn", theme::warn_style()),
                Severity::Info => ("info", theme::muted_style()),
            };
            Row::new(vec![
                Cell::from(label).style(style),
                Cell::from(finding.message.clone()),
            ])
        })
        .collect();

    let title = if app.search_query.is_empty() {
        format!(" Findings ({}) ", app.findings.len())
    } else {
        format!(" Findings ({} match) ", filtered.len())
    };

    let widths = [Constraint::Length(9), Constraint::Min(20)];
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

fn draw_configs(frame: &mut Frame, area: Rect, app: &App) {
    let header = Row::new(vec!["Config", "State"])
        .style(theme::title_style())
        .height(1);

    let filtered = app.filtered_configs();
    let rows: Vec<Row> = filtered
        .iter()
        .copied()
        .map(|file| {
            let (label, style) = match file.state {
                FileState::Linked => ("linked", theme::ok_style()),
                FileState::Materialized => ("ok", theme::ok_style()),
                FileState::Missing => ("missing", theme::bad_style()),
                FileState::Drifted => ("drifted", theme::warn_style()),
                FileState::Foreign => ("foreign", theme::warn_style()),
                FileState::Unrenderable => ("no values", theme::bad_style()),
            };
            Row::new(vec![
                Cell::from(file.path.clone()),
                Cell::from(label).style(style),
            ])
        })
        .collect();

    let ok = app
        .configs
        .iter()
        .filter(|f| matches!(f.state, FileState::Linked | FileState::Materialized))
        .count();
    let title = format!(" Configs ({}/{} ok) ", ok, app.configs.len());

    let widths = [Constraint::Min(16), Constraint::Length(10)];
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
