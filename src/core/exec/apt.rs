use color_eyre::eyre::{Result, bail};

use super::Executor;
use crate::core::manifest::{Install, Tool};
use crate::core::plan::Action;

pub struct Apt;

impl Executor for Apt {
    fn install(&self, tool: &Tool, install: &Install) -> Result<Vec<Action>> {
        let Install::Apt { packages } = install else {
            bail!("apt executor called for a non-apt tool '{}'", tool.name);
        };
        Ok(vec![Action::Shell {
            command: format!("sudo apt-get install -y {}", packages.join(" ")),
        }])
    }

    fn upgrade(&self, tool: &Tool, install: &Install) -> Result<Vec<Action>> {
        let Install::Apt { packages } = install else {
            bail!("apt executor called for a non-apt tool '{}'", tool.name);
        };
        // --only-upgrade keeps an upgrade from silently installing something
        // that was removed out from under the manifest.
        Ok(vec![Action::Shell {
            command: format!(
                "sudo apt-get install -y --only-upgrade {}",
                packages.join(" ")
            ),
        }])
    }
}
