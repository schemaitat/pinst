//! Where the corpus this run checks actually lives.
//!
//! Deliberately **not** `core::source::resolve`. That one accepts a directory
//! only when a `manifest.toml` sits beside it, and falls back to the ancestors
//! of the running binary. Both rules are right for `configs/` and
//! `docs/tools/`, which are trees pinst *ships to a machine*; both are wrong
//! for `.ash/`, which belongs to whichever repo the caller is standing in. A
//! pinst installed at `~/.local/bin` would otherwise check its own checkout's
//! corpus while the caller sits three directories deep in another project —
//! and report it as clean.
//!
//! So discovery walks up from the working directory looking for `.ash/`
//! itself, which is the only thing that identifies a corpus.

// This module lands one phase ahead of its callers, and `-D warnings` makes
// that a build failure. Scoped to non-test builds so the tests still have to
// exercise everything, and dated rather than left as a bare `allow`:
// `index` (phase 2) takes most of it, `skills` (phase 4) the last three.
// Delete this attribute when that phase lands.
#![cfg_attr(not(test), allow(dead_code))]

use std::path::{Path, PathBuf};

use color_eyre::eyre::{Result, eyre};

/// The corpus directory's name, relative to the repo root.
pub const ASH_DIR: &str = ".ash";

/// Points the harness at a corpus explicitly, the way `PINST_DOCS_SOURCE`
/// overrides the catalogue. Accepts the repo root or the `.ash` directory.
pub const ROOT_ENV: &str = "PINST_ASH_DIR";

/// A repo that has a corpus: the directory *containing* `.ash`.
///
/// Every path the harness reads hangs off this, so a test can point the whole
/// subsystem at a `tempdir` by constructing one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusRoot {
    root: PathBuf,
}

impl CorpusRoot {
    /// An explicit path, then `$PINST_ASH_DIR`, then a walk up from the
    /// working directory. The error names the directory searched from,
    /// because "no corpus found" is nearly always "wrong working directory".
    pub fn resolve(explicit: Option<&Path>) -> Result<Self> {
        if let Some(path) = explicit {
            return Self::at(path)
                .ok_or_else(|| eyre!("no {ASH_DIR}/ at or below {}", path.display()));
        }
        if let Some(value) = std::env::var_os(ROOT_ENV) {
            let path = PathBuf::from(&value);
            return Self::at(&path).ok_or_else(|| {
                eyre!(
                    "{ROOT_ENV} points at {}, which has no {ASH_DIR}/",
                    path.display()
                )
            });
        }
        let cwd = std::env::current_dir()?;
        Self::discover(&cwd)
            .ok_or_else(|| eyre!("no {ASH_DIR}/ in {} or any parent directory", cwd.display()))
    }

    /// Takes either the repo root or the `.ash` directory itself, because a
    /// caller setting `PINST_ASH_DIR` has no way to guess which we wanted and
    /// both readings are unambiguous.
    pub fn at(path: &Path) -> Option<Self> {
        if path.file_name() == Some(std::ffi::OsStr::new(ASH_DIR)) && path.is_dir() {
            return path.parent().map(Self::new);
        }
        Self::containing(path)
    }

    /// The first ancestor of `start` — `start` itself included — that holds a
    /// `.ash` directory.
    pub fn discover(start: &Path) -> Option<Self> {
        start.ancestors().find_map(Self::containing)
    }

    fn containing(dir: &Path) -> Option<Self> {
        dir.join(ASH_DIR).is_dir().then(|| Self::new(dir))
    }

    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    pub fn ash_dir(&self) -> PathBuf {
        self.root.join(ASH_DIR)
    }

    pub fn plans_dir(&self) -> PathBuf {
        self.ash_dir().join("plans")
    }

    pub fn index_file(&self) -> PathBuf {
        self.ash_dir().join("INDEX.md")
    }

    pub fn changelog_file(&self) -> PathBuf {
        self.ash_dir().join("CHANGELOG.log")
    }

    pub fn learnings_file(&self) -> PathBuf {
        self.ash_dir().join("LEARNINGS.md")
    }

    /// The vendor-neutral source of truth for skills.
    pub fn skills_dir(&self) -> PathBuf {
        self.root.join(".agents/skills")
    }

    /// Where `agents-wire.sh` projects them for Claude Code.
    pub fn wired_dir(&self) -> PathBuf {
        self.root.join(".claude/skills")
    }

