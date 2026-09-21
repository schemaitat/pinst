use color_eyre::eyre::{Result, bail};

use super::Executor;
use crate::core::manifest::{Install, Tool};
use crate::core::plan::Action;

/// Homebrew refuses to run as root, so — unlike every other executor in this
/// module — this one must never prefix `sudo` onto the command it emits. The
/// `Runner` applies no elevation of its own; a `sudo brew` would have to come
/// from here, so leaving it out is the whole guarantee (RISK-002).
pub struct Brew;

impl Executor for Brew {
    fn install(&self, tool: &Tool, install: &Install) -> Result<Vec<Action>> {
        let Install::Brew { formulae, cask } = install else {
            bail!("brew executor called for a non-brew tool '{}'", tool.name);
        };
        let flag = if *cask { "--cask " } else { "" };
        Ok(vec![Action::Shell {
            command: format!("brew install {flag}{}", formulae.join(" ")),
        }])
    }

    fn upgrade(&self, tool: &Tool, install: &Install) -> Result<Vec<Action>> {
        let Install::Brew { formulae, cask } = install else {
            bail!("brew executor called for a non-brew tool '{}'", tool.name);
        };
        let flag = if *cask { "--cask " } else { "" };
        Ok(vec![Action::Shell {
            command: format!("brew upgrade {flag}{}", formulae.join(" ")),
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool() -> Tool {
        crate::core::manifest::embedded()
            .unwrap()
            .tool("curl")
            .unwrap()
            .clone()
    }

    fn action_command(actions: &[Action]) -> &str {
        match &actions[0] {
            Action::Shell { command } => command,
            other => panic!("expected a shell action, got {other:?}"),
        }
    }

    #[test]
    fn install_lists_every_formula() {
        let install = Install::Brew {
            formulae: vec!["a".to_string(), "b".to_string()],
            cask: false,
        };
        let actions = Brew.install(&tool(), &install).unwrap();
        assert_eq!(action_command(&actions), "brew install a b");
    }

    #[test]
    fn cask_true_adds_the_flag() {
        let install = Install::Brew {
            formulae: vec!["some-app".to_string()],
            cask: true,
        };
        let actions = Brew.install(&tool(), &install).unwrap();
        assert_eq!(action_command(&actions), "brew install --cask some-app");
    }

    #[test]
    fn upgrade_emits_brew_upgrade() {
        let install = Install::Brew {
            formulae: vec!["a".to_string()],
            cask: false,
        };
        let actions = Brew.upgrade(&tool(), &install).unwrap();
        assert_eq!(action_command(&actions), "brew upgrade a");
    }

    // TEST-008 / RISK-002 / SEC-002: Homebrew refuses to run as root, so
    // nothing this executor emits may ask for sudo.
    #[test]
    fn never_emits_sudo() {
        let install = Install::Brew {
            formulae: vec!["a".to_string()],
            cask: true,
        };
        for actions in [
            Brew.install(&tool(), &install).unwrap(),
            Brew.upgrade(&tool(), &install).unwrap(),
        ] {
            for action in &actions {
                if let Action::Shell { command } = action {
                    assert!(!command.contains("sudo"), "{command}");
                }
            }
        }
    }
}
