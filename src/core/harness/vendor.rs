//! The one table naming where a runtime actually looks for a skill or a
//! command.
//!
//! Nothing outside this file may know that Claude Code reads
//! `.claude/skills/` and `.claude/commands/`. Adding a second runtime is one
//! more `Vendor` variant and one more row in `dirs()`; everything upstream —
//! `install`, `status`, the TUI tab — already iterates `Vendor::ALL` and
//! never spells a vendor path itself (REQ-008).

use schemars::JsonSchema;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Vendor {
    Claude,
}

impl Vendor {
    /// Every runtime pinst knows how to install the harness for. One entry
    /// today, by design — see `ASSUMPTION-003` in the plan this shipped
    /// under.
    pub const ALL: [Vendor; 1] = [Vendor::Claude];

    pub fn label(self) -> &'static str {
        match self {
            Vendor::Claude => "claude",
        }
    }

    pub fn parse(name: &str) -> Option<Vendor> {
        Vendor::ALL.into_iter().find(|v| v.label() == name)
    }

    /// The vendor directory's name at the root of a scope — `.claude` for
    /// both a project root and `$HOME`, which is why `skills_dir`/
    /// `commands_dir` take a root rather than a scope: the vendor layout
    /// underneath is identical either way, only the root differs.
    fn root_name(self) -> &'static str {
        match self {
            Vendor::Claude => ".claude",
        }
    }

    pub fn skills_dir(self, root: &std::path::Path) -> PathBuf {
        root.join(self.root_name()).join("skills")
    }

    pub fn commands_dir(self, root: &std::path::Path) -> PathBuf {
        root.join(self.root_name()).join("commands")
    }
}

impl std::fmt::Display for Vendor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn claude_s_directories_hang_off_dot_claude() {
        let root = Path::new("/repo");
        assert_eq!(
            Vendor::Claude.skills_dir(root),
            Path::new("/repo/.claude/skills")
        );
        assert_eq!(
            Vendor::Claude.commands_dir(root),
            Path::new("/repo/.claude/commands")
        );
    }

    #[test]
    fn parse_round_trips_the_label() {
        for vendor in Vendor::ALL {
            assert_eq!(Vendor::parse(vendor.label()), Some(vendor));
        }
        assert_eq!(Vendor::parse("nonsense"), None);
    }
}
