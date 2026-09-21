use color_eyre::eyre::{Result, bail};

use super::Executor;
use crate::core::manifest::{Install, Tool};
use crate::core::plan::Action;

pub struct GitClone;

impl Executor for GitClone {
    fn install(&self, tool: &Tool, install: &Install) -> Result<Vec<Action>> {
        let Install::GitClone { url, dest, depth } = install else {
            bail!("git_clone executor called for tool '{}'", tool.name);
        };
        let depth_arg = depth.map(|d| format!("--depth={d} ")).unwrap_or_default();
        Ok(vec![Action::Shell {
            command: format!("git clone {depth_arg}{url} \"{dest}\""),
        }])
    }

    fn upgrade(&self, tool: &Tool, install: &Install) -> Result<Vec<Action>> {
        let Install::GitClone { dest, .. } = install else {
            bail!("git_clone executor called for tool '{}'", tool.name);
        };
        Ok(vec![Action::Shell {
            command: format!("git -C \"{dest}\" pull --ff-only"),
        }])
    }
}
