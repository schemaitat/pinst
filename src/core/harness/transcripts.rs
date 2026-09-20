//! Invocation counts, from a runtime's own session transcripts.
//!
//! **A second opinion, never the system of record.** Off unless asked for,
//! and nothing in `qc` may ever depend on it: this reads an undocumented
//! format owned by someone else's release cycle, outside the repo, on one
//! vendor's machine. Counting invocations alone would have graded
//! `plan-implement` as dead — zero invocations of either shape while it had
//! written a complete run log (LESSON-012).
//!
//! Two record shapes, because a skill has two front doors: a `Skill` tool
//! call carrying `input.skill`, and a slash command, which appears as a
//! `<command-name>` marker in user content. Counting only the first misses
//! `/cc` and `/pr` entirely.
//!
//! # Two rules this module must not break
//!
//! **Derived counts only.** A skill name reaches the output only after
//! matching a directory in `.agents/skills`, so nothing typed into a
//! conversation can be printed — no quoted line, no path, no argument
//! string. Dates are truncated to a day. Nothing here writes into `.ash/`.
//!
//! **Everything fails soft.** An unreadable directory, a malformed line or
//! an unrecognised schema degrades to `unmeasured` and never changes an exit
//! code.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::corpus::Corpus;

/// What the report shows for one skill.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Seen {
    pub count: u64,
    /// The most recent day it was invoked, `YYYY-MM-DD`.
    pub last: Option<String>,
}

/// Counts invocations per skill, or `None` when the directory could not be
/// read at all — which the caller reports as `unmeasured`.
pub fn count(dir: &Path, corpus: &Corpus) -> Option<BTreeMap<String, Seen>> {
    if !dir.is_dir() {
        return None;
    }
    let known = known_skills(&corpus.root.skills_dir())?;
    let commands = command_map(&corpus.root.commands_dir(), &known);

    let mut counts: BTreeMap<String, Seen> = BTreeMap::new();
    for transcript in transcripts(dir) {
        let Ok(text) = std::fs::read_to_string(&transcript) else {
            continue;
        };
        for line in text.lines() {
            // Cheap reject before paying for a JSON parse: the overwhelming
            // majority of transcript lines mention neither.
            if !line.contains("Skill") && !line.contains("command-name") {
                continue;
            }
            let Ok(record) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let day = record["timestamp"]
                .as_str()
                .filter(|stamp| stamp.len() >= 10)
                .map(|stamp| stamp[..10].to_string());
            scan(
                &record["message"]["content"],
                &known,
                &commands,
                &day,
                &mut counts,
            );
        }
    }
    Some(counts)
}

fn scan(
    content: &serde_json::Value,
    known: &BTreeSet<String>,
    commands: &BTreeMap<String, String>,
    day: &Option<String>,
    counts: &mut BTreeMap<String, Seen>,
) {
    if let Some(text) = content.as_str() {
        scan_text(text, commands, day, counts);
        return;
    }
    let Some(blocks) = content.as_array() else {
        return;
    };
    for block in blocks {
        match block["type"].as_str() {
            Some("tool_use") if block["name"].as_str() == Some("Skill") => {
                if let Some(skill) = block["input"]["skill"].as_str()
                    && known.contains(skill)
                {
                    bump(skill, day, counts);
                }
            }
            Some("text") => {
                if let Some(text) = block["text"].as_str() {
                    scan_text(text, commands, day, counts);
                }
            }
            _ => {}
        }
    }
}

/// Finds `<command-name>/foo</command-name>` markers and maps each to the
/// skill its command file names.
fn scan_text(
    text: &str,
    commands: &BTreeMap<String, String>,
    day: &Option<String>,
    counts: &mut BTreeMap<String, Seen>,
) {
    let mut rest = text;
    while let Some(start) = rest.find("<command-name>") {
        let after = &rest[start + "<command-name>".len()..];
        let Some(end) = after.find("</command-name>") else {
            return;
        };
        let name = after[..end].trim_start_matches('/');
        if let Some(skill) = commands.get(name) {
            bump(skill, day, counts);
        }
        rest = &after[end..];
    }
}

fn bump(skill: &str, day: &Option<String>, counts: &mut BTreeMap<String, Seen>) {
    let seen = counts.entry(skill.to_string()).or_default();
    seen.count += 1;
    if let Some(day) = day
        && seen.last.as_ref().is_none_or(|last| day > last)
    {
        seen.last = Some(day.clone());
    }
}

/// The set of names that may ever be printed.
fn known_skills(dir: &Path) -> Option<BTreeSet<String>> {
    let entries = std::fs::read_dir(dir).ok()?;
    Some(
        entries
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect(),
    )
}

/// `cc` -> `conventional-commits`, read from the path each command file tells
/// the agent to open.
fn command_map(dir: &Path, known: &BTreeSet<String>) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return map;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e != "md").unwrap_or(true) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(skill) = referenced_skill(&text, known) else {
            continue;
        };
        let command = path.file_stem().unwrap().to_string_lossy().to_string();
        map.insert(command, skill);
    }
    map
}

