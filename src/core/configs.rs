//! The configs pinst carries, and how they get into `$HOME`.
//!
//! This is what replaces GNU Stow and the `~/dotfiles` repo. The tree under
//! `configs/` is embedded into the binary at compile time, so a single
//! downloaded binary can lay down a machine's configs with no clone and no
//! network. When the source tree *is* present (a dev machine with this repo
//! checked out), pinst symlinks `$HOME` at it instead of materializing copies,
//! so edits round-trip without a rebuild.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use color_eyre::eyre::{Context, Result, bail};
use include_dir::{Dir, include_dir};
use schemars::JsonSchema;
use serde::Serialize;

use super::manifest::{ConfigPackage, Manifest};
use super::plan::{Action, Plan, Step, StepKind};
use super::template::{self, Values};

static EMBEDDED: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/configs");

/// Where config content is read from for this run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A `configs/` directory on disk. Targets are symlinked at it so edits
    /// in either direction are immediately live.
    Tree(PathBuf),
    /// The copy compiled into the binary. Targets are written as real files.
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

/// Resolves the config source: an explicit `$PINST_SOURCE`, else a `configs/`
/// directory next to the working directory or above the running binary
/// (cargo puts it at `target/<profile>/pinst`), else the embedded copy.
pub fn resolve_source() -> Source {
    if let Some(explicit) = std::env::var_os("PINST_SOURCE") {
        let path = PathBuf::from(explicit);
        let candidate = if path.ends_with("configs") {
            path
        } else {
            path.join("configs")
        };
        if candidate.is_dir() {
            return Source::Tree(candidate);
        }
    }

    let cwd = PathBuf::from("configs");
    if cwd.is_dir() {
        return Source::Tree(cwd);
    }

    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors().skip(1).take(4) {
            let candidate = ancestor.join("configs");
            if candidate.is_dir() {
                return Source::Tree(candidate);
            }
        }
    }

    Source::Embedded
}

