//! The plan corpus, read off disk into something the checks can ask questions
//! of.
//!
//! This module only *models* — it decides nothing and reports nothing. Every
//! verdict about what is wrong with a corpus belongs to `check`, and every
//! rate belongs to `skills`; keeping the reader free of both is what lets the
//! two share it without one inheriting the other's exit-code contract.
//!
//! Nothing here fails on a malformed plan either. A directory whose name does
//! not parse, a README with no frontmatter, a phase file numbered `ab` — all
//! of them are loaded as far as they go and carry the reason they did not go
//! further, because a checker that stops at the first bad plan reports one
//! problem per run.

// This module lands one phase ahead of its callers, and `-D warnings` makes
// that a build failure. Scoped to non-test builds so the tests still have to
// exercise everything, and dated rather than left as a bare `allow`:
// `index` (phase 2) reads the plan list, `check` (phase 3) the rest.
// Delete this attribute when that phase lands.
#![cfg_attr(not(test), allow(dead_code))]

use std::path::{Path, PathBuf};

use color_eyre::eyre::Result;

use super::frontmatter::{self, Frontmatter, ParseError};
use super::root::CorpusRoot;

/// A document that was supposed to carry frontmatter.
#[derive(Debug, Clone)]
pub struct Document {
    pub path: PathBuf,
    /// `None` when the file has no `---` block at all, which is a different
    /// finding from one that is malformed.
    pub frontmatter: Option<Frontmatter>,
    /// Set when the block exists but is outside the accepted subset.
    pub error: Option<ParseError>,
}

impl Document {
    fn load(path: PathBuf) -> Self {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        match frontmatter::parse(&text) {
            Ok(frontmatter) => Self {
                path,
                frontmatter,
                error: None,
            },
            Err(error) => Self {
                path,
                frontmatter: None,
                error: Some(error),
            },
        }
    }

    pub fn text(&self, key: &str) -> Option<String> {
        self.frontmatter.as_ref().and_then(|fm| fm.text(key))
    }

    pub fn status(&self) -> Option<String> {
        self.text("status")
    }

    /// Whether the file declares a `## Status` section as well as frontmatter
    /// status — two copies of one mutable field, which the skills forbid.
    pub fn has_status_section(&self) -> bool {
        std::fs::read_to_string(&self.path)
            .map(|text| text.lines().any(|line| line.trim_end() == "## Status"))
            .unwrap_or(false)
    }
}

/// One `phase-NN.md`.
#[derive(Debug, Clone)]
pub struct Phase {
    pub document: Document,
    /// The number in the *filename*, which is the one the chain is checked
    /// against. `None` when the name is not `phase-<two digits>.md`.
    pub number: Option<u32>,
    pub file_name: String,
}

/// One directory under `.ash/plans/`.
#[derive(Debug, Clone)]
pub struct Plan {
    pub dir: PathBuf,
    /// The directory's own name, which is what every finding id carries.
    pub name: String,
    /// `<yymmdd>-<six letters>`, split out of the directory name. `None` when
    /// the name does not match the required shape.
    pub id: Option<String>,
    pub slug: Option<String>,
    /// `None` when the directory has no `README.md`.
    pub readme: Option<Document>,
    pub phases: Vec<Phase>,
    pub has_learnings: bool,
    /// True when some run under `logs/` recorded a `run_end`.
    pub has_finished_run: bool,
}

impl Plan {
    pub fn status(&self) -> Option<String> {
        self.readme.as_ref().and_then(Document::status)
    }

    pub fn is_done(&self) -> bool {
        self.status().as_deref() == Some("Done")
    }

    pub fn learnings_file(&self) -> PathBuf {
        self.dir.join("learnings.md")
    }
}

/// Everything under one corpus root.
#[derive(Debug)]
pub struct Corpus {
    pub root: CorpusRoot,
    pub plans: Vec<Plan>,
    /// False when `.ash/plans` does not exist at all.
    pub plans_dir_exists: bool,
}

impl Corpus {
    pub fn load(root: CorpusRoot) -> Result<Self> {
        let plans_dir = root.plans_dir();
        if !plans_dir.is_dir() {
            return Ok(Self {
                root,
                plans: Vec::new(),
                plans_dir_exists: false,
            });
        }

        let mut dirs: Vec<PathBuf> = std::fs::read_dir(&plans_dir)?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|path| path.is_dir())
            .collect();
        // Plan ids start with a yymmdd date, so a plain name sort is already
        // chronological — the reason the index needs no sort key.
        dirs.sort();

