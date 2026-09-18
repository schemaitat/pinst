use color_eyre::eyre::Result;

use crate::cli::SelectArgs;
use crate::cli::output::{Ctx, ExitCode};

/// One-shot provisioning for a fresh machine: install the profile's tools,
/// lay down the configs, and finish with a diagnosis.
///
/// Identical in mechanism to `apply` — the difference is the default
/// selection. `apply` defaults to the whole manifest because it is run on a
/// machine that already has one; `bootstrap` defaults to the `default`
/// profile, which is the answer to "what does a new box need?".
pub async fn run(ctx: &Ctx, args: &SelectArgs) -> Result<ExitCode> {
    let mut args = args.clone();
    if args.profile.is_none() && args.tags.is_empty() && args.tools.is_empty() {
        args.profile = Some("default".to_string());
    }

    ctx.note(format!(
        "bootstrapping with profile '{}'",
        args.profile.as_deref().unwrap_or("(explicit selection)")
    ));
    if ctx.dry_run {
        ctx.note("dry run: nothing will be installed or written");
    }

    super::apply::converge(ctx, &args, "bootstrap").await
}