/// One file pinst manages, and where it belongs in `$HOME`.
#[derive(Debug, Clone)]
pub struct ConfigFile {
    pub package: String,
    /// Path relative to `$HOME` (the package tree mirrors `$HOME`).
    pub relative: PathBuf,
    pub target: PathBuf,
    /// Source path on disk, when the source is a tree.
    pub source_path: Option<PathBuf>,
    pub templated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FileState {
    /// A symlink pointing at our source tree.
    Linked,
    /// A real file whose content matches what pinst carries.
    Materialized,
    /// Nothing at the target path.
    Missing,
    /// A real file whose content differs — someone edited it in place.
    Drifted,
    /// A symlink pointing somewhere else (e.g. still into `~/dotfiles`).
    Foreign,
    /// A templated file whose `${VAR}` values are not available. Reported
    /// rather than fatal: a fresh machine has no values file yet, and the
    /// rest of the run must still be able to proceed.
    Unrenderable,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FileStatus {
    pub package: String,
    pub path: String,
    pub target: PathBuf,
    pub state: FileState,
    pub templated: bool,
}

/// The config set: the files, the source they came from, and the values used
/// to render templated ones.
pub struct ConfigSet {
    pub source: Source,
    pub files: Vec<ConfigFile>,
    values: Values,
}

impl ConfigSet {
    pub fn load(manifest: &Manifest, home: &Path) -> Result<Self> {
        let source = resolve_source();
        let values = Values::load()?;
        let mut files = Vec::new();

        for package in &manifest.configs {
            collect_package(&source, package, home, &mut files)?;
        }
        files.sort_by(|a, b| (&a.package, &a.relative).cmp(&(&b.package, &b.relative)));

        Ok(Self {
            source,
            files,
            values,
        })
    }

    /// The bytes that belong at a file's target, with templating applied.
    pub fn content(&self, file: &ConfigFile) -> Result<Vec<u8>> {
        let raw = match &self.source {
            Source::Tree(root) => {
                let path = root.join(&file.package).join(&file.relative);
                std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?
            }
            Source::Embedded => {
                let key = embedded_key(&file.package, &file.relative);
                let entry = EMBEDDED
                    .get_file(&key)
                    .ok_or_else(|| color_eyre::eyre::eyre!("embedded config missing: {key}"))?;
                entry.contents().to_vec()
            }
        };

        if !file.templated {
            return Ok(raw);
        }

        let text = String::from_utf8(raw)
            .with_context(|| format!("templated config {} is not UTF-8", file.relative.display()))?;
        Ok(template::render(&text, &self.values)?.into_bytes())
    }

    pub fn classify(&self, file: &ConfigFile) -> Result<FileState> {
        if file.templated && self.content(file).is_err() {
            return Ok(FileState::Unrenderable);
        }

        let Ok(meta) = std::fs::symlink_metadata(&file.target) else {
            return Ok(FileState::Missing);
        };

        if meta.file_type().is_symlink() {
            let resolved = std::fs::read_link(&file.target).unwrap_or_default();
            let expected = file.source_path.as_ref();
            let matches = match (expected, resolved.canonicalize().ok()) {
                (Some(expected), Some(actual)) => {
                    expected.canonicalize().map(|e| e == actual).unwrap_or(false)
                }
                _ => false,
            };
            return Ok(if matches {
                FileState::Linked
            } else {
                FileState::Foreign
            });
        }

        let on_disk = std::fs::read(&file.target)
            .with_context(|| format!("reading {}", file.target.display()))?;
        let Ok(expected) = self.content(file) else {
            return Ok(FileState::Unrenderable);
        };
        Ok(if on_disk == expected {
            FileState::Materialized
        } else {
            FileState::Drifted
        })
    }

    /// Why a templated file cannot be rendered, for reporting.
    pub fn render_error(&self, file: &ConfigFile) -> Option<String> {
        self.content(file).err().map(|err| format!("{err:#}"))
    }

    pub fn status(&self) -> Result<Vec<FileStatus>> {
        self.files
            .iter()
            .map(|file| {
                Ok(FileStatus {
                    package: file.package.clone(),
                    path: file.relative.display().to_string(),
                    target: file.target.clone(),
                    state: self.classify(file)?,
                    templated: file.templated,
                })
            })
            .collect()
    }

    /// Builds the plan that brings `$HOME` in line with what pinst carries.
    ///
    /// Anything already in the desired state is skipped (idempotency); an
    /// existing real file that differs is backed up to a timestamped path
    /// rather than clobbered — the one behavior from `bootstrap.sh`'s stow
    /// retry that made re-running safe on a box with a stock `.zshrc`.
    pub fn build_plan(&self) -> Result<Plan> {
        self.build_plan_where(|_| true)
    }

    /// Builds the plan for only the files whose current state `accept`
    /// approves. `doctor --fix` uses this to leave drifted files alone —
    /// overwriting a hand-edit is a decision only a human should make.
    pub fn build_plan_where(&self, accept: impl Fn(FileState) -> bool) -> Result<Plan> {
        let mut plan = Plan::default();
        let stamp = timestamp();

        for file in &self.files {
            let state = self.classify(file)?;
            if !accept(state) {
                continue;
            }
            let id = format!("config:{}:{}", file.package, file.relative.display());
            let description = format!("~/{}", file.relative.display());

            let desired_is_link = self.source.is_tree() && !file.templated;
            let satisfied = match state {
                FileState::Linked => desired_is_link,
                FileState::Materialized => !desired_is_link,
                _ => false,
            };

            if satisfied {
                plan.push(
                    Step::new(id, StepKind::Config, description)
                        .tool(&file.package)
                        .skipped(match state {
                            FileState::Linked => "already linked",
                            _ => "already up to date",
                        }),
                );
                continue;
            }

            if state == FileState::Unrenderable {
                let reason = self
                    .render_error(file)
                    .unwrap_or_else(|| "cannot be rendered".to_string());
                plan.push(
                    Step::new(id, StepKind::Config, description)
                        .tool(&file.package)
                        .blocked(reason),
                );
                continue;
            }

            let mut actions = Vec::new();
            if matches!(state, FileState::Drifted) {
                let backup = backup_path(&file.target, &stamp);
                actions.push(Action::Backup {
                    path: file.target.clone(),
                    to: backup,
                });
            }

            if desired_is_link {
                let source_path = file
                    .source_path
                    .clone()
                    .expect("tree source always has a source path");
                actions.push(Action::Link {
                    source: source_path,
                    target: file.target.clone(),
                });
            } else {
                let content = self.content(file)?;
                actions.push(Action::Write {
                    target: file.target.clone(),
                    bytes: content.len(),
                    content: Arc::new(content),
                });
            }

            plan.push(
                Step::new(id, StepKind::Config, description)
                    .tool(&file.package)
                    .actions(actions),
            );
        }

        Ok(plan)
    }

    /// Copies on-disk edits back into the source tree. Only possible when the
    /// source is a tree — there is nowhere to write an embedded copy.
    pub fn adopt(&self, dry_run: bool) -> Result<Vec<FileStatus>> {
        let Source::Tree(root) = &self.source else {
            bail!(
                "nothing to adopt into: this binary is running from its embedded configs. \
                 Check out the pinst repo and set PINST_SOURCE to it."
            );
        };

        let mut adopted = Vec::new();
        for file in &self.files {
            if self.classify(file)? != FileState::Drifted {
                continue;
            }
            let dest = root.join(&file.package).join(&file.relative);
            if file.templated {
                // Adopting a rendered file would write the machine's own
                // values back over the placeholders.
                bail!(
                    "cannot adopt templated config {} — edit {} in the source tree instead",
                    file.relative.display(),
                    dest.display()
                );
            }
            if !dry_run {
                std::fs::copy(&file.target, &dest).with_context(|| {
                    format!("adopting {} -> {}", file.target.display(), dest.display())
                })?;
            }
            adopted.push(FileStatus {
                package: file.package.clone(),
                path: file.relative.display().to_string(),
                target: file.target.clone(),
                state: FileState::Drifted,
                templated: file.templated,
            });
        }
        Ok(adopted)
    }
}

fn collect_package(
    source: &Source,
    package: &ConfigPackage,
    home: &Path,
    out: &mut Vec<ConfigFile>,
) -> Result<()> {
    let relatives: Vec<PathBuf> = match source {
        Source::Tree(root) => {
            let dir = root.join(&package.source);
            if !dir.is_dir() {
                bail!("config package '{}' not found at {}", package.name, dir.display());
            }
            let mut found = Vec::new();
            walk(&dir, &dir, &mut found);
            found
        }
        Source::Embedded => {
            let mut found = Vec::new();
            let Some(dir) = EMBEDDED.get_dir(&package.source) else {
                bail!("config package '{}' is not embedded in this binary", package.name);
            };
            collect_embedded(dir, &package.source, &mut found);
            found
        }
    };

    for relative in relatives {
        let templated = package
            .templates
            .iter()
            .any(|t| Path::new(t) == relative.as_path());
        out.push(ConfigFile {
            package: package.name.clone(),
            target: home.join(&relative),
            source_path: match source {
                Source::Tree(root) => Some(root.join(&package.source).join(&relative)),
                Source::Embedded => None,
            },
            relative,
            templated,
        });
    }
    Ok(())
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if file_type.is_dir() {
            walk(root, &path, out);
        } else if file_type.is_file()
            && let Ok(relative) = path.strip_prefix(root)
        {
            // Regular files only: sockets and logs that live beside a config
            // (herdr writes both into its config dir) are runtime state, not
            // something to link into $HOME.
            out.push(relative.to_path_buf());
        }
    }
}

fn collect_embedded(dir: &Dir<'_>, package_root: &str, out: &mut Vec<PathBuf>) {
    for file in dir.files() {
        if let Ok(relative) = file.path().strip_prefix(package_root) {
            out.push(relative.to_path_buf());
        }
    }
    for sub in dir.dirs() {
        collect_embedded(sub, package_root, out);
    }
}

fn embedded_key(package: &str, relative: &Path) -> String {
    format!("{package}/{}", relative.display())
}

fn backup_path(target: &Path, stamp: &str) -> PathBuf {
    let mut name = target.as_os_str().to_os_string();
    name.push(format!(".pre-pinst.{stamp}"));
    PathBuf::from(name)
}

fn timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    secs.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::exec::Runner;
    use crate::core::manifest;

