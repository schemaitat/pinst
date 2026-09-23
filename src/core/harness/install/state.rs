//! What each managed path actually holds, compared against `.agents/`.
//!
//! The vocabulary mirrors `core::configs::FileState` on purpose — an
//! operator who has read `pinst config status` should not have to learn a
//! second one for the same six situations (`PAT-002`, `GUD-001`). The
//! difference from `configs` is `Unmanaged`: a config package is a fixed,
//! known list of files, but a vendor's skills or commands directory is a
//! directory a person also puts things in by hand, and `pinst harness
//! status` has to say so rather than pretend it does not exist — this is
//! what carries over the equivalent finding the old bash wiring script
//! used to emit for the same situation.

use std::path::{Path, PathBuf};

use color_eyre::eyre::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::super::asset::{self, Asset, AssetKind, Source};
use super::super::vendor::Vendor;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetState {
    /// A symlink pointing at the source tree.
    Linked,
    /// A real file/directory whose content matches the source.
    Copied,
    /// Nothing at the target path.
    Missing,
    /// A real file/directory whose content differs from the source.
    Drifted,
    /// A symlink pointing somewhere else.
    Foreign,
    /// Something at a managed vendor directory that names no asset this
    /// harness knows about — someone's own skill or command, sitting beside
    /// ours.
    Unmanaged,
}

impl AssetState {
    /// Whether this state means "installed and correct" — the same
    /// satisfied/not split `configs::ConfigSet::build_plan_where` makes.
    pub fn satisfied(self) -> bool {
        matches!(self, AssetState::Linked | AssetState::Copied)
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AssetStatus {
    pub vendor: Vendor,
    pub kind: AssetKindLabel,
    pub name: String,
    pub target: PathBuf,
    pub state: AssetState,
}

/// `AssetKind`, spelled for serialization — `AssetKind` itself carries no
/// `Serialize` because it lives in `asset`, which nothing else needed to
/// serialize before this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetKindLabel {
    Skill,
    Command,
}

impl From<AssetKind> for AssetKindLabel {
    fn from(kind: AssetKind) -> Self {
        match kind {
            AssetKind::Skill => AssetKindLabel::Skill,
            AssetKind::Command => AssetKindLabel::Command,
        }
    }
}

impl From<AssetKindLabel> for AssetKind {
    fn from(kind: AssetKindLabel) -> Self {
        match kind {
            AssetKindLabel::Skill => AssetKind::Skill,
            AssetKindLabel::Command => AssetKind::Command,
        }
    }
}

impl AssetKindLabel {
    pub fn label(self) -> &'static str {
        match self {
            AssetKindLabel::Skill => "skill",
            AssetKindLabel::Command => "command",
        }
    }
}

/// Where an asset's source lives under `.agents/`, as a single relative
/// path — a skill's directory, or a command's one file. What `classify`
/// compares the target against, and what a symlink target is expected to
/// resolve to.
pub fn source_relative(asset: &Asset) -> PathBuf {
    match asset.kind {
        AssetKind::Skill => Path::new("skills").join(&asset.name),
        AssetKind::Command => asset.entry().to_path_buf(),
    }
}

/// Where this asset is installed for a vendor at a scope's root.
pub fn target_path(vendor: Vendor, root: &Path, asset: &Asset) -> PathBuf {
    match asset.kind {
        AssetKind::Skill => vendor.skills_dir(root).join(&asset.name),
        AssetKind::Command => vendor.commands_dir(root).join(format!("{}.md", asset.name)),
    }
}

