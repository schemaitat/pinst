//! Where compiled-in content is read from for this run.
//!
//! pinst carries two trees inside the binary — `configs/` (what goes into
//! `$HOME`) and `docs/tools/` (the tool catalogue) — and both want the same
//! property: on a machine with this repo checked out, read the tree on disk so
//! edits are live without a rebuild; on a machine that only has the downloaded
//! binary, read the embedded copy. This module is that decision, once, so the
//! two trees cannot drift apart on what counts as a checkout.

use std::path::{Path, PathBuf};

/// Where a tree's content is read from for this run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A directory on disk. Config targets are symlinked at it so edits in
    /// either direction are immediately live.
    Tree(PathBuf),
    /// The copy compiled into the binary. Config targets are written as real
    /// files.
    Embedded,
}

impl Source {
    pub fn describe(&self) -> String {
        match self {
            Source::Tree(path) => format!("source tree ({})", path.display()),
            Source::Embedded => "embedded in binary".to_string(),
        }
    }

    pub fn is_tree(&self) -> bool {
        matches!(self, Source::Tree(_))
    }
}

/// Resolves where `<subdir>` is read from: an explicit `$<env_var>`, else the
/// subdirectory of a checkout found at the working directory or above the
/// running binary (cargo puts it at `target/<profile>/pinst`), else the copy
/// embedded at compile time.
pub fn resolve(subdir: &str, env_var: &str) -> Source {
    if let Some(explicit) = std::env::var_os(env_var) {
        let path = PathBuf::from(explicit);
        let candidate = if path.ends_with(subdir) {
            path
        } else {
            path.join(subdir)
        };
        // An explicit override is trusted as a location, but still has to be
        // absolute before we point symlinks at it.
        if let Some(tree) = absolute_tree(&candidate) {
            return Source::Tree(tree);
        }
    }

    if let Some(tree) = checkout_tree(Path::new("."), subdir) {
        return Source::Tree(tree);
    }

    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors().skip(1).take(4) {
            if let Some(tree) = checkout_tree(ancestor, subdir) {
                return Source::Tree(tree);
            }
        }
    }

    Source::Embedded
}

/// Canonicalizes a candidate tree.
///
/// Absolute is not optional: `source_path` becomes the target of the symlinks
/// written into `$HOME`, and a relative path like `configs/zsh/.zshrc` would
/// be resolved by the kernel against `$HOME`, producing a dangling link.
fn absolute_tree(candidate: &Path) -> Option<PathBuf> {
    candidate.is_dir().then(|| candidate.canonicalize().ok())?
}

/// Accepts `<root>/<subdir>` only if `root` really is this project's checkout
/// — a directory with a `manifest.toml` in it.
///
/// Without that check, an unrelated `~/configs` directory would be picked up
/// when pinst runs from `~/.local/bin` and every config command would fail
/// against it instead of falling back to the embedded copy.
pub(crate) fn checkout_tree(root: &Path, subdir: &str) -> Option<PathBuf> {
    if !root.join("manifest.toml").is_file() {
        return None;
    }
    absolute_tree(&root.join(subdir))
}

/// Serializes the tests that set process-wide environment variables.
/// `std::env::set_var` is global, so two of these running concurrently would
/// read each other's override. Shared with `docs`, which overrides its own.
#[cfg(test)]
pub(crate) fn test_env_lock() -> std::sync::MutexGuard<'static, ()> {
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkout(dir: &Path, subdir: &str) {
        std::fs::write(dir.join("manifest.toml"), "[meta]\nschema_version = 1\n").unwrap();
        std::fs::create_dir_all(dir.join(subdir)).unwrap();
    }

    #[test]
    fn override_pointing_at_the_parent_finds_the_subdir() {
        let _guard = test_env_lock();
        let root = tempfile::tempdir().unwrap();
        checkout(root.path(), "docs/tools");

        unsafe { std::env::set_var("PINST_TEST_SOURCE", root.path()) };
        let source = resolve("docs/tools", "PINST_TEST_SOURCE");
        unsafe { std::env::remove_var("PINST_TEST_SOURCE") };

        assert_eq!(
            source,
            Source::Tree(root.path().canonicalize().unwrap().join("docs/tools"))
        );
    }

    #[test]
    fn override_pointing_straight_at_the_subdir_is_taken_as_is() {
        let _guard = test_env_lock();
        let root = tempfile::tempdir().unwrap();
        checkout(root.path(), "configs");

        unsafe { std::env::set_var("PINST_TEST_SOURCE", root.path().join("configs")) };
        let source = resolve("configs", "PINST_TEST_SOURCE");
        unsafe { std::env::remove_var("PINST_TEST_SOURCE") };

        assert_eq!(
            source,
            Source::Tree(root.path().canonicalize().unwrap().join("configs"))
        );
    }

    #[test]
    fn a_directory_without_a_manifest_beside_it_is_not_a_checkout() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("configs")).unwrap();
        assert_eq!(checkout_tree(root.path(), "configs"), None);
    }

    #[test]
    fn a_checkout_without_the_subdir_is_not_a_tree() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("manifest.toml"), "").unwrap();
        assert_eq!(checkout_tree(root.path(), "docs/tools"), None);
    }
}
