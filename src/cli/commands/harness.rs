//! `pinst harness` — the checks over this repo's own agent corpus.
//!
//! The odd one out among the command families, in two ways worth stating
//! where someone will read them. It loads **no manifest**: its subject is the
//! repo the caller is standing in, not the machine. And it is **synchronous**
//! — nothing here probes a tool or talks to the network, so there is no
//! reason to make every caller `.await` a future that never yields.
//!
//! The exit codes carry the verdict as everywhere else: `0` clean, `2` a
//! usage error, `3` ran fine and found things to act on.

use color_eyre::eyre::Result;
use schemars::JsonSchema;
use serde::Serialize;

use crate::cli::output::{Ctx, Envelope, ExitCode, Status};
use crate::cli::{HarnessAction, HarnessArgs, HarnessNewIdArgs};
use crate::core::harness::id;

/// What `new-id` returns. One field, but an envelope item all the same: an
/// agent that has learned to read `items[0]` from every other command should
/// not have to special-case this one.
#[derive(Debug, Serialize, JsonSchema)]
pub struct MintedId {
    pub id: String,
}

pub fn run(ctx: &Ctx, args: &HarnessArgs) -> Result<ExitCode> {
    match &args.action {
        HarnessAction::NewId(new_id) => mint(ctx, new_id),
    }
}

/// Prints the bare id on stdout in human mode.
///
/// Deliberately unadorned: `scripts/ash.sh new-id` printed one line, and the
/// `/plan` command substitutes its output straight into a prompt. Anything
/// else here — a label, a trailing note — becomes part of a plan directory
/// name the first time someone pastes it.
fn mint(ctx: &Ctx, args: &HarnessNewIdArgs) -> Result<ExitCode> {
    let id = id::mint(args.date.as_deref())?;
    if !ctx.json {
        println!("{id}");
    }
    ctx.finish(Envelope::new(
        "harness new-id",
        Status::Ok,
        vec![MintedId { id }],
    ))
}
