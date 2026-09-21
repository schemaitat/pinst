use color_eyre::eyre::{Result, bail};

use super::Executor;
use crate::core::manifest::{Install, Tool};
use crate::core::plan::Action;

pub struct CurlScript;

impl Executor for CurlScript {
    fn install(&self, tool: &Tool, install: &Install) -> Result<Vec<Action>> {
        let Install::CurlScript {
            url, shell, args, ..
        } = install
        else {
            bail!("curl_script executor called for tool '{}'", tool.name);
        };
        let mut command = format!("curl -fsSL {url} | {shell} -s");
        if !args.is_empty() {
            command.push_str(" --");
            for arg in args {
                command.push(' ');
                command.push_str(arg);
            }
        }
        Ok(vec![Action::Shell { command }])
    }
}
