//! The interactive picker for `pinst harness install`: which scope, which
//! skills, which commands.
//!
//! Split in two on purpose. [`Picker`] is a state machine with no terminal
//! in it — key codes in, a new state out — so the whole flow is tested by
//! calling functions, not by driving a fake terminal (LESSON-018: a test
//! that wants to override ambient state is telling you to make it a
//! parameter). [`run`] is the thin ratatui shell around it, entered with
//! `ratatui::init()` and left with `ratatui::restore()` exactly the way
//! `editor::launch` suspends and resumes the dashboard — except there is no
//! dashboard running yet here, so this owns the whole terminal for its
//! duration rather than borrowing one.

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph};

use crate::cli::ScopeArg;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PickStep {
    Scope,
    Skills,
    Commands,
    Confirm,
}

/// The picker's whole state: which step it is on, the current scope choice,
/// and a checklist for skills and commands. Never touches a terminal —
/// `handle_key` is a pure function from a key code to a new state.
#[derive(Debug, Clone)]
pub struct Picker {
    step: PickStep,
    scope: ScopeArg,
    skills: Vec<(String, bool)>,
    skill_cursor: usize,
    commands: Vec<(String, bool)>,
    command_cursor: usize,
    cancelled: bool,
    confirmed: bool,
}

impl Picker {
    /// `skills`/`commands` arrive pre-ticked with whatever is already
    /// installed at `initial_scope` — the question a returning user answers
    /// is "what *should* be installed", not "what would you like to add",
    /// and those differ the moment anything already is.
    pub fn new(
        initial_scope: ScopeArg,
        skills: Vec<(String, bool)>,
        commands: Vec<(String, bool)>,
    ) -> Self {
        Self {
            step: PickStep::Scope,
            scope: initial_scope,
            skills,
            skill_cursor: 0,
            commands,
            command_cursor: 0,
            cancelled: false,
            confirmed: false,
        }
    }

    pub fn cancelled(&self) -> bool {
        self.cancelled
    }

    pub fn confirmed(&self) -> bool {
        self.confirmed
    }

    pub fn scope(&self) -> ScopeArg {
        self.scope
    }

    pub fn selected_skills(&self) -> Vec<String> {
        self.skills
            .iter()
            .filter(|(_, on)| *on)
            .map(|(name, _)| name.clone())
            .collect()
    }

    pub fn selected_commands(&self) -> Vec<String> {
        self.commands
            .iter()
            .filter(|(_, on)| *on)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Advances the state machine by one key. `Esc` cancels from any step;
    /// everything else is scoped to the step currently showing.
    pub fn handle_key(&mut self, code: KeyCode) {
        if self.cancelled || self.confirmed {
            return;
        }
        if code == KeyCode::Esc {
            self.cancelled = true;
            return;
        }
        match self.step {
            PickStep::Scope => self.handle_scope_key(code),
            PickStep::Skills => Self::handle_list_key(
                code,
                &mut self.skills,
                &mut self.skill_cursor,
                &mut self.step,
                PickStep::Commands,
            ),
            PickStep::Commands => Self::handle_list_key(
                code,
                &mut self.commands,
                &mut self.command_cursor,
                &mut self.step,
                PickStep::Confirm,
            ),
            PickStep::Confirm => {
                if code == KeyCode::Enter {
                    self.confirmed = true;
                }
            }
        }
    }

    fn handle_scope_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Up | KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('k') => {
                self.scope = match self.scope {
                    ScopeArg::Project => ScopeArg::Global,
                    _ => ScopeArg::Project,
                };
            }
            KeyCode::Enter => self.step = PickStep::Skills,
            _ => {}
        }
    }

    /// Shared by the skills and commands steps, which behave identically:
    /// move a cursor, toggle the item under it, select everything with `a`,
    /// advance with `Enter`.
    fn handle_list_key(
        code: KeyCode,
        items: &mut [(String, bool)],
        cursor: &mut usize,
        step: &mut PickStep,
        next: PickStep,
    ) {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                if !items.is_empty() {
                    *cursor = (*cursor + items.len() - 1) % items.len();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !items.is_empty() {
                    *cursor = (*cursor + 1) % items.len();
                }
            }
            KeyCode::Char(' ') => {
                if let Some(item) = items.get_mut(*cursor) {
                    item.1 = !item.1;
                }
            }
            KeyCode::Char('a') => {
                for item in items.iter_mut() {
                    item.1 = true;
                }
            }
            KeyCode::Enter => *step = next,
            _ => {}
        }
    }
}

