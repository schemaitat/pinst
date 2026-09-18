//! A small Catppuccin-flavored palette (matching the oh-my-posh theme these
//! dotfiles already use) so pinst's chrome feels consistent with the rest of
//! the terminal setup it manages.

use ratatui::style::{Color, Modifier, Style};

pub const SURFACE: Color = Color::Rgb(49, 50, 68);
pub const TEXT: Color = Color::Rgb(205, 214, 244);
pub const SUBTEXT: Color = Color::Rgb(166, 173, 200);
pub const ACCENT: Color = Color::Rgb(137, 180, 250);
pub const GREEN: Color = Color::Rgb(166, 227, 161);
pub const YELLOW: Color = Color::Rgb(249, 226, 175);
pub const RED: Color = Color::Rgb(243, 139, 168);
pub const MAUVE: Color = Color::Rgb(203, 166, 247);

pub fn title_style() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn selected_style() -> Style {
    Style::default()
        .bg(SURFACE)
        .fg(TEXT)
        .add_modifier(Modifier::BOLD)
}

pub fn ok_style() -> Style {
    Style::default().fg(GREEN)
}

pub fn warn_style() -> Style {
    Style::default().fg(YELLOW)
}

pub fn bad_style() -> Style {
    Style::default().fg(RED)
}

pub fn muted_style() -> Style {
    Style::default().fg(SUBTEXT)
}

pub fn accent_style() -> Style {
    Style::default().fg(MAUVE)
}
