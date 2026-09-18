use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table};

use super::theme;
use crate::app::App;
use crate::health::{LinkStatus, ToolPathStatus};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    if !app.health_ready {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme::muted_style())
            .title(" Dotfiles Health ")
            .title_style(theme::accent_style());
        frame.render_widget(
            Paragraph::new("waiting for tool probing to finish before running the health check...")
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

    draw_files(frame, chunks[0], app);
    draw_tools(frame, chunks[1], app);
}

fn draw_files(frame: &mut Frame, area: Rect, app: &App) {
    let header = Row::new(vec!["Package", "File", "Status"])
        .style(theme::title_style())
        .height(1);

    let filtered = app.filtered_health_files();

    let rows: Vec<Row> = filtered
        .iter()
        .copied()
        .map(|file| {
            let (text, style) = match file.status {
                LinkStatus::Ok => ("ok", theme::ok_style()),
                LinkStatus::Missing => ("missing", theme::warn_style()),
                LinkStatus::Broken => ("broken", theme::bad_style()),
                LinkStatus::Conflict => ("conflict", theme::bad_style()),
            };
            Row::new(vec![
                Cell::from(file.package.clone()).style(theme::muted_style()),
                Cell::from(file.relative_path.clone()),
                Cell::from(text).style(style),
            ])
        })
        .collect();

    let ok_count = app
        .health_files
        .iter()
        .filter(|f| f.status == LinkStatus::Ok)
        .count();
    let title = if app.search_query.is_empty() {
        format!(" Stow Symlinks ({}/{} ok) ", ok_count, app.health_files.len())
    } else {
        format!(
            " Stow Symlinks ({} match{}, {}/{} ok) ",
            filtered.len(),
            if filtered.len() == 1 { "" } else { "es" },
            ok_count,
            app.health_files.len()
        )
    };

    let widths = [
        Constraint::Length(10),
        Constraint::Min(16),
        Constraint::Length(10),
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

fn draw_tools(frame: &mut Frame, area: Rect, app: &App) {
    let header = Row::new(vec!["Tool", "PATH"])
        .style(theme::title_style())
        .height(1);

    let filtered = app.filtered_health_tools();

    let rows: Vec<Row> = filtered
        .iter()
        .copied()
        .map(|tool| {
            let (text, style) = match tool.status {
                ToolPathStatus::OnPath => ("on PATH", theme::ok_style()),
                ToolPathStatus::Missing => ("missing", theme::bad_style()),
                ToolPathStatus::Unknown => ("unknown", theme::muted_style()),
            };
            Row::new(vec![
                Cell::from(tool.tool.clone()),
                Cell::from(text).style(style),
            ])
        })
        .collect();

    let title = if app.search_query.is_empty() {
        " Tool PATH ".to_string()
    } else {
        format!(
            " Tool PATH ({} match{}) ",
            filtered.len(),
            if filtered.len() == 1 { "" } else { "es" }
        )
    };

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
