use color_eyre::eyre::{Result, bail};

use super::Executor;
use crate::core::manifest::{Install, Tool};
use crate::core::plan::Action;

/// Escape hatch for installers whose documented invocation does not fit the
/// piped-curl shape (rustup's --proto flags, oh-my-zsh's `sh -c "$(curl ...)"`).
pub struct ShellInstall;

impl Executor for ShellInstall {
    fn install(&self, tool: &Tool, install: &Install) -> Result<Vec<Action>> {
        let Install::Shell { command } = install else {
            bail!("shell executor called for tool '{}'", tool.name);
        };
        Ok(vec![Action::Shell {
            command: command.clone(),
        }])
    }
}