    /// The embedded tree must never carry a secret. `.zshrc` sources
    /// `~/.zshrc.secrets` at runtime; that file itself stays untracked.
    #[test]
    fn no_secrets_are_embedded() {
        let mut offenders = Vec::new();
        collect_all(&EMBEDDED, &mut offenders);
        for path in &offenders {
            let name = path.to_lowercase();
            assert!(
                !(name.contains("secret")
                    || name.contains("credential")
                    || name.ends_with(".pem")
                    || name.ends_with(".key")
                    || name.contains("id_rsa")
                    || name.contains(".env")),
                "secret-shaped file embedded in the binary: {path}"
            );
        }
        assert!(!offenders.is_empty(), "expected the config tree to be embedded");
    }

    fn collect_all(dir: &Dir<'_>, out: &mut Vec<String>) {
        for file in dir.files() {
            out.push(file.path().display().to_string());
        }
        for sub in dir.dirs() {
            collect_all(sub, out);
        }
    }

    fn set_for(home: &Path, source: Source) -> ConfigSet {
        let manifest = manifest::embedded().unwrap();
        let mut files = Vec::new();
        for package in &manifest.configs {
            collect_package(&source, package, home, &mut files).unwrap();
        }
        files.sort_by(|a, b| (&a.package, &a.relative).cmp(&(&b.package, &b.relative)));
        ConfigSet {
            source,
            files,
            values: Values::new(std::collections::BTreeMap::from([
                ("GIT_USER_NAME".to_string(), "Ada".to_string()),
                ("GIT_USER_EMAIL".to_string(), "ada@example.com".to_string()),
            ])),
        }
    }

