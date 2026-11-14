// mobfarm's build script: only declares the binary-provenance stamp as an input (tick-exact Nov 14 02:41
// (b)), so a build with a new `MCTAS_SOURCE_TREE` re-embeds it (`mobfarm::provenance::BUILT_FROM`, read
// through option_env!, which rustc also records as an env dependency). A stale stamp is still caught at
// run time: `provenance::check` refuses when the stamp isn't the worktree's current code tree.
fn main() {
    println!("cargo:rerun-if-env-changed=MCTAS_SOURCE_TREE");
}
