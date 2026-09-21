//! Tells cargo which trees the binary embeds.
//!
//! `include_dir!` compiles `configs/` and `docs/tools/` into the binary, but
//! cargo cannot see through the macro: without these lines, editing a config
//! or adding a docs page changes nothing cargo considers an input, so the
//! stale copy from the last build keeps shipping. That failure is silent on a
//! dev machine — the source tree is read directly there — and only shows up
//! on a machine running the downloaded binary, which is the worst place to
//! find it.
fn main() {
    println!("cargo:rerun-if-changed=configs");
    println!("cargo:rerun-if-changed=docs/tools");
    println!("cargo:rerun-if-changed=.agents");
    println!("cargo:rerun-if-changed=manifest.toml");
}