fn referenced_skill(text: &str, known: &BTreeSet<String>) -> Option<String> {
    let mut rest = text;
    while let Some(at) = rest.find(".agents/skills/") {
        let after = &rest[at + ".agents/skills/".len()..];
        let name: String = after
            .chars()
            .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
            .collect();
        if known.contains(&name) {
            return Some(name);
        }
        rest = after;
    }
    None
}

/// `<dir>/*/*.jsonl`, the layout the runtime writes.
fn transcripts(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let Ok(projects) = std::fs::read_dir(dir) else {
        return found;
    };
    for project in projects.flatten() {
        let Ok(files) = std::fs::read_dir(project.path()) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::harness::root::CorpusRoot;

    /// A corpus with two skills and one slash command pointing at one of
    /// them, plus a transcript directory to fill.
    fn fixture() -> (tempfile::TempDir, Corpus) {
        let dir = tempfile::tempdir().unwrap();
        for skill in ["plan-write", "conventional-commits"] {
            std::fs::create_dir_all(dir.path().join(".agents/skills").join(skill)).unwrap();
        }
        std::fs::create_dir_all(dir.path().join(".claude/commands")).unwrap();
        std::fs::write(
            dir.path().join(".claude/commands/cc.md"),
            "Read `.agents/skills/conventional-commits/SKILL.md` and follow it.\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join(".ash/plans")).unwrap();
        let corpus = Corpus::load(CorpusRoot::new(dir.path())).unwrap();
        (dir, corpus)
    }

    fn transcript(root: &Path, lines: &[&str]) -> std::path::PathBuf {
        let project = root.join("projects/some-project");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join("session.jsonl"), lines.join("\n")).unwrap();
        root.join("projects")
    }

    /// Both front doors, because counting only the tool call misses every
    /// slash command — which is how `/cc` and `/pr` came out as zero.
    #[test]
    fn counts_tool_calls_and_slash_commands_alike() {
        let (dir, corpus) = fixture();
        let transcripts = transcript(
            dir.path(),
            &[
                r#"{"timestamp":"2026-09-18T10:00:00Z","message":{"content":[{"type":"tool_use","name":"Skill","input":{"skill":"plan-write"}}]}}"#,
                r#"{"timestamp":"2026-09-19T10:00:00Z","message":{"content":[{"type":"tool_use","name":"Skill","input":{"skill":"plan-write"}}]}}"#,
                r#"{"timestamp":"2026-09-20T10:00:00Z","message":{"content":"<command-name>/cc</command-name> please"}}"#,
            ],
        );

        let counts = count(&transcripts, &corpus).unwrap();
        assert_eq!(counts["plan-write"].count, 2);
        assert_eq!(counts["plan-write"].last.as_deref(), Some("2026-09-19"));
        assert_eq!(counts["conventional-commits"].count, 1);
        assert_eq!(
            counts["conventional-commits"].last.as_deref(),
            Some("2026-09-20")
        );
    }

    /// SEC: a name reaches the output only after matching a real skill
    /// directory, so nothing typed into a conversation can be printed.
    #[test]
    fn a_name_that_is_not_a_known_skill_is_never_emitted() {
        let (dir, corpus) = fixture();
        let transcripts = transcript(
            dir.path(),
            &[
                r#"{"timestamp":"2026-09-20T10:00:00Z","message":{"content":[{"type":"tool_use","name":"Skill","input":{"skill":"../../etc/passwd"}}]}}"#,
                r#"{"timestamp":"2026-09-20T10:00:00Z","message":{"content":"<command-name>/not-a-command</command-name>"}}"#,
            ],
        );
        assert!(count(&transcripts, &corpus).unwrap().is_empty());
    }

    /// Fails soft, in every direction: the whole path is opt-in and nothing
    /// in `qc` depends on it.
    #[test]
    fn malformed_input_degrades_rather_than_failing() {
        let (dir, corpus) = fixture();
        let transcripts = transcript(
            dir.path(),
            &[
                "not json at all, but it mentions Skill",
                r#"{"message":{"content":[{"type":"tool_use","name":"Skill"}]}}"#,
                r#"{"message":{"content":42}}"#,
                r#"{"timestamp":"2026-09-20T10:00:00Z","message":{"content":[{"type":"tool_use","name":"Skill","input":{"skill":"plan-write"}}]}}"#,
            ],
        );
        let counts = count(&transcripts, &corpus).unwrap();
        assert_eq!(counts["plan-write"].count, 1);
    }

    #[test]
    fn an_unreadable_directory_is_unmeasured_rather_than_zero() {
        let (_dir, corpus) = fixture();
        assert!(count(Path::new("/nonexistent-xyzzy"), &corpus).is_none());
    }

    /// An invocation with no timestamp still counts; only the date is
    /// unknown.
    #[test]
    fn a_record_without_a_timestamp_still_counts() {
        let (dir, corpus) = fixture();
        let transcripts = transcript(
            dir.path(),
            &[
                r#"{"message":{"content":[{"type":"tool_use","name":"Skill","input":{"skill":"plan-write"}}]}}"#,
            ],
        );
        let counts = count(&transcripts, &corpus).unwrap();
        assert_eq!(counts["plan-write"].count, 1);
        assert_eq!(counts["plan-write"].last, None);
    }
}
