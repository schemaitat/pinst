//! The shippable harness: `.agents/`, embedded at compile time.
//!
//! Two trees already do this — `configs/` and `docs/tools/` — and `.agents/`
//! joins them for the same reason: a binary downloaded onto a machine with
//! no clone still has to be able to *install* the harness it enforces, not
//! only check a corpus that happens to already have one. `.ash/` stays out,
//! deliberately: it is content the checked repo owns, not content pinst
//! ships to a machine (see `harness::root` for the fuller argument, which
//! applies unchanged here).
//!
//! Unlike `configs/`, which is organized by target path under `$HOME`,
//! `.agents/` is organized by *kind*: `skills/<name>/` (a directory — a
//! `SKILL.md` plus whatever references or scripts it carries) and
//! `commands/<name>.md` (a single file). `Asset` captures that shape once so
//! nothing downstream has to special-case "a skill is a directory but a
//! command is a file".

use std::path::{Path, PathBuf};

use color_eyre::eyre::{Context, Result, bail};
use include_dir::{Dir, include_dir};

pub use super::super::source::Source;

static EMBEDDED: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/.agents");

/// The subdirectory `include_dir!` was pointed at, and what `core::source`
/// resolves against — kept as a constant so the two cannot name different
/// paths by a typo.
pub const SUBDIR: &str = ".agents";

/// Points the harness's own source at a checkout explicitly, the way
/// `PINST_SOURCE` overrides `configs/` and `PINST_DOCS_SOURCE` overrides the
/// catalogue.
pub const SOURCE_ENV: &str = "PINST_HARNESS_SOURCE";

/// Resolves where `.agents/` is read from for this run: an explicit
/// `$PINST_HARNESS_SOURCE`, else a checkout found at the working directory or
/// above the binary, else the copy embedded at compile time.
pub fn resolve_source() -> Source {
    super::super::source::resolve(SUBDIR, SOURCE_ENV)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssetKind {
    Skill,
    Command,
}

/// One thing the harness installs: a skill directory or a single command
/// file, named the way a vendor directory names it (a skill's directory
/// name, a command's file stem).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Asset {
    pub kind: AssetKind,
    pub name: String,
    /// Every file that belongs to this asset, relative to `.agents/`. A
    /// command is one entry; a skill is `SKILL.md` plus whatever else lives
    /// in its directory, because a skill can carry references and scripts a
    /// single-file view would silently drop (RISK-005).
    pub files: Vec<PathBuf>,
}

impl Asset {
    /// Where this asset's *entry point* lives under `.agents/` — `SKILL.md`
    /// for a skill, the command file itself for a command. What a vendor
    /// directory links or copies to.
    pub fn entry(&self) -> &Path {
        match self.kind {
            AssetKind::Skill => self
                .files
                .iter()
                .find(|f| f.file_name().is_some_and(|n| n == "SKILL.md"))
                .unwrap_or(&self.files[0]),
            AssetKind::Command => &self.files[0],
        }
    }
}

/// Every skill under `skills/*/` and every command under `commands/*.md`, in
/// name order within each kind.
pub fn enumerate(source: &Source) -> Result<Vec<Asset>> {
    let mut assets = match source {
        Source::Tree(root) => enumerate_tree(root)?,
        Source::Embedded => enumerate_embedded(),
    };
    assets.sort();
    Ok(assets)
}

fn enumerate_tree(root: &Path) -> Result<Vec<Asset>> {
    let mut assets = Vec::new();

    let skills_dir = root.join("skills");
    if skills_dir.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&skills_dir)
            .with_context(|| format!("reading {}", skills_dir.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        entries.sort();
        for dir in entries {
            let name = dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            let mut files = Vec::new();
            walk_tree(root, &dir, &mut files);
            files.sort();
            if !files.is_empty() {
                assets.push(Asset {
                    kind: AssetKind::Skill,
                    name,
                    files,
                });
            }
        }
    }

    let commands_dir = root.join("commands");
    if commands_dir.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&commands_dir)
            .with_context(|| format!("reading {}", commands_dir.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "md"))
            .collect();
        entries.sort();
        for file in entries {
            let name = file
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            let relative = file.strip_prefix(root).unwrap_or(&file).to_path_buf();
            assets.push(Asset {
                kind: AssetKind::Command,
                name,
                files: vec![relative],
            });
        }
    }

    Ok(assets)
}

/// Recursive within a skill directory, exactly as `configs::walk` is within a
/// config package: a skill may carry more than one file.
fn walk_tree(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if file_type.is_dir() {
            walk_tree(root, &path, out);
        } else if file_type.is_file()
            && let Ok(relative) = path.strip_prefix(root)
        {
            out.push(relative.to_path_buf());
        }
    }
}