/// What the picker resolved to: the scope, and the ticked skills/commands.
pub struct Picked {
    pub scope: ScopeArg,
    pub skills: Vec<String>,
    pub commands: Vec<String>,
}

/// Runs the picker at a real terminal. Returns `None` on cancel, `Some` on
/// confirm. The caller is responsible for never reaching this in `--json`
/// mode or on a non-interactive stream — see `Ctx::interactive`, the same
/// predicate `confirm` already relies on for the same reason: a prompt on a
/// pipe is how an agent deadlocks, not an error either side can recover
/// from.
pub fn run(
    initial_scope: ScopeArg,
    skills: Vec<(String, bool)>,
    commands: Vec<(String, bool)>,
) -> color_eyre::eyre::Result<Option<Picked>> {
    let mut picker = Picker::new(initial_scope, skills, commands);

    let mut terminal = ratatui::init();
    let result = (|| -> color_eyre::eyre::Result<()> {
        terminal.draw(|frame| draw(frame, &picker))?;
        loop {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                picker.handle_key(key.code);
                terminal.draw(|frame| draw(frame, &picker))?;
                if picker.cancelled() || picker.confirmed() {
                    break;
                }
            }
        }
        Ok(())
    })();
    ratatui::restore();
    result?;

    if picker.confirmed() {
        Ok(Some(Picked {
            scope: picker.scope(),
            skills: picker.selected_skills(),
            commands: picker.selected_commands(),
        }))
    } else {
        Ok(None)
    }
}

fn draw(frame: &mut Frame, picker: &Picker) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(area);

    let title = Paragraph::new("pinst harness install").block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" What to install "),
    );
    frame.render_widget(title, chunks[0]);

    match picker.step {
        PickStep::Scope => draw_scope(frame, chunks[1], picker),
        PickStep::Skills => draw_list(
            frame,
            chunks[1],
            "Skills",
            &picker.skills,
            picker.skill_cursor,
        ),
        PickStep::Commands => draw_list(
            frame,
            chunks[1],
            "Commands",
            &picker.commands,
            picker.command_cursor,
        ),
        PickStep::Confirm => draw_confirm(frame, chunks[1], picker),
    }

    let hint = match picker.step {
        PickStep::Scope => "[j/k] choose  [Enter] next  [Esc] cancel",
        PickStep::Skills | PickStep::Commands => {
            "[j/k] move  [space] toggle  [a] all  [Enter] next  [Esc] cancel"
        }
        PickStep::Confirm => "[Enter] install  [Esc] cancel",
    };
    frame.render_widget(Paragraph::new(hint), chunks[2]);
}

fn draw_scope(frame: &mut Frame, area: Rect, picker: &Picker) {
    let items: Vec<ListItem> = [ScopeArg::Project, ScopeArg::Global]
        .into_iter()
        .map(|scope| {
            let label = match scope {
                ScopeArg::Project => "project (./.claude)",
                ScopeArg::Global => "global (~/.claude)",
                ScopeArg::Both => unreachable!("the picker never offers both"),
            };
            let selected = scope_matches(scope, picker.scope);
            let mark = if selected { "> " } else { "  " };
            let style = if selected {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(Span::styled(format!("{mark}{label}"), style)))
        })
        .collect();
    frame.render_widget(
        List::new(items).block(Block::default().borders(Borders::ALL)),
        area,
    );
}

fn scope_matches(a: ScopeArg, b: ScopeArg) -> bool {
    matches!(
        (a, b),
        (ScopeArg::Project, ScopeArg::Project) | (ScopeArg::Global, ScopeArg::Global)
    )
}

fn draw_list(frame: &mut Frame, area: Rect, title: &str, items: &[(String, bool)], cursor: usize) {
    let rows: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, (name, on))| {
            let cursor_mark = if i == cursor { ">" } else { " " };
            let check = if *on { "[x]" } else { "[ ]" };
            ListItem::new(format!("{cursor_mark} {check} {name}"))
        })
        .collect();
    frame.render_widget(
        List::new(rows).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {title} ")),
        ),
        area,
    );
}

