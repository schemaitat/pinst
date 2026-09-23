//! The agent harness: the checks that keep this repo's `.ash/` plan corpus
//! and its `.agents/skills/` contracts honest.
//!
//! This was 1054 lines of bash in `scripts/ash.sh` until plan `260920-wtburh`
//! moved it here. The move is not a rewrite for its own sake: the script
//! spoke pinst's contract — a versioned JSON envelope, findings with a stable
//! id and a remediation, exit `0`/`2`/`3` — by hand, in `printf`, as a second
//! implementation of what `cli::output` and `core::doctor` already define in
//! types. What the binary could not do was ship it. pinst installs from a
//! GitHub release onto a machine with no clone; the corpus checks stayed
//! behind in a `scripts/` directory that machine never gets.
//!
//! Unlike `configs/` and `docs/tools/`, **nothing here is embedded**. Those
//! are trees pinst ships *to* a machine; a plan corpus belongs to whichever
//! repo the caller is standing in, and a compiled-in copy would answer about
//! the wrong one. What makes the command self-contained is that the logic is
//! in the binary — see `root` for why discovery is its own thing.

pub mod asset;
pub mod check;
pub mod corpus;
pub mod evals;
pub mod frontmatter;
pub mod id;
pub mod index;
pub mod install;
pub mod project;
pub mod renumber;
pub mod root;
pub mod skills;
pub mod transcripts;
pub mod vendor;
