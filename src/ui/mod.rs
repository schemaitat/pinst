mod health;
mod overview;
mod searchbar;
mod statusbar;
mod theme;
mod upgrades;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, Tabs};

use crate::app::{App, Tab};

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(frame.area());

    searchbar::draw(frame, chunks[0], app);
    draw_tabs(frame, chunks[1], app);

    match app.tab {
        Tab::Overview => overview::draw(frame, chunks[2], app),
        Tab::Health => health::draw(frame, chunks[2], app),
        Tab::Upgrades => upgrades::draw(frame, chunks[2], app),
    }

    statusbar::draw(frame, chunks[3], app);

    if app.picker_open {
        draw_picker(frame, app);
    }
}

fn draw_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let titles: Vec<Line> = Tab::ALL.iter().map(|t| Line::from(t.title())).collect();
    let selected = Tab::ALL.iter().position(|t| *t == app.tab).unwrap_or(0);
    let tabs = Tabs::new(titles)
        .select(selected)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme::muted_style())
                .title(" pinst - ~/dotfiles manager ")
                .title_style(theme::title_style()),
        )
        .style(theme::muted_style())
        .highlight_style(theme::accent_style());
    frame.render_widget(tabs, area);
}

fn draw_picker(frame: &mut Frame, app: &App) {
    let area = centered_rect(60, 60, frame.area());
    let targets = app.editor_targets();
    let items: Vec<ListItem> = targets
        .iter()
        .map(|t| ListItem::new(t.label.clone()))
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme::accent_style())
                .title(" Open in $EDITOR (Enter to open, Esc to cancel) ")
                .title_style(theme::title_style()),
        )
        .highlight_style(theme::selected_style())
        .highlight_symbol("> ");

    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(app.picker_selected.min(targets.len().saturating_sub(1))));

    frame.render_widget(Clear, area);
    frame.render_stateful_widget(list, area, &mut state);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