fn draw_confirm(frame: &mut Frame, area: Rect, picker: &Picker) {
    let scope_label = match picker.scope {
        ScopeArg::Project => "project",
        ScopeArg::Global => "global",
        ScopeArg::Both => unreachable!("the picker never offers both"),
    };
    let text = format!(
        "install {} skill(s) and {} command(s) into {scope_label} scope?",
        picker.selected_skills().len(),
        picker.selected_commands().len()
    );
    frame.render_widget(
        Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(" Confirm ")),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(names: &[&str]) -> Vec<(String, bool)> {
        names.iter().map(|n| (n.to_string(), false)).collect()
    }

    #[test]
    fn the_default_scope_is_whatever_the_caller_passed_in() {
        let picker = Picker::new(ScopeArg::Global, items(&["a"]), items(&["b"]));
        assert_eq!(picker.scope(), ScopeArg::Global);
    }

    #[test]
    fn arrow_keys_toggle_between_the_two_scopes() {
        let mut picker = Picker::new(ScopeArg::Project, items(&[]), items(&[]));
        picker.handle_key(KeyCode::Down);
        assert_eq!(picker.scope(), ScopeArg::Global);
        picker.handle_key(KeyCode::Up);
        assert_eq!(picker.scope(), ScopeArg::Project);
    }

    #[test]
    fn space_toggles_the_item_under_the_cursor_only() {
        let mut picker = Picker::new(ScopeArg::Project, items(&["a", "b"]), items(&[]));
        picker.handle_key(KeyCode::Enter); // scope -> skills
        picker.handle_key(KeyCode::Char(' '));
        assert_eq!(picker.selected_skills(), vec!["a".to_string()]);
        picker.handle_key(KeyCode::Down);
        picker.handle_key(KeyCode::Char(' '));
        assert_eq!(
            picker.selected_skills(),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn a_selects_every_remaining_item() {
        let mut picker = Picker::new(ScopeArg::Project, items(&["a", "b", "c"]), items(&[]));
        picker.handle_key(KeyCode::Enter);
        picker.handle_key(KeyCode::Char('a'));
        assert_eq!(picker.selected_skills().len(), 3);
    }

    #[test]
    fn the_full_flow_reaches_confirmed_with_the_right_selection() {
        let mut picker = Picker::new(
            ScopeArg::Project,
            items(&["skill-a", "skill-b"]),
            items(&["cmd-a"]),
        );
        picker.handle_key(KeyCode::Enter); // scope -> skills
        picker.handle_key(KeyCode::Char(' ')); // tick skill-a
        picker.handle_key(KeyCode::Enter); // skills -> commands
        picker.handle_key(KeyCode::Char('a')); // all commands
        picker.handle_key(KeyCode::Enter); // commands -> confirm
        assert!(!picker.confirmed());
        picker.handle_key(KeyCode::Enter); // confirm
        assert!(picker.confirmed());
        assert_eq!(picker.selected_skills(), vec!["skill-a".to_string()]);
        assert_eq!(picker.selected_commands(), vec!["cmd-a".to_string()]);
    }

    #[test]
    fn esc_cancels_from_any_step_and_further_keys_are_ignored() {
        let mut picker = Picker::new(ScopeArg::Project, items(&["a"]), items(&["b"]));
        picker.handle_key(KeyCode::Enter);
        picker.handle_key(KeyCode::Esc);
        assert!(picker.cancelled());
        assert!(!picker.confirmed());
        // Nothing after cancel changes the state.
        picker.handle_key(KeyCode::Char(' '));
        assert!(picker.selected_skills().is_empty());
    }

    #[test]
    fn cursor_movement_wraps_at_both_ends() {
        let mut picker = Picker::new(ScopeArg::Project, items(&["a", "b"]), items(&[]));
        picker.handle_key(KeyCode::Enter);
        picker.handle_key(KeyCode::Up); // wraps from 0 to the last item
        picker.handle_key(KeyCode::Char(' '));
        assert_eq!(picker.selected_skills(), vec!["b".to_string()]);
    }
}
