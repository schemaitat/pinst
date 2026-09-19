//! The pinst engine. Everything that decides or does something lives here;
//! `cli` and `ui` are thin frontends over this module and must not contain
//! logic of their own.

pub mod configs;
pub mod doctor;
pub mod engine;
pub mod exec;
pub mod graph;
pub mod manifest;
pub mod plan;
pub mod probe;
pub mod template;
pub mod upgrade;

use std::path::PathBuf;

/// Marks a failure caused by *how pinst was asked* — an unknown tool or
/// profile, a missing or invalid manifest — rather than by something going
/// wrong while doing the work. `main` maps it to exit code 2, the
/// documented usage-error code, so an agent can tell a bad invocation from
/// a real failure.
#[derive(Debug)]
pub struct UsageError(pub String);

impl std::fmt::Display for UsageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for UsageError {}

/// Builds a `UsageError` report.
pub fn usage(message: impl Into<String>) -> color_eyre::eyre::Report {
    color_eyre::eyre::Report::new(UsageError(message.into()))
}

/// `$HOME`, or a clear failure — every config path is relative to it, so
/// guessing would write someone's dotfiles into the wrong place.
pub fn home_dir() -> color_eyre::eyre::Result<PathBuf> {
    match std::env::var_os("HOME") {
        Some(home) if !home.is_empty() => Ok(PathBuf::from(home)),
        _ => color_eyre::eyre::bail!("HOME is not set; pinst cannot resolve config targets"),
    }
}