        let plans = dirs.into_iter().map(load_plan).collect();
        Ok(Self {
            root,
            plans,
            plans_dir_exists: true,
        })
    }

    pub fn len(&self) -> usize {
        self.plans.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plans.is_empty()
    }

    /// A path as a finding id should carry it: relative to the corpus root,
    /// so the id does not change with the checkout location.
    pub fn relative(&self, path: &Path) -> String {
        self.root.relative(path).display().to_string()
    }

    /// A path relative to `.ash/plans/`, the form `rel()` produced in bash.
    pub fn plan_relative(&self, path: &Path) -> String {
        path.strip_prefix(self.root.plans_dir())
            .unwrap_or(path)
            .display()
            .to_string()
    }
}

/// `260919-qwerty-add-mlflow` -> (`260919-qwerty`, `add-mlflow`).
///
/// Only a name of exactly the documented shape splits; anything else yields
/// `None` and becomes `plan.malformed-dir`, rather than being half-parsed
/// into an id that would then mismatch its own frontmatter.
pub fn split_dir_name(name: &str) -> Option<(String, String)> {
    let (date, rest) = name.split_once('-')?;
    let (hash, slug) = rest.split_once('-')?;
    let shaped = date.len() == 6
        && date.chars().all(|c| c.is_ascii_digit())
        && hash.len() == 6
        && hash.chars().all(|c| c.is_ascii_lowercase())
        && !slug.is_empty()
        && slug.split('-').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        });
    shaped.then(|| (format!("{date}-{hash}"), slug.to_string()))
}

fn load_plan(dir: PathBuf) -> Plan {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let split = split_dir_name(&name);

    let readme_path = dir.join("README.md");
    let readme = readme_path.is_file().then(|| Document::load(readme_path));

    let mut phases: Vec<Phase> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.file_name()
                .map(|n| n.to_string_lossy().starts_with("phase-"))
                .unwrap_or(false)
                && path.extension().map(|e| e == "md").unwrap_or(false)
        })
        .map(|path| {
            let file_name = path.file_name().unwrap().to_string_lossy().to_string();
            let number = file_name
                .strip_prefix("phase-")
                .and_then(|rest| rest.strip_suffix(".md"))
                .filter(|digits| digits.len() == 2 && digits.chars().all(|c| c.is_ascii_digit()))
                .and_then(|digits| digits.parse().ok());
            Phase {
                document: Document::load(path),
                number,
                file_name,
            }
        })
        .collect();
    phases.sort_by(|a, b| a.file_name.cmp(&b.file_name));

    Plan {
        has_learnings: dir.join("learnings.md").is_file(),
        has_finished_run: has_finished_run(&dir.join("logs")),
        id: split.as_ref().map(|(id, _)| id.clone()),
        slug: split.map(|(_, slug)| slug),
        dir,
        name,
        readme,
        phases,
    }
}

