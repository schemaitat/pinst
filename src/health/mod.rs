//! Dotfiles Health checks: Stow symlink integrity per package (TASK-011)
//! plus a PATH cross-check for registry tools that reuses Phase 2's probe
//! results instead of re-running subprocess checks (TASK-012).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::probe::ProbeResult;
use crate::registry::ToolSpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkStatus {
    Ok,
    Missing,
    Broken,
    /// A plain (non-symlink) file sits at the stow target and its contents
    /// differ from the repo's tracked version. Deliberately does NOT fire
    /// for a plain file that matches the repo (e.g. one pulled in via
    /// `just adopt`) — see RISK-004.
    Conflict,
}

#[derive(Debug, Clone)]
pub struct FileHealth {
    pub package: String,
    pub relative_path: String,
    pub target: PathBuf,
    pub status: LinkStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPathStatus {
    OnPath,
    Missing,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ToolHealth {
    pub tool: String,
    pub status: ToolPathStatus,
}

/// Reads the `packages` variable out of the dotfiles `justfile` (the same
/// list `just install`/`just restow` use) so pinst's package list can never
/// drift from the one actually driving Stow. Falls back to the current
/// README-documented default if the justfile can't be parsed.
pub fn dotfiles_packages(dotfiles_dir: &Path) -> Vec<String> {
    let justfile = dotfiles_dir.join("justfile");
    if let Ok(contents) = fs::read_to_string(&justfile) {
        for line in contents.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("packages")
                && let Some(rest) = rest.trim_start().strip_prefix(":=")
            {
                let rest = rest.trim().trim_matches('"');
                let pkgs: Vec<String> = rest.split_whitespace().map(str::to_string).collect();
                if !pkgs.is_empty() {
                    return pkgs;
                }
            }
        }
    }
    vec![
        "nvim".to_string(),
        "zsh".to_string(),
        "herdr".to_string(),
        "git".to_string(),
    ]
}

pub fn check_files(dotfiles_dir: &Path, home_dir: &Path) -> Vec<FileHealth> {
    let mut results = Vec::new();
    for package in dotfiles_packages(dotfiles_dir) {
        let package_dir = dotfiles_dir.join(&package);
        let mut files = Vec::new();
        walk_files(&package_dir, &package_dir, &mut files);
        files.sort();
        for relative in files {
            let source = package_dir.join(&relative);
            let target = home_dir.join(&relative);
            let status = classify(&source, &target);
            results.push(FileHealth {
                package: package.clone(),
                relative_path: relative.to_string_lossy().into_owned(),
                target,
                status,
            });
        }
    }
    results
}

fn walk_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if file_type.is_dir() {
            walk_files(root, &path, out);
        } else if file_type.is_file() {
            // Regular files only: some packages (e.g. herdr, which writes
            // its live log/socket straight through the directory-level Stow
            // symlink) leave non-file runtime state sitting inside the
            // package dir. Sockets/FIFOs aren't dotfiles to track, and
            // trying to `read()` one for the Conflict content-diff below
            // would just fail and misreport it as a conflict.
            if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_path_buf());
            }
        }
    }
}

fn classify(source: &Path, target: &Path) -> LinkStatus {
    let Ok(meta) = fs::symlink_metadata(target) else {
        return LinkStatus::Missing;
    };
    if meta.file_type().is_symlink() {
        match (fs::canonicalize(target), fs::canonicalize(source)) {
            (Ok(resolved), Ok(expected)) if resolved == expected => LinkStatus::Ok,
            _ => LinkStatus::Broken,
        }
    } else {
        match (fs::read(target), fs::read(source)) {
            (Ok(t), Ok(s)) if t == s => LinkStatus::Ok,
            _ => LinkStatus::Conflict,
        }
    }
}

/// PATH-checks every registry tool: reuses an already-completed probe when
/// one exists (TASK-012's "avoid duplicating Phase 2's subprocess calls"),
/// falling back to an independent `which` lookup only for tools that carry
/// a `bin_name` but haven't been probed yet.
pub fn check_tool_paths(
    registry: &[ToolSpec],
    probes: &BTreeMap<String, ProbeResult>,
) -> Vec<ToolHealth> {
    registry
        .iter()
        .map(|spec| {
            let status = if let Some(probe) = probes.get(&spec.name) {
                if probe.installed {
                    ToolPathStatus::OnPath
                } else {
                    ToolPathStatus::Missing
                }
            } else if let Some(bin) = &spec.bin_name {
                if which::which(bin).is_ok() {
                    ToolPathStatus::OnPath
                } else {
                    ToolPathStatus::Missing
                }
            } else {
                ToolPathStatus::Unknown
            };
            ToolHealth {
                tool: spec.name.clone(),
                status,
            }
        })
        .collect()
}
