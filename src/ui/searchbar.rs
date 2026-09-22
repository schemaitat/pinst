use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use super::theme;
use crate::app::App;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let (line, border_style) = if app.search_mode {
        (
            Line::from(vec![
                Span::styled("/ ", theme::accent_style()),
                Span::styled(app.search_query.as_str(), theme::title_style()),
                Span::styled("_", theme::accent_style()),
            ]),
            theme::accent_style(),
        )
    } else if !app.search_query.is_empty() {
        (
            Line::from(vec![
                Span::styled("filter: ", theme::muted_style()),
                Span::styled(app.search_query.as_str(), theme::title_style()),
                Span::styled("   [/] edit   [Esc] clear", theme::muted_style()),
            ]),
            theme::muted_style(),
        )
    } else {
        (
            Line::from(Span::styled(
                "[/] search tools, files, packages",
                theme::muted_style(),
            )),
            theme::muted_style(),
        )
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style);
    frame.render_widget(Paragraph::new(line).block(block), area);
}
