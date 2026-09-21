//! Where a **project**-scoped harness install lands.
//!
//! Deliberately not `harness::root::CorpusRoot`, which requires a `.ash/` to
//! already exist. A repo adopting the harness for the first time has none —
//! that is the entire point of running `pinst harness install` there — so
//! reusing corpus discovery would make the command refuse to run in exactly
//! the place it is most useful (REQ-007).
//!
//! The fallback order after "no `.ash/` yet" is the git top-level, because
//! that is almost always what "this project" means, and finally the working
//! directory, so the command never simply refuses.

use std::path::{Path, PathBuf};

use color_eyre::eyre::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRoot {
    root: PathBuf,
}

impl InstallRoot {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    /// An explicit path, else the nearest ancestor of the working directory
    /// holding `.ash/`, else the git top-level, else the working directory
    /// itself.
    pub fn resolve(explicit: Option<&Path>) -> Result<Self> {
        let cwd = std::env::current_dir()?;
        Self::resolve_from(explicit, &cwd)
    }

    /// `resolve`, with the working directory taken as a parameter instead of
    /// read from the process. A test overriding "the working directory"
    /// through `std::env::set_current_dir` would mutate process-wide state
    /// that every test in this binary shares — including ones that run
    /// concurrently — so the fallback path is exercised through this instead
    /// (LESSON-018).
    fn resolve_from(explicit: Option<&Path>, cwd: &Path) -> Result<Self> {
        if let Some(path) = explicit {
            return Ok(Self::new(path));
        }
        if let Some(found) = Self::discover_ash(cwd) {
            return Ok(found);
        }
        if let Some(top) = git_top_level(cwd) {
            return Ok(Self::new(top));
        }
        Ok(Self::new(cwd))
    }

    fn discover_ash(start: &Path) -> Option<Self> {
        start
            .ancestors()
            .find(|dir| dir.join(".ash").is_dir())
            .map(Self::new)
    }
}

fn git_top_level(start: &Path) -> Option<PathBuf> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(start)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_explicit_root_wins_over_everything() {
        let root = InstallRoot::resolve(Some(Path::new("/wherever"))).unwrap();
        assert_eq!(root.path(), Path::new("/wherever"));
    }

    #[test]
    fn discovery_walks_up_to_a_directory_holding_dot_ash() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".ash")).unwrap();
        let deep = tmp.path().join("a/b/c");
        std::fs::create_dir_all(&deep).unwrap();

        let found = InstallRoot::discover_ash(&deep).unwrap();
        assert_eq!(found.path(), tmp.path());
    }

    #[test]
    fn with_no_dot_ash_and_no_git_repo_the_working_directory_itself_is_the_root() {
        // `/tmp` itself (not a fresh tempdir under it) is reliably outside
        // any git worktree and holds no `.ash/`, so this exercises the last
        // fallback without touching the process's actual working directory.
        let root = InstallRoot::resolve_from(None, Path::new("/tmp")).unwrap();
        assert_eq!(root.path(), Path::new("/tmp"));
    }

    #[test]
    fn a_git_top_level_wins_over_a_bare_working_directory_when_there_is_no_dot_ash() {
        let tmp = tempfile::tempdir().unwrap();
        let canonical = tmp.path().canonicalize().unwrap();
        let deep = canonical.join("a/b");
        std::fs::create_dir_all(&deep).unwrap();
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&canonical)
            .arg("init")
            .arg("-q")
            .status()
            .unwrap();
        assert!(status.success());

        let root = InstallRoot::resolve_from(None, &deep).unwrap();
        assert_eq!(root.path(), canonical);
    }
}
