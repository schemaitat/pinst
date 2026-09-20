//! The Docs tab: the catalogue, browsable.
//!
//! The dashboard already answers "what is on this machine"; this answers the
//! question that always comes next. The list on the left is ranked by the same
//! search the CLI uses, so `/` here and `pinst docs search` there cannot
//! disagree about what matches.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap};

use super::theme;
use crate::app::App;
use crate::core::docs::page::{PageStatus, ToolDoc};
use crate::core::manifest::Tool;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(28), Constraint::Min(20)])
        .split(area);

    draw_list(frame, chunks[0], app);
    draw_page(frame, chunks[1], app);
}

fn draw_list(frame: &mut Frame, area: Rect, app: &App) {
    let tools = app.filtered_docs();
    let items: Vec<ListItem> = tools
        .iter()
        .map(|tool| {
            let (mark, style) = match app.catalogue.get(&tool.name) {
                Some(page) if page.status == PageStatus::Authored => ("*", theme::ok_style()),
                // A draft is marked, always: a consumer acting on an unverified
                // recipe should know nobody has run it.
                Some(_) => ("~", theme::warn_style()),
                None => (" ", theme::muted_style()),
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{mark} "), style),
                Span::raw(tool.name.clone()),
            ]))
        })
        .collect();

    let title = if app.search_query.is_empty() {
        format!(" Tools ({}) ", tools.len())
    } else {
        format!(" Ranked ({}) ", tools.len())
    };

    let selected = app.docs_selected.min(tools.len().saturating_sub(1));
    let mut state = ListState::default().with_selected((!tools.is_empty()).then_some(selected));
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme::muted_style())
                .title(title)
                .title_style(theme::accent_style()),
        )
        .highlight_style(theme::selected_style())
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_page(frame: &mut Frame, area: Rect, app: &App) {
    let block = |title: String| {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme::muted_style())
            .title(title)
            .title_style(theme::accent_style())
    };

    let Some((tool, page)) = app.selected_doc() else {
        frame.render_widget(
            Paragraph::new("nothing matches this search")
                .style(theme::muted_style())
                .block(block(" Page ".to_string())),
            area,
        );
        return;
    };

    let lines = match page {
        Some(page) => render_page(page),
        None => render_gap(tool),
    };

    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(block(format!(" {} ", tool.name))),
        area,
    );
}

fn render_page(page: &ToolDoc) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let mut badge = page.status.as_str().to_string();
    if let Some(verified) = &page.verified_with {
        badge.push_str(&format!(", verified with {verified}"));
    }
    lines.push(Line::from(Span::styled(
        format!("[{badge}]"),
        if page.status == PageStatus::Authored {
            theme::ok_style()
        } else {
            theme::warn_style()
        },
    )));
    lines.push(Line::from(page.what.clone()));

    if let Some(when) = &page.when {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(when.clone(), theme::muted_style())));
    }

    if !page.recipes.is_empty() {
        lines.push(Line::from(""));
        for recipe in &page.recipes {
            lines.push(Line::from(Span::styled(
                format!("$ {}", recipe.cmd),
                theme::title_style(),
            )));
            lines.push(Line::from(Span::styled(
                format!("  {}", recipe.does),
                theme::muted_style(),
            )));
        }
    }

    if let Some(gotchas) = &page.gotchas {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("Gotchas", theme::warn_style())));
        for line in gotchas.trim().lines() {
            lines.push(Line::from(line.to_string()));
        }
    }

    if !page.see_also.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("See also: {}", page.see_also.join(", ")),
            theme::muted_style(),
        )));
    }

    lines
}

/// What a tool with no page gets. The TUI never captures: running a binary
/// because a cursor moved onto its row is not something a dashboard should do.
fn render_gap(tool: &Tool) -> Vec<Line<'static>> {
    vec![
        Line::from(Span::styled("[no page]", theme::muted_style())),
        Line::from(tool.summary.clone()),
        Line::from(""),
        Line::from(Span::styled(
            format!(
                "pinst docs adopt {} — seed one from the tool's own help",
                tool.name
            ),
            theme::muted_style(),
        )),
    ]
}