    pub fn commands_dir(&self) -> PathBuf {
        self.root.join(".claude/commands")
    }

    /// A path rendered relative to the corpus root.
    ///
    /// Finding ids are match keys, so they must not change with the checkout
    /// location — this is what keeps `plan.id-mismatch.<name>` stable whether
    /// the repo is at `/home/me/pinst` or in a worktree.
    pub fn relative<'p>(&self, path: &'p Path) -> &'p Path {
        path.strip_prefix(&self.root).unwrap_or(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus(dir: &Path) {
        std::fs::create_dir_all(dir.join(".ash/plans")).unwrap();
    }

    #[test]
    fn discovery_walks_up_to_the_directory_holding_dot_ash() {
        let root = tempfile::tempdir().unwrap();
        corpus(root.path());
        let deep = root.path().join("src/core/harness");
        std::fs::create_dir_all(&deep).unwrap();

        assert_eq!(
            CorpusRoot::discover(&deep),
            Some(CorpusRoot::new(root.path()))
        );
    }

    #[test]
    fn a_tree_with_no_dot_ash_is_not_a_corpus() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src")).unwrap();
        assert_eq!(CorpusRoot::discover(&root.path().join("src")), None);
    }

    /// The whole reason this module exists rather than reusing
    /// `source::checkout_tree`: a corpus needs no `manifest.toml`, because the
    /// repo being checked is usually not pinst.
    #[test]
    fn a_corpus_needs_no_manifest_beside_it() {
        let root = tempfile::tempdir().unwrap();
        corpus(root.path());
        assert!(!root.path().join("manifest.toml").exists());
        assert!(CorpusRoot::discover(root.path()).is_some());
    }

    #[test]
    fn an_explicit_path_is_taken_either_way_round() {
        let root = tempfile::tempdir().unwrap();
        corpus(root.path());
        let expected = Some(CorpusRoot::new(root.path()));

        assert_eq!(CorpusRoot::at(root.path()), expected);
        assert_eq!(CorpusRoot::at(&root.path().join(".ash")), expected);
    }

    /// Every accessor, spelled out. A typo in one of these would send a
    /// check looking for a file that does not exist and report the corpus as
    /// clean — the quietest possible way for this subsystem to be wrong.
    #[test]
    fn paths_hang_off_the_root() {
        let root = CorpusRoot::new("/repo");
        assert_eq!(root.path(), Path::new("/repo"));
        assert_eq!(root.ash_dir(), Path::new("/repo/.ash"));
        assert_eq!(root.plans_dir(), Path::new("/repo/.ash/plans"));
        assert_eq!(root.index_file(), Path::new("/repo/.ash/INDEX.md"));
        assert_eq!(root.changelog_file(), Path::new("/repo/.ash/CHANGELOG.log"));
        assert_eq!(root.learnings_file(), Path::new("/repo/.ash/LEARNINGS.md"));
        assert_eq!(root.skills_dir(), Path::new("/repo/.agents/skills"));
        assert_eq!(root.wired_dir(), Path::new("/repo/.claude/skills"));
        assert_eq!(root.commands_dir(), Path::new("/repo/.claude/commands"));
        assert_eq!(
            root.relative(Path::new("/repo/.ash/plans/260919-qwerty-x/README.md")),
            Path::new(".ash/plans/260919-qwerty-x/README.md")
        );
    }

    /// `resolve` reads process-wide state, so it borrows the lock `source`
    /// already keeps for exactly this — two of these running concurrently
    /// would read each other's override.
    #[test]
    fn the_env_override_wins_over_discovery() {
        let _guard = crate::core::source::test_env_lock();
        let root = tempfile::tempdir().unwrap();
        corpus(root.path());

        unsafe { std::env::set_var(ROOT_ENV, root.path()) };
        let resolved = CorpusRoot::resolve(None);
        let pointed_at_ash = CorpusRoot::resolve(None).is_ok();
        unsafe { std::env::remove_var(ROOT_ENV) };

        assert_eq!(resolved.unwrap(), CorpusRoot::new(root.path()));
        assert!(pointed_at_ash);
    }

    #[test]
    fn an_explicit_root_with_no_corpus_is_an_error_naming_the_path() {
        let empty = tempfile::tempdir().unwrap();
        let err = CorpusRoot::resolve(Some(empty.path()))
            .unwrap_err()
            .to_string();
        assert!(err.contains(&empty.path().display().to_string()), "{err}");
    }
}
