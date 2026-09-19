use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use super::theme;
use crate::app::App;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let hints =
        "[Tab/1-3] view  [j/k] move  [/] search  [e] edit config  [r] refresh upgrades  [q] quit";
    let line = Line::from(vec![
        Span::styled(app.status.clone(), theme::title_style()),
        Span::raw("   "),
        Span::styled(hints, theme::muted_style()),
    ]);
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(theme::muted_style());
    frame.render_widget(Paragraph::new(line).block(block), area);
}
