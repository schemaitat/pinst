use color_eyre::eyre::{Result, bail};

use super::Executor;
use crate::core::manifest::{Install, Tool};
use crate::core::plan::Action;

pub struct CurlScript;

impl Executor for CurlScript {
    fn install(&self, tool: &Tool, install: &Install) -> Result<Vec<Action>> {
        let Install::CurlScript {
            url,
            shell,
            args,
            sha256,
            ..
        } = install
        else {
            bail!("curl_script executor called for tool '{}'", tool.name);
        };
        let verify = sha256
            .as_deref()
            .map(|hash| super::verify_hash(hash, "\"$tmp/install\""))
            .transpose()?
            .unwrap_or_else(|| ":".into());
        let arguments = args
            .iter()
            .map(|arg| super::quote(arg))
            .collect::<Vec<_>>()
            .join(" ");
        let command = format!(
            "set -eu; tmp=$(mktemp -d); trap 'rm -rf \"$tmp\"' EXIT; {} {} -o \"$tmp/install\"; {verify}; {} \"$tmp/install\" {arguments}",
            super::CURL,
            super::quote(url),
            super::quote(shell)
        );
        Ok(vec![Action::Shell { command }])
    }
}