fn enumerate_embedded() -> Vec<Asset> {
    let mut assets = Vec::new();

    if let Some(skills) = EMBEDDED.get_dir("skills") {
        let mut dirs: Vec<&Dir<'_>> = skills.dirs().collect();
        dirs.sort_by_key(|d| d.path().to_path_buf());
        for dir in dirs {
            let name = dir
                .path()
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            let mut files: Vec<PathBuf> = Vec::new();
            walk_embedded(dir, &mut files);
            files.sort();
            if !files.is_empty() {
                assets.push(Asset {
                    kind: AssetKind::Skill,
                    name,
                    files,
                });
            }
        }
    }

    if let Some(commands) = EMBEDDED.get_dir("commands") {
        let mut files: Vec<_> = commands.files().collect();
        files.sort_by_key(|f| f.path().to_path_buf());
        for file in files {
            let path = file.path();
            if path.extension().is_none_or(|e| e != "md") {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            assets.push(Asset {
                kind: AssetKind::Command,
                name,
                files: vec![path.to_path_buf()],
            });
        }
    }

    assets
}

fn walk_embedded(dir: &Dir<'_>, out: &mut Vec<PathBuf>) {
    for file in dir.files() {
        out.push(file.path().to_path_buf());
    }
    for sub in dir.dirs() {
        walk_embedded(sub, out);
    }
}

/// The bytes at `relative` (relative to `.agents/`), from whichever source is
/// in play.
pub fn content(source: &Source, relative: &Path) -> Result<Vec<u8>> {
    match source {
        Source::Tree(root) => {
            let path = root.join(relative);
            std::fs::read(&path).with_context(|| format!("reading {}", path.display()))
        }
        Source::Embedded => {
            let key = relative.to_string_lossy();
            let entry = EMBEDDED
                .get_file(key.as_ref())
                .ok_or_else(|| color_eyre::eyre::eyre!("embedded harness asset missing: {key}"))?;
            Ok(entry.contents().to_vec())
        }
    }
}

/// The absolute path to `relative` in a tree source, for symlinking. Not
/// meaningful for `Source::Embedded`, which has no path on disk.
pub fn source_path(source: &Source, relative: &Path) -> Result<PathBuf> {
    match source {
        Source::Tree(root) => Ok(root.join(relative)),
        Source::Embedded => bail!("no source path: this binary carries .agents/ embedded"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> Source {
        Source::Tree(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(SUBDIR))
    }

    #[test]
    fn the_checkout_tree_and_the_embedded_copy_enumerate_the_same_assets() {
        // The same property `docs` and `configs` already assert: what a dev
        // machine reads off disk and what a provisioned machine reads out of
        // the binary must not be able to disagree (LESSON-015).
        let from_tree = enumerate(&tree()).unwrap();
        let from_binary = enumerate(&Source::Embedded).unwrap();

        let tree_names: Vec<(AssetKind, &str)> = from_tree
            .iter()
            .map(|a| (a.kind, a.name.as_str()))
            .collect();
        let binary_names: Vec<(AssetKind, &str)> = from_binary
            .iter()
            .map(|a| (a.kind, a.name.as_str()))
            .collect();
        assert_eq!(tree_names, binary_names);

        for (t, b) in from_tree.iter().zip(from_binary.iter()) {
            for (tf, bf) in t.files.iter().zip(b.files.iter()) {
                assert_eq!(tf, bf);
                let tree_bytes = content(&tree(), tf).unwrap();
                let binary_bytes = content(&Source::Embedded, bf).unwrap();
                assert_eq!(tree_bytes, binary_bytes, "{} differs", tf.display());
            }
        }
    }

    #[test]
    fn enumeration_finds_every_skill_and_command_in_this_repo() {
        let assets = enumerate(&tree()).unwrap();
        let skills: Vec<&str> = assets
            .iter()
            .filter(|a| a.kind == AssetKind::Skill)
            .map(|a| a.name.as_str())
            .collect();
        let commands: Vec<&str> = assets
            .iter()
            .filter(|a| a.kind == AssetKind::Command)
            .map(|a| a.name.as_str())
            .collect();
        for expected in [
            "conventional-commits",
            "create-pr",
            "plan-write",
            "plan-implement",
            "plan-learnings",
            "pinst",
        ] {
            assert!(skills.contains(&expected), "missing skill {expected}");
        }
        for expected in ["plan", "implement", "learn", "cc", "pr", "distil"] {
            assert!(commands.contains(&expected), "missing command {expected}");
        }
    }

    #[test]
    fn a_skill_carries_every_file_in_its_directory_not_only_skill_md() {
        let dir = tempfile::tempdir().unwrap();
        let skill = dir.path().join("skills/multi-file");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(skill.join("SKILL.md"), "---\nname: multi-file\n---\n").unwrap();
        std::fs::create_dir_all(skill.join("references")).unwrap();
        std::fs::write(skill.join("references/notes.md"), "extra").unwrap();

        let assets = enumerate(&Source::Tree(dir.path().to_path_buf())).unwrap();
        let asset = assets.iter().find(|a| a.name == "multi-file").unwrap();
        assert_eq!(asset.files.len(), 2, "{:?}", asset.files);
        assert_eq!(asset.entry().file_name().unwrap(), "SKILL.md");
    }

    #[test]
    fn an_empty_skill_directory_produces_no_asset() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("skills/ghost")).unwrap();
        let assets = enumerate(&Source::Tree(dir.path().to_path_buf())).unwrap();
        assert!(assets.iter().all(|a| a.name != "ghost"));
    }

    #[test]
    fn the_source_env_var_redirects_the_harness_at_another_checkout() {
        let _guard = crate::core::source::test_env_lock();
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("manifest.toml"), "").unwrap();
        std::fs::create_dir_all(root.path().join(SUBDIR).join("commands")).unwrap();
        std::fs::write(root.path().join(SUBDIR).join("commands/x.md"), "hello").unwrap();

        unsafe { std::env::set_var(SOURCE_ENV, root.path()) };
        let source = resolve_source();
        unsafe { std::env::remove_var(SOURCE_ENV) };

        let assets = enumerate(&source).unwrap();
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].name, "x");
    }
}
