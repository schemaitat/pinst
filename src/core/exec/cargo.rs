use color_eyre::eyre::{Result, bail};

use super::Executor;
use crate::core::manifest::{Install, Tool};
use crate::core::plan::Action;

pub struct Cargo;

impl Executor for Cargo {
    fn install(&self, tool: &Tool) -> Result<Vec<Action>> {
        let Install::Cargo { crate_name } = &tool.install else {
            bail!("cargo executor called for a non-cargo tool '{}'", tool.name);
        };
        Ok(vec![Action::Shell {
            command: format!("cargo install --locked {crate_name}"),
        }])
    }
}