/// A run that reached `run_end`, which is the only thing that distinguishes
/// "implementation finished" from "implementation is under way" — and
/// therefore the only honest trigger for expecting a `learnings.md`
/// (LESSON-011).
fn has_finished_run(logs: &Path) -> bool {
    std::fs::read_dir(logs)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().map(|e| e == "log").unwrap_or(false))
        .any(|path| {
            std::fs::read_to_string(path)
                .map(|text| text.contains(r#""event":"run_end""#))
                .unwrap_or(false)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_a_well_formed_directory_name() {
        assert_eq!(
            split_dir_name("260919-qwerty-add-mlflow-experiment-tracking"),
            Some((
                "260919-qwerty".to_string(),
                "add-mlflow-experiment-tracking".to_string()
            ))
        );
    }

    #[test]
    fn refuses_to_half_parse_a_malformed_name() {
        for bad in [
            "260919-qwerty",        // no slug
            "26099-qwerty-thing",   // short date
            "260919-QWERTY-thing",  // upper case hash
            "260919-qwertyy-thing", // seven letters
            "260919-qwerty--thing", // empty slug segment
            "plans",
        ] {
            assert_eq!(split_dir_name(bad), None, "{bad} should not split");
        }
    }

    /// The real corpus is the fixture that matters: every directory in it must
    /// model cleanly, with an id, a slug and a README.
    #[test]
    fn the_real_corpus_loads() {
        let root = CorpusRoot::new(env!("CARGO_MANIFEST_DIR"));
        let corpus = Corpus::load(root).unwrap();

        assert!(corpus.plans_dir_exists);
        assert!(corpus.len() >= 5, "found only {} plans", corpus.len());
        for plan in &corpus.plans {
            assert!(plan.id.is_some(), "{} has no id", plan.name);
            assert!(plan.readme.is_some(), "{} has no README", plan.name);
            let readme = plan.readme.as_ref().unwrap();
            assert!(
                readme.error.is_none(),
                "{} README: {:?}",
                plan.name,
                readme.error
            );
            assert!(readme.status().is_some(), "{} has no status", plan.name);
            for phase in &plan.phases {
                assert!(
                    phase.number.is_some(),
                    "{} has {}",
                    plan.name,
                    phase.file_name
                );
            }
        }
    }

    /// Plans come back in name order, which is date order, which is what the
    /// index relies on instead of a sort key.
    #[test]
    fn plans_are_ordered_chronologically() {
        let corpus = Corpus::load(CorpusRoot::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let names: Vec<&str> = corpus.plans.iter().map(|p| p.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    /// The accessors `check` will decide with, exercised against the corpus
    /// they will actually run on. A finished plan has all four of: a Done
    /// status, a `learnings.md`, a run that reached `run_end`, and phase
    /// documents that loaded.
    #[test]
    fn a_finished_plan_reports_itself_finished() {
        let corpus = Corpus::load(CorpusRoot::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        assert_eq!(corpus.root, CorpusRoot::new(env!("CARGO_MANIFEST_DIR")));

        let done = corpus
            .plans
            .iter()
            .find(|p| p.name.starts_with("260919-vldfei"))
            .expect("the self-improving-agent-harness plan should be in the corpus");

        assert!(done.is_done(), "status was {:?}", done.status());
        assert_eq!(done.slug.as_deref(), Some("self-improving-agent-harness"));
        assert!(done.has_learnings);
        assert!(done.has_finished_run, "its run log records a run_end");
        assert_eq!(done.learnings_file(), done.dir.join("learnings.md"));
        assert!(done.learnings_file().is_file());

        let phase = done.phases.first().expect("it has phase files");
        assert!(phase.document.path.ends_with("phase-01.md"));
        assert_eq!(phase.document.status().as_deref(), Some("Done"));
    }

    /// A plan under way is distinguishable from a finished one by the run log
    /// alone — which is what keeps `plan.learnings-missing` from firing for
    /// the whole duration of every implementation run (LESSON-011).
    #[test]
    fn a_run_still_in_flight_has_no_finished_run() {
        let dir = tempfile::tempdir().unwrap();
        let plan = dir.path().join(".ash/plans/260919-qwerty-thing/logs");
        std::fs::create_dir_all(&plan).unwrap();
        std::fs::write(plan.join("run.log"), "{\"event\":\"run_start\"}\n").unwrap();

        let corpus = Corpus::load(CorpusRoot::new(dir.path())).unwrap();
        let only = &corpus.plans[0];
        assert!(!only.has_finished_run);
        assert!(!only.has_learnings);
        assert!(only.readme.is_none(), "it has no README yet");

        std::fs::write(plan.join("run.log"), "{\"event\":\"run_end\"}\n").unwrap();
        let corpus = Corpus::load(CorpusRoot::new(dir.path())).unwrap();
        assert!(corpus.plans[0].has_finished_run);
    }

    #[test]
    fn a_status_section_beside_frontmatter_status_is_visible() {
        let dir = tempfile::tempdir().unwrap();
        let with = dir.path().join("with.md");
        let without = dir.path().join("without.md");
        std::fs::write(&with, "---\nstatus: Done\n---\n\n## Status\nDone\n").unwrap();
        std::fs::write(&without, "---\nstatus: Done\n---\n\n## Steps\n").unwrap();

        assert!(Document::load(with).has_status_section());
        assert!(!Document::load(without).has_status_section());
    }

    /// Finding ids carry a corpus-relative path so they do not change with
    /// the checkout location — the same id must come out of a worktree and a
    /// clone.
    #[test]
    fn paths_in_finding_ids_are_relative_to_the_corpus() {
        let corpus = Corpus::load(CorpusRoot::new("/repo")).unwrap();
        let file = std::path::Path::new("/repo/.ash/plans/260919-qwerty-x/phase-01.md");

        assert_eq!(
            corpus.relative(file),
            ".ash/plans/260919-qwerty-x/phase-01.md"
        );
        assert_eq!(corpus.plan_relative(file), "260919-qwerty-x/phase-01.md");
    }

    #[test]
    fn a_root_with_no_plans_directory_loads_empty() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".ash")).unwrap();
        let corpus = Corpus::load(CorpusRoot::new(dir.path())).unwrap();
        assert!(!corpus.plans_dir_exists);
        assert!(corpus.is_empty());
    }
}
