//! Installing and uninstalling the harness: turning `.agents/` into
//! `.claude/skills/` and `.claude/commands/` (or their `~/.claude/`
//! equivalents), and undoing exactly that.

pub mod check;
pub mod corpus_init;
pub mod plan;
pub mod receipt;
pub mod record;
pub mod state;
