//! Moving one `### LESSON-NNN` heading to a free number, and taking its
//! citations with it.
//!
//! `LESSON-NNN` in `.ash/LEARNINGS.md` is minted by hand — highest existing
//! number plus one — in a file every parallel branch appends to, so it
//! collides (LESSON-022). `check` reports the collision; this is the remedy
//! the same lesson asked for, because "grep the whole corpus and figure out
//! which hits actually refer to it" is toil a command can do.
//!
//! The one hard question is **which** citations to move. A blind replace of
//! every `LESSON-<old>` is unsafe: the number being vacated usually has
//! legitimate citations elsewhere that predate the collision and refer to the
//! *surviving* lesson, and rewriting those trades a visible failure (an
//! unrenumbered reference) for a silent one (a reference pointing at the
//! wrong lesson). So the rewrite is scoped to what the current branch itself
//! added, read out of `git diff` against the merge-base — the one boundary
//! that is provably safe without a human reading every hit.
//!
//! When no merge-base can be resolved (no `git`, no repo, an unknown
//! `--against` ref) the scope is unknown, so nothing is guessed at: the
//! heading is still renamed and every other occurrence is *reported* instead
//! of rewritten. That degrade is the mitigation for having no git at all, and
//! it is a documented behaviour with its own test rather than an accident
//! (LESSON-029).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use color_eyre::eyre::{Context, Result};
use schemars::JsonSchema;
use serde::Serialize;

use crate::core::usage;

use super::root::CorpusRoot;

/// One `### LESSON-NNN: <title>` heading in `.ash/LEARNINGS.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// The whole id as written, e.g. `LESSON-019` — zero padding included,
    /// because that is the literal every citation carries.
    pub id: String,
    pub number: u64,
    /// Everything after the colon, trimmed. Empty when the heading has none.
    pub title: String,
    /// 1-based, so it reads the same as `file:line` everywhere else.
    pub line: usize,
}

/// Every lesson heading in the file, in document order.
///
/// Deliberately looser than `corpus::read_lessons`, which models a lesson's
/// whole block: renaming needs the *heading line* and nothing else, and a
/// lesson whose body is malformed still has to be renumberable.
pub fn headings(text: &str) -> Vec<Heading> {
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let (id, title) = super::corpus::split_lesson_heading(line)?;
            Some(Heading {
                number: id.strip_prefix(PREFIX)?.parse().ok()?,
                id,
                title,
                line: index + 1,
            })
        })
        .collect()
}

const PREFIX: &str = "LESSON-";

/// The one heading whose title contains `needle`, case-insensitively.
///
/// Zero matches and several matches are both usage errors, and both name what
/// was found: the caller is about to rewrite a shared file, and "it picked the
/// wrong lesson" is the one failure mode that would be worse than not running.
pub fn locate<'h>(headings: &'h [Heading], needle: &str) -> Result<&'h Heading> {
    let needle = needle.to_lowercase();
    let matching = |exact: bool| -> Vec<&Heading> {
        headings
            .iter()
            .filter(|heading| {
                let title = heading.title.to_lowercase();
                if exact {
                    title == needle
                } else {
                    title.contains(&needle)
                }
            })
            .collect()
    };

    // A whole title beats a substring of a longer one. `lesson.duplicate-id`
    // hands the caller a complete title to paste, and without this a title
    // that happens to be a prefix of another lesson's would come back
    // ambiguous — the remediation refusing to run for a reason the reader
    // cannot see from the command they were given.
    let exact = matching(true);
    let matched = if exact.len() == 1 {
        exact
    } else {
        matching(false)
    };

    match matched.as_slice() {
        [one] => Ok(one),
        [] => Err(usage(format!(
            "no lesson heading's title contains '{needle}'; the file has {}",
            summarize(headings)
        ))),
        several => Err(usage(format!(
            "'{needle}' matches {} headings: {}",
            several.len(),
            list(several.iter().copied())
        ))),
    }
}