/// One asset's installed state at `target`.
pub fn classify(source: &Source, asset: &Asset, target: &Path) -> Result<AssetState> {
    let Ok(meta) = std::fs::symlink_metadata(target) else {
        return Ok(AssetState::Missing);
    };

    if meta.file_type().is_symlink() {
        let expected = match source {
            Source::Tree(_) => asset::source_path(source, &source_relative(asset)).ok(),
            Source::Embedded => None,
        };
        let matches = match (expected, target.canonicalize().ok()) {
            (Some(expected), Some(actual)) => expected
                .canonicalize()
                .map(|e| e == actual)
                .unwrap_or(false),
            _ => false,
        };
        return Ok(if matches {
            AssetState::Linked
        } else {
            AssetState::Foreign
        });
    }

    let matches = match asset.kind {
        AssetKind::Command => {
            let expected = asset::content(source, asset.entry())?;
            std::fs::read(target).ok() == Some(expected)
        }
        AssetKind::Skill => dir_matches(source, asset, target)?,
    };
    Ok(if matches {
        AssetState::Copied
    } else {
        AssetState::Drifted
    })
}

/// Every file a skill carries, matched byte for byte against a real
/// directory. A file the target is missing, or carries an extra of, is a
/// mismatch — not a partial match, because a skill installed by copy has to
/// be trusted whole.
fn dir_matches(source: &Source, asset: &Asset, target: &Path) -> Result<bool> {
    if !target.is_dir() {
        return Ok(false);
    }
    let root = Path::new("skills").join(&asset.name);
    for file in &asset.files {
        let Ok(within) = file.strip_prefix(&root) else {
            continue;
        };
        let expected = asset::content(source, file)?;
        let actual = std::fs::read(target.join(within)).ok();
        if actual.as_ref() != Some(&expected) {
            return Ok(false);
        }
    }
    let mut on_disk = 0usize;
    count_files(target, &mut on_disk);
    Ok(on_disk == asset.files.len())
}

fn count_files(dir: &Path, count: &mut usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            count_files(&entry.path(), count);
        } else if file_type.is_file() {
            *count += 1;
        }
    }
}

/// Every asset's state, plus anything sitting in the vendor's directories
/// that names no known asset (`Unmanaged`).
pub fn status(source: &Source, vendor: Vendor, root: &Path) -> Result<Vec<AssetStatus>> {
    let assets = asset::enumerate(source)?;
    let mut out = Vec::new();
    let mut known_skills = std::collections::BTreeSet::new();
    let mut known_commands = std::collections::BTreeSet::new();

    for asset in &assets {
        let target = target_path(vendor, root, asset);
        let state = classify(source, asset, &target)?;
        match asset.kind {
            AssetKind::Skill => {
                known_skills.insert(asset.name.clone());
            }
            AssetKind::Command => {
                known_commands.insert(asset.name.clone());
            }
        }
        out.push(AssetStatus {
            vendor,
            kind: asset.kind.into(),
            name: asset.name.clone(),
            target,
            state,
        });
    }

    out.extend(unmanaged_entries(
        vendor.skills_dir(root),
        vendor,
        AssetKindLabel::Skill,
        &known_skills,
    ));
    out.extend(unmanaged_entries(
        vendor.commands_dir(root),
        vendor,
        AssetKindLabel::Command,
        &known_commands,
    ));

    Ok(out)
}

