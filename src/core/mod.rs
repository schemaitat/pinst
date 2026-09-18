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

/// `$HOME`, or a clear failure — every config path is relative to it, so
/// guessing would write someone's dotfiles into the wrong place.
pub fn home_dir() -> color_eyre::eyre::Result<PathBuf> {
    match std::env::var_os("HOME") {
        Some(home) if !home.is_empty() => Ok(PathBuf::from(home)),
        _ => color_eyre::eyre::bail!("HOME is not set; pinst cannot resolve config targets"),
    }
}