/// One past the highest number in use, zero-padded to the width the corpus
/// already writes — a corpus of `LESSON-031` mints `LESSON-032`, not
/// `LESSON-32`, because every citation is a literal string match.
pub fn next_free_id(headings: &[Heading]) -> String {
    let highest = headings.iter().max_by_key(|heading| heading.number);
    let (number, width) = match highest {
        // The id minus its prefix is the digits as written, so the width comes
        // from the corpus rather than from a constant that could drift out of
        // step with it.
        Some(heading) => (heading.number + 1, heading.id.len() - PREFIX.len()),
        None => (1, 3),
    };
    format!("{PREFIX}{number:0width$}")
}

/// The id to move to: `--to` if the caller gave one, else the next free one.
///
/// A `--to` that names an existing heading is refused rather than merged into
/// it — creating the very duplicate this command exists to resolve.
pub fn target_id(headings: &[Heading], to: Option<&str>) -> Result<String> {
    let Some(to) = to else {
        return Ok(next_free_id(headings));
    };
    let digits = to.strip_prefix(PREFIX).unwrap_or_default();
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err(usage(format!(
            "--to expects an id of the form {PREFIX}NNN, got '{to}'"
        )));
    }
    if let Some(taken) = headings.iter().find(|heading| heading.id == to) {
        return Err(usage(format!(
            "{to} is already '{}' at line {}; pick a free id or omit --to",
            taken.title, taken.line
        )));
    }
    Ok(to.to_string())
}

fn summarize(headings: &[Heading]) -> String {
    match headings.len() {
        0 => "no lesson headings at all".to_string(),
        n => format!("{n}: {}", list(headings.iter())),
    }
}

fn list<'h>(headings: impl Iterator<Item = &'h Heading>) -> String {
    headings
        .map(|heading| format!("{} ({})", heading.id, heading.title))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Rewrites every `LESSON-<digits>` token in a line that *is* `old`, leaving
/// every other one alone.
///
/// Digit-boundary matching, the same rule `corpus::lesson_ids` reads with: a
/// plain substring replace of `LESSON-1` would eat the front of `LESSON-19`
/// and leave `LESSON-329` behind. Returns `None` when nothing changed, so a
/// caller can tell "rewritten" from "looked at".
fn rewrite(line: &str, old: &str, new: &str) -> Option<String> {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    let mut changed = false;
    while let Some(at) = rest.find(PREFIX) {
        out.push_str(&rest[..at]);
        let tail = &rest[at + PREFIX.len()..];
        let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
        let token = format!("{PREFIX}{digits}");
        if !digits.is_empty() && token == old {
            out.push_str(new);
            changed = true;
        } else {
            out.push_str(&token);
        }
        rest = &tail[digits.len()..];
    }
    out.push_str(rest);
    changed.then_some(out)
}

/// One line this run touched, or found and deliberately did not touch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Citation {
    /// Relative to the corpus root, so a report reads the same from a
    /// worktree and a clone.
    pub file: String,
    /// 1-based.
    pub line: usize,
    /// The line as it stands *after* a rewrite, or as it stands on disk when
    /// this is an unrewritten hit.
    pub text: String,
}

/// How the rewrite scope was decided — the single most important thing about
/// a run, so it travels with the report rather than only being printed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Scope {
    /// Citations added by this branch, per `git diff` against `merge_base`.
    Scoped { against: String, merge_base: String },
    /// No merge-base: the heading was renamed and everything else reported.
    Degraded { against: String, reason: String },
}

impl Scope {
    pub fn is_degraded(&self) -> bool {
        matches!(self, Scope::Degraded { .. })
    }
}

/// What one run did, or would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Report {
    pub old_id: String,
    pub new_id: String,
    pub title: String,
    /// The heading line itself, always rewritten — it is the rename.
    pub heading: Citation,
    /// Every other line the rewrite moved, heading excluded.
    pub rewritten: Vec<Citation>,
    /// Hits found but left alone: everything outside the safe scope in
    /// degrade mode. Empty on a scoped run, which has nothing to hand back.
    pub unrewritten: Vec<Citation>,
    pub scope: Scope,
    /// True when nothing was written — the report is identical either way.
    pub dry_run: bool,
}

