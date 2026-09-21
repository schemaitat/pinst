use color_eyre::eyre::{Result, bail};

use super::Executor;
use crate::core::manifest::{Install, Tool};
use crate::core::plan::Action;

pub struct Nvm;

impl Executor for Nvm {
    fn install(&self, tool: &Tool, install: &Install) -> Result<Vec<Action>> {
        let Install::Nvm { version } = install else {
            bail!("nvm executor called for tool '{}'", tool.name);
        };
        // nvm is a shell function, not a binary: it has to be sourced into
        // the same shell that then calls it.
        Ok(vec![Action::Shell {
            command: format!(
                "export NVM_DIR=\"$HOME/.nvm\" && . \"$NVM_DIR/nvm.sh\" && nvm install {version}"
            ),
        }])
    }
}
