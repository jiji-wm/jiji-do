//! clap surface. Verbs are dispatched by name against the registry; `--debug`
//! reports capability filtering.

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "jiji-do", version, about = "Helix-style launcher for jiji")]
pub struct Cli {
    /// Print resolved capabilities and per-verb kept/filtered status, then exit.
    #[arg(long, global = true)]
    pub debug: bool,

    /// Verb to dispatch directly. Omit to open the fuzzel menu.
    pub verb: Option<String>,
}