/// What `run` was asked to do.
#[derive(Debug, Clone)]
pub struct Options {
    /// A substring of the heading's title, case-insensitive.
    pub title: String,
    pub to: Option<String>,
    /// The ref to take a merge-base against.
    pub against: String,
    pub dry_run: bool,
}

/// Renames one lesson and moves the citations this branch introduced.
pub fn run(root: &CorpusRoot, options: &Options) -> Result<Report> {
    let file = root.learnings_file();
    let text =
        std::fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?;

    let headings = headings(&text);
    let heading = locate(&headings, &options.title)?.clone();
    let old_id = heading.id.clone();
    let new_id = target_id(&headings, options.to.as_deref())?;

    let (scope, mut hits) = match merge_base(root, &options.against) {
        Ok(merge_base) => (
            Scope::Scoped {
                against: options.against.clone(),
                merge_base: merge_base.clone(),
            },
            added_lines(root, &merge_base)?,
        ),
        Err(reason) => (
            Scope::Degraded {
                against: options.against.clone(),
                reason: reason.to_string(),
            },
            Vec::new(),
        ),
    };

    // The heading line is a citation of the old id like any other, and it is
    // the one line that moves whatever the scope turned out to be. Putting it
    // in the same edit set keeps "rename" and "rewrite" from being two code
    // paths that could disagree about the same line.
    hits.retain(|hit| !(hit.0 == file && hit.1 == heading.line));
    hits.insert(0, (file.clone(), heading.line));

    let mut edits: BTreeMap<PathBuf, Vec<usize>> = BTreeMap::new();
    for (path, line) in hits {
        edits.entry(path).or_default().push(line);
    }

    let mut heading_after = None;
    let mut rewritten = Vec::new();
    for (path, lines) in &edits {
        for citation in apply(root, path, lines, &old_id, &new_id, options.dry_run)? {
            if path == &file && citation.line == heading.line {
                heading_after = Some(citation);
            } else {
                rewritten.push(citation);
            }
        }
    }

    // `locate` found the heading by reading this very line, so it contains the
    // old id by construction and the rewrite cannot have declined it.
    let heading_after = heading_after.expect("the heading line carries the old id");

    let unrewritten = if scope.is_degraded() {
        occurrences(root, &old_id)
            .into_iter()
            .filter(|hit| !(hit.0 == file && hit.1 == heading.line))
            .map(|(path, line, text)| citation(root, &path, line, text))
            .collect()
    } else {
        Vec::new()
    };

    Ok(Report {
        old_id,
        new_id,
        title: heading.title,
        heading: heading_after,
        rewritten,
        unrewritten,
        scope,
        dry_run: options.dry_run,
    })
}

/// Rewrites the named lines of one file, returning the ones that changed.
///
/// A line is only touched when it still carries the old id: with
/// `--unified=0` the line numbers come from a diff, and a file with
/// uncommitted edits can have moved underneath it. Checking the content
/// before writing turns that from a silent corruption into a line the report
/// simply does not list.
fn apply(
    root: &CorpusRoot,
    path: &Path,
    lines: &[usize],
    old_id: &str,
    new_id: &str,
    dry_run: bool,
) -> Result<Vec<Citation>> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(Vec::new());
    };
    // `split` rather than `lines`, so a file's trailing newline — or its
    // absence — survives the round trip byte for byte.
    let mut rows: Vec<String> = text.split('\n').map(str::to_string).collect();
    let mut changed = Vec::new();

    for &line in lines {
        let Some(row) = rows.get_mut(line - 1) else {
            continue;
        };
        let Some(new) = rewrite(row, old_id, new_id) else {
            continue;
        };
        *row = new.clone();
        changed.push(citation(root, path, line, new));
    }

    if !changed.is_empty() && !dry_run {
        std::fs::write(path, rows.join("\n"))
            .with_context(|| format!("writing {}", path.display()))?;
    }
    Ok(changed)
}