    fn apply(set: &ConfigSet) {
        let plan = set.build_plan().unwrap();
        let runner = Runner::new(false);
        for step in plan.pending() {
            for action in &step.actions {
                runner.run(action).unwrap();
            }
        }
    }

    #[test]
    fn materialize_mode_writes_real_files_from_the_embedded_copy() {
        let home = tempfile::tempdir().unwrap();
        let set = set_for(home.path(), Source::Embedded);

        apply(&set);

        let zshrc = home.path().join(".zshrc");
        assert!(zshrc.is_file());
        assert!(!zshrc.symlink_metadata().unwrap().file_type().is_symlink());
        assert!(home.path().join(".config/nvim/init.lua").is_file());

        // Every file now reports as satisfied — the run is idempotent.
        assert!(set.build_plan().unwrap().pending_count() == 0);
    }

    #[test]
    fn link_mode_symlinks_at_the_source_tree() {
        let home = tempfile::tempdir().unwrap();
        let source = Source::Tree(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("configs"));
        let set = set_for(home.path(), source);

        apply(&set);

        let zshrc = home.path().join(".zshrc");
        assert!(
            zshrc.symlink_metadata().unwrap().file_type().is_symlink(),
            "a present source tree should be linked, not copied"
        );
        assert!(set.build_plan().unwrap().pending_count() == 0);
    }