fn unmanaged_entries(
    dir: PathBuf,
    vendor: Vendor,
    kind: AssetKindLabel,
    known: &std::collections::BTreeSet<String>,
) -> Vec<AssetStatus> {
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = match kind {
            AssetKindLabel::Skill => path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string(),
            AssetKindLabel::Command => {
                if path.extension().is_none_or(|e| e != "md") {
                    continue;
                }
                path.file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_string()
            }
        };
        if !known.contains(&name) {
            found.push(AssetStatus {
                vendor,
                kind,
                name,
                target: path,
                state: AssetState::Unmanaged,
            });
        }
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_skill_tree() -> (tempfile::TempDir, Source) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("skills/demo")).unwrap();
        std::fs::write(
            dir.path().join("skills/demo/SKILL.md"),
            "---\nname: demo\n---\nbody\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("commands")).unwrap();
        std::fs::write(dir.path().join("commands/demo.md"), "command body").unwrap();
        let source = Source::Tree(dir.path().to_path_buf());
        (dir, source)
    }

    #[test]
    fn a_missing_target_is_missing() {
        let (_dir, source) = one_skill_tree();
        let assets = asset::enumerate(&source).unwrap();
        let skill = assets.iter().find(|a| a.name == "demo").unwrap();
        let target = tempfile::tempdir().unwrap();
        let state = classify(&source, skill, &target.path().join("nope")).unwrap();
        assert_eq!(state, AssetState::Missing);
    }

    #[test]
    fn a_correct_symlink_is_linked_and_a_dangling_one_is_foreign() {
        let (_dir, source) = one_skill_tree();
        let assets = asset::enumerate(&source).unwrap();
        let skill = assets.iter().find(|a| a.name == "demo").unwrap();

        let vendor_root = tempfile::tempdir().unwrap();
        let dest = vendor_root.path().join("linked");
        let src_path = asset::source_path(&source, &source_relative(skill)).unwrap();
        std::os::unix::fs::symlink(&src_path, &dest).unwrap();
        assert_eq!(classify(&source, skill, &dest).unwrap(), AssetState::Linked);

        let elsewhere = vendor_root.path().join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        let foreign = vendor_root.path().join("foreign");
        std::os::unix::fs::symlink(&elsewhere, &foreign).unwrap();
        assert_eq!(
            classify(&source, skill, &foreign).unwrap(),
            AssetState::Foreign
        );
    }

    #[test]
    fn a_copy_with_matching_content_is_copied_and_a_drifted_one_is_drifted() {
        let (_dir, source) = one_skill_tree();
        let assets = asset::enumerate(&source).unwrap();
        let skill = assets.iter().find(|a| a.name == "demo").unwrap();

        let vendor_root = tempfile::tempdir().unwrap();
        let dest = vendor_root.path().join("copied");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("SKILL.md"), "---\nname: demo\n---\nbody\n").unwrap();
        assert_eq!(classify(&source, skill, &dest).unwrap(), AssetState::Copied);

        std::fs::write(dest.join("SKILL.md"), "edited by hand").unwrap();
        assert_eq!(
            classify(&source, skill, &dest).unwrap(),
            AssetState::Drifted
        );
    }

    #[test]
    fn a_command_file_classifies_the_same_way_as_a_skill_directory() {
        let (_dir, source) = one_skill_tree();
        let assets = asset::enumerate(&source).unwrap();
        let command = assets
            .iter()
            .find(|a| a.name == "demo" && a.kind == AssetKind::Command)
            .unwrap();

        let vendor_root = tempfile::tempdir().unwrap();
        let dest = vendor_root.path().join("demo.md");
        std::fs::write(&dest, "command body").unwrap();
        assert_eq!(
            classify(&source, command, &dest).unwrap(),
            AssetState::Copied
        );
        std::fs::write(&dest, "different").unwrap();
        assert_eq!(
            classify(&source, command, &dest).unwrap(),
            AssetState::Drifted
        );
    }

    #[test]
    fn status_reports_an_entry_with_no_matching_source_asset_as_unmanaged() {
        let (_dir, source) = one_skill_tree();
        let root = tempfile::tempdir().unwrap();
        let skills_dir = Vendor::Claude.skills_dir(root.path());
        std::fs::create_dir_all(&skills_dir).unwrap();
        std::fs::create_dir_all(skills_dir.join("someone-elses-skill")).unwrap();
        std::fs::write(skills_dir.join("someone-elses-skill/SKILL.md"), "not ours").unwrap();

        let rows = status(&source, Vendor::Claude, root.path()).unwrap();
        let unmanaged = rows
            .iter()
            .find(|r| r.name == "someone-elses-skill")
            .unwrap();
        assert_eq!(unmanaged.state, AssetState::Unmanaged);
        // ...and it must never be reported as demo's own state.
        let demo = rows
            .iter()
            .find(|r| r.name == "demo" && r.kind == AssetKindLabel::Skill)
            .unwrap();
        assert_eq!(demo.state, AssetState::Missing);
    }
}