fn citation(root: &CorpusRoot, path: &Path, line: usize, text: String) -> Citation {
    Citation {
        file: root.relative(path).display().to_string(),
        line,
        text: text.trim_end().to_string(),
    }
}

/// The commit this branch diverged from `against` at.
fn merge_base(root: &CorpusRoot, against: &str) -> Result<String> {
    let out = git(root, &["merge-base", "HEAD", against])?;
    Ok(out.trim().to_string())
}

/// Every added line in `merge_base..working tree`, as `(path, line)`.
///
/// Diffed against the **working tree**, not `HEAD`: the line numbers are used
/// to index into files on disk, so they have to describe the files on disk.
/// Diffing to `HEAD` would be correct only while the tree is clean, and the
/// tree is never clean at the moment someone is resolving a collision.
fn added_lines(root: &CorpusRoot, merge_base: &str) -> Result<Vec<(PathBuf, usize)>> {
    // `--relative` makes every path relative to the corpus root and drops
    // anything outside it, which is also what keeps a corpus in a repo
    // subdirectory from rewriting its siblings.
    let diff = git(root, &["diff", "--unified=0", "--relative", merge_base])?;
    Ok(parse_diff(&diff)
        .into_iter()
        .map(|(path, line)| (root.path().join(path), line))
        .collect())
}

/// Pulls `(path, new-file line number)` for every added line out of a
/// `--unified=0` diff.
///
/// Hand-rolled rather than regex'd because the shape being read is tiny and
/// fixed: with zero context, every `+` line in a hunk is an addition, and they
/// run consecutively from the hunk header's start line.
fn parse_diff(diff: &str) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    let mut file: Option<String> = None;
    let mut next = 0usize;

    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("+++ ") {
            // `/dev/null` is a deletion: there is no new file to point at.
            file = rest.strip_prefix("b/").map(str::to_string);
            continue;
        }
        if let Some(rest) = line.strip_prefix("@@ ") {
            next = rest
                .split_whitespace()
                .find_map(|part| part.strip_prefix('+'))
                .and_then(|span| span.split(',').next())
                .and_then(|start| start.parse().ok())
                .unwrap_or(0);
            continue;
        }
        if line.starts_with("+++") || !line.starts_with('+') {
            continue;
        }
        if let Some(file) = file.as_ref()
            && next > 0
        {
            found.push((file.clone(), next));
            next += 1;
        }
    }
    found
}

