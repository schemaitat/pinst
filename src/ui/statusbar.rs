use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use super::theme;
use crate::app::{App, Tab};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let hints = if app.tab == Tab::Harness {
        "[Tab/1-5] view  [j/k] move  [/] search  [i] install  [u] uninstall  [q] quit"
    } else if app.tab == Tab::Health {
        "[Tab/1-5] view  [←/→] pane  [j/k] move  [/] search  [e] edit config  [q] quit"
    } else if app.tab == Tab::Docs {
        "[Tab/1-5] view  [j/k] tool  [PgUp/PgDn] page  [/] search  [q] quit"
    } else if app.tab == Tab::Upgrades {
        "[Tab/1-5] view  [j/k] move  [/] search  [r] cached  [R] force  [q] quit"
    } else {
        "[Tab/1-5] view  [j/k] move  [/] search  [e] edit config  [q] quit"
    };
    let line = Line::from(vec![
        Span::styled(app.status.as_str(), theme::title_style()),
        Span::raw("   "),
        Span::styled(hints, theme::muted_style()),
    ]);
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(theme::muted_style());
    frame.render_widget(Paragraph::new(line).block(block), area);
}