    #[test]
    fn existing_files_are_backed_up_not_clobbered() {
        let home = tempfile::tempdir().unwrap();
        let zshrc = home.path().join(".zshrc");
        std::fs::write(&zshrc, "# the machine's own zshrc\n").unwrap();

        let set = set_for(home.path(), Source::Embedded);
        apply(&set);

        let backups: Vec<PathBuf> = std::fs::read_dir(home.path())
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with(".zshrc.pre-pinst."))
                    .unwrap_or(false)
            })
            .collect();

        assert_eq!(backups.len(), 1, "the prior file must survive under a backup name");
        assert_eq!(
            std::fs::read_to_string(&backups[0]).unwrap(),
            "# the machine's own zshrc\n"
        );
        assert!(std::fs::read_to_string(&zshrc).unwrap().contains("oh-my-zsh"));
    }

    #[test]
    fn drift_is_detected_and_adopted_back_into_the_source_tree() {
        let home = tempfile::tempdir().unwrap();
        // A throwaway copy of the tree, so adopting cannot touch the repo.
        let tree = tempfile::tempdir().unwrap();
        let configs = tree.path().join("configs");
        copy_tree(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("configs"),
            &configs,
        );

        let set = set_for(home.path(), Source::Tree(configs.clone()));
        apply(&set);

        // Linked files are edited through the link, so break the link first
        // to simulate a machine where the file was replaced outright.
        let zshrc = home.path().join(".zshrc");
        std::fs::remove_file(&zshrc).unwrap();
        std::fs::write(&zshrc, "# edited in place\n").unwrap();

        let drifted = set
            .status()
            .unwrap()
            .into_iter()
            .filter(|s| s.state == FileState::Drifted)
            .count();
        assert_eq!(drifted, 1);

        set.adopt(false).unwrap();
        assert_eq!(
            std::fs::read_to_string(configs.join("zsh/.zshrc")).unwrap(),
            "# edited in place\n"
        );
    }

    #[test]
    fn templated_configs_render_values() {
        let home = tempfile::tempdir().unwrap();
        let set = set_for(home.path(), Source::Embedded);
        apply(&set);

        let gitconfig = std::fs::read_to_string(home.path().join(".gitconfig")).unwrap();
        assert!(gitconfig.contains("name = Ada"), "{gitconfig}");
        assert!(gitconfig.contains("email = ada@example.com"));
        assert!(!gitconfig.contains("${"), "no placeholders may survive");
    }

    #[test]
    fn a_fresh_machine_without_values_still_gets_every_other_config() {
        // The state a brand-new box is in: no values.toml, so the templated
        // .gitconfig cannot render. That must not sink the whole run.
        let home = tempfile::tempdir().unwrap();
        let manifest = manifest::embedded().unwrap();
        let source = Source::Embedded;
        let mut files = Vec::new();
        for package in &manifest.configs {
            collect_package(&source, package, home.path(), &mut files).unwrap();
        }
        let set = ConfigSet {
            source,
            files,
            values: Values::new(std::collections::BTreeMap::new()),
        };

        let plan = set.build_plan().expect("planning must not fail on missing values");
        apply(&set);

        assert!(home.path().join(".zshrc").is_file(), "unrelated configs still land");
        assert!(!home.path().join(".gitconfig").exists(), "the unrenderable one is skipped");

        let blocked = plan
            .steps
            .iter()
            .filter(|s| matches!(s.state, crate::core::plan::StepState::Blocked(_)))
            .count();
        assert_eq!(blocked, 1, "the templated file is reported, not silently dropped");

        let gitconfig = set
            .status()
            .unwrap()
            .into_iter()
            .find(|s| s.path == ".gitconfig")
            .unwrap();
        assert_eq!(gitconfig.state, FileState::Unrenderable);
    }

    #[test]
    fn a_symlink_into_another_tree_is_reported_as_foreign() {
        // This is the state a machine is in before cutover: $HOME still
        // points into ~/dotfiles.
        let home = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let other = elsewhere.path().join("zshrc");
        std::fs::write(&other, "# someone else's\n").unwrap();
        std::os::unix::fs::symlink(&other, home.path().join(".zshrc")).unwrap();

        let set = set_for(home.path(), Source::Embedded);
        let status = set.status().unwrap();
        let zshrc = status.iter().find(|s| s.path == ".zshrc").unwrap();
        assert_eq!(zshrc.state, FileState::Foreign);
    }

    fn copy_tree(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap().flatten() {
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }
}