fn git(root: &CorpusRoot, args: &[&str]) -> Result<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root.path())
        .args(args)
        .output()
        .with_context(|| format!("running git {}", args.join(" ")))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        color_eyre::eyre::bail!("git {} failed: {}", args.join(" "), stderr.trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Directories the degrade-mode walk never descends into.
///
/// Deliberately *not* `check::NOT_SOURCE`, which also skips `.ash` and every
/// `.md` file: those exclusions exist so a lesson cannot be enforced by the
/// prose describing it, and applying them here would hide the citations this
/// walk is looking for — nearly all of which are markdown inside `.ash`.
const SKIP: [&str; 3] = [".git", "target", "node_modules"];

/// Every line in the repo carrying `old_id` as a whole token.
///
/// Only used in degrade mode, where nothing is rewritten, so this is a report
/// and never an edit set.
fn occurrences(root: &CorpusRoot, old_id: &str) -> Vec<(PathBuf, usize, String)> {
    let mut found = Vec::new();
    walk(root.path(), old_id, &mut found);
    found.sort();
    found
}

fn walk(dir: &Path, old_id: &str, found: &mut Vec<(PathBuf, usize, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    paths.sort();
    for path in paths {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if SKIP.iter().any(|skip| name == *skip) {
            continue;
        }
        if path.is_dir() {
            walk(&path, old_id, found);
            continue;
        }
        // Binary files simply do not read as UTF-8, which is the cheapest
        // honest filter available and the same one `check` relies on.
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            if rewrite(line, old_id, old_id).is_some() {
                found.push((path.clone(), index + 1, line.to_string()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "\
# Distilled learnings

### LESSON-001: A shared counter collides
**Status:** prose

### LESSON-002: Something else entirely
**Status:** prose
**Seen in:** see also LESSON-001

### LESSON-031: The last one
**Status:** prose
";

    fn parsed() -> Vec<Heading> {
        headings(FILE)
    }

    /// TEST-001: exactly one match resolves; zero and several are usage
    /// errors that name what was actually there, since picking the wrong
    /// lesson is the one outcome worse than not running.
    #[test]
    fn a_heading_is_located_by_a_substring_of_its_title() {
        let headings = parsed();
        assert_eq!(headings.len(), 3);

        let found = locate(&headings, "shared COUNTER").unwrap();
        assert_eq!(found.id, "LESSON-001");
        assert_eq!(found.title, "A shared counter collides");
        assert_eq!(found.line, 3);

        let absent = locate(&headings, "no such lesson").unwrap_err();
        assert!(absent.to_string().contains("LESSON-031"), "{absent}");
        assert!(
            absent.downcast_ref::<crate::core::UsageError>().is_some(),
            "{absent:#}"
        );

        // Every title here contains an "s", which is exactly the kind of
        // too-loose substring that must refuse rather than pick one.
        let ambiguous = locate(&headings, "s").unwrap_err().to_string();
        assert!(ambiguous.contains("matches 3 headings"), "{ambiguous}");
        assert!(ambiguous.contains("LESSON-001"), "{ambiguous}");
        assert!(ambiguous.contains("LESSON-031"), "{ambiguous}");
    }

    /// TEST-002: the next free id is one past the maximum at the corpus's own
    /// zero-padded width, and a `--to` already in use is refused rather than
    /// creating the duplicate this command exists to resolve.
    #[test]
    fn the_next_free_id_follows_the_highest_one_in_use() {
        let headings = parsed();
        assert_eq!(next_free_id(&headings), "LESSON-032");
        assert_eq!(next_free_id(&[]), "LESSON-001");

        assert_eq!(target_id(&headings, None).unwrap(), "LESSON-032");
        assert_eq!(
            target_id(&headings, Some("LESSON-099")).unwrap(),
            "LESSON-099"
        );

        let taken = target_id(&headings, Some("LESSON-002")).unwrap_err();
        assert!(taken.to_string().contains("already"), "{taken}");
        assert!(
            taken.downcast_ref::<crate::core::UsageError>().is_some(),
            "{taken:#}"
        );

        let malformed = target_id(&headings, Some("42")).unwrap_err();
        assert!(malformed.to_string().contains("LESSON-NNN"), "{malformed}");
    }

    /// The digit boundary is the whole reason this is not a `sed`: a
    /// substring replace of `LESSON-1` would eat the front of `LESSON-19`.
    #[test]
    fn only_a_whole_id_token_is_rewritten() {
        assert_eq!(
            rewrite("see LESSON-1 and LESSON-19", "LESSON-1", "LESSON-40"),
            Some("see LESSON-40 and LESSON-19".to_string())
        );
        assert_eq!(rewrite("see LESSON-19", "LESSON-1", "LESSON-40"), None);
        assert_eq!(rewrite("nothing here", "LESSON-1", "LESSON-40"), None);
        assert_eq!(
            rewrite("LESSON-001 twice: LESSON-001", "LESSON-001", "LESSON-032"),
            Some("LESSON-032 twice: LESSON-032".to_string())
        );
        // Zero padding is part of the literal, so it is part of the match.
        assert_eq!(rewrite("LESSON-001", "LESSON-1", "LESSON-9"), None);
    }

    // --- the git fixture ----------------------------------------------------
    // A real repo in a tempdir, not a mocked `git`. The whole safety claim of
    // this module is a statement about what `git diff` reports, so a fake one
    // would only ever prove that the fake agrees with itself.

    struct Repo {
        dir: tempfile::TempDir,
    }

    impl Repo {
        fn write(&self, rel: &str, text: &str) {
            let path = self.dir.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }

        fn read(&self, rel: &str) -> String {
            std::fs::read_to_string(self.dir.path().join(rel)).unwrap()
        }

        fn git(&self, args: &[&str]) {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(self.dir.path())
                // Identity on the command line, so the test does not depend
                // on whatever global gitconfig the machine happens to have.
                .args(["-c", "user.email=t@example.com", "-c", "user.name=T"])
                .args(args)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr)
            );
        }

        fn root(&self) -> CorpusRoot {
            CorpusRoot::new(self.dir.path())
        }
    }

    /// `main` holds LESSON-019 and a citation of it. A branch then mints a
    /// *second* LESSON-019 and cites that one — the collision this command
    /// exists to resolve.
    fn colliding_repo() -> Repo {
        let repo = Repo {
            dir: tempfile::tempdir().unwrap(),
        };
        repo.write(
            ".ash/LEARNINGS.md",
            "# Distilled learnings\n\n\
             ### LESSON-019: The one that reached main first\n\
             **Status:** prose\n\n",
        );
        repo.write(
            "docs/note.md",
            "This predates the branch and means main's LESSON-019.\n",
        );
        repo.git(&["init", "-b", "main", "--quiet"]);
        repo.git(&["add", "-A"]);
        repo.git(&["commit", "--quiet", "-m", "main"]);

        repo.git(&["checkout", "--quiet", "-b", "work"]);
        repo.write(
            ".ash/LEARNINGS.md",
            "# Distilled learnings\n\n\
             ### LESSON-019: The one that reached main first\n\
             **Status:** prose\n\n\
             ### LESSON-019: The one the branch minted\n\
             **Status:** prose\n\n\
             ### LESSON-020: A later lesson\n\
             **Seen in:** the same run as LESSON-019, the branch's one\n",
        );
        repo.write("docs/branch.md", "New prose citing LESSON-019 here.\n");
        repo.git(&["add", "-A"]);
        repo.git(&["commit", "--quiet", "-m", "branch"]);
        repo
    }

    fn options(title: &str, against: &str) -> Options {
        Options {
            title: title.to_string(),
            to: None,
            against: against.to_string(),
            dry_run: false,
        }
    }

    /// TEST-003: the citation the branch added moves; the one that predates
    /// the branch's divergence from `main` does not. This is the entire
    /// reason the tool reads a diff instead of running `sed`.
    #[test]
    fn only_citations_this_branch_added_are_rewritten() {
        let repo = colliding_repo();
        let report = run(&repo.root(), &options("the branch minted", "main")).unwrap();

        assert_eq!(report.old_id, "LESSON-019");
        assert_eq!(report.new_id, "LESSON-021");
        assert!(!report.scope.is_degraded(), "{:?}", report.scope);
        assert!(report.unrewritten.is_empty(), "{:?}", report.unrewritten);

        let learnings = repo.read(".ash/LEARNINGS.md");
        // The branch's heading moved...
        assert!(learnings.contains("### LESSON-021: The one the branch minted"));
        // ...and so did the citation the branch added in the same diff.
        assert!(learnings.contains("the same run as LESSON-021, the branch's one"));
        assert!(repo.read("docs/branch.md").contains("LESSON-021"));

        // ...while main's heading and the citation predating the branch are
        // untouched, because nothing in this run's diff added them.
        assert!(learnings.contains("### LESSON-019: The one that reached main first"));
        assert!(repo.read("docs/note.md").contains("LESSON-019"));

        let moved: Vec<&str> = report.rewritten.iter().map(|c| c.file.as_str()).collect();
        assert_eq!(moved, [".ash/LEARNINGS.md", "docs/branch.md"]);
        assert_eq!(report.heading.file, ".ash/LEARNINGS.md");
        assert_eq!(report.heading.line, 6);
    }

    /// TEST-004: with no merge-base to be had, the heading is still renamed —
    /// that much was named explicitly — and every other hit comes back as a
    /// report rather than a guess.
    #[test]
    fn an_unresolvable_ref_renames_the_heading_and_reports_the_rest() {
        let repo = colliding_repo();
        let report = run(&repo.root(), &options("the branch minted", "no-such-ref")).unwrap();

        assert!(report.scope.is_degraded(), "{:?}", report.scope);
        assert!(report.rewritten.is_empty(), "{:?}", report.rewritten);
        assert_eq!(
            report.heading.text,
            "### LESSON-021: The one the branch minted"
        );

        // Only the heading was written.
        let learnings = repo.read(".ash/LEARNINGS.md");
        assert!(learnings.contains("### LESSON-021: The one the branch minted"));
        assert!(learnings.contains("the same run as LESSON-019, the branch's one"));
        assert!(repo.read("docs/branch.md").contains("LESSON-019"));
        assert!(repo.read("docs/note.md").contains("LESSON-019"));

        // ...and everything it did not touch was handed back instead.
        let hits: Vec<&str> = report.unrewritten.iter().map(|c| c.file.as_str()).collect();
        assert_eq!(
            hits,
            [
                ".ash/LEARNINGS.md",
                ".ash/LEARNINGS.md",
                "docs/branch.md",
                "docs/note.md"
            ]
        );
    }

    /// The same degrade, reached the other way: a directory that is no git
    /// repo at all. RISK-002's two causes have one behaviour.
    #[test]
    fn a_tree_that_is_no_repo_degrades_the_same_way() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".ash")).unwrap();
        std::fs::write(dir.path().join(".ash/LEARNINGS.md"), FILE).unwrap();

        let report = run(&CorpusRoot::new(dir.path()), &options("last one", "main")).unwrap();
        assert!(report.scope.is_degraded(), "{:?}", report.scope);
        assert_eq!(report.new_id, "LESSON-032");
    }

    /// TEST-005: dry-run returns the report a real run would, and leaves the
    /// tree exactly as it found it — the preview is worthless if it is
    /// computed by a different path than the write.
    #[test]
    fn a_dry_run_reports_the_same_thing_and_writes_nothing() {
        let repo = colliding_repo();
        let before = repo.read(".ash/LEARNINGS.md");

        let mut options = options("the branch minted", "main");
        options.dry_run = true;
        let preview = run(&repo.root(), &options).unwrap();

        assert!(preview.dry_run);
        assert_eq!(repo.read(".ash/LEARNINGS.md"), before);
        assert_eq!(
            repo.read("docs/branch.md"),
            "New prose citing LESSON-019 here.\n"
        );

        options.dry_run = false;
        let real = run(&repo.root(), &options).unwrap();
        assert_ne!(repo.read(".ash/LEARNINGS.md"), before);
        assert_eq!(
            Report {
                dry_run: false,
                ..preview
            },
            real
        );
    }

    #[test]
    fn a_unified_zero_diff_yields_each_added_line_and_its_number() {
        let diff = "\
diff --git a/.ash/LEARNINGS.md b/.ash/LEARNINGS.md
--- a/.ash/LEARNINGS.md
+++ b/.ash/LEARNINGS.md
@@ -3,0 +4,2 @@
+### LESSON-019: New
+cites LESSON-019
@@ -20 +21 @@
-old
+new
diff --git a/gone.md b/gone.md
--- a/gone.md
+++ /dev/null
@@ -1 +0,0 @@
-was here
";
        assert_eq!(
            parse_diff(diff),
            vec![
                (".ash/LEARNINGS.md".to_string(), 4),
                (".ash/LEARNINGS.md".to_string(), 5),
                (".ash/LEARNINGS.md".to_string(), 21),
            ]
        );
    }
}
