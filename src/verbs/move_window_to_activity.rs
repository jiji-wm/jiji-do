//! Passthrough verb: move the launch-time focused window to a picker-chosen
//! (or directly-named) activity via `jiji-activities move-window [<name>]
//! --window=<id>`. Bails before the subprocess fires when no window is focused
//! at launch, so we never let `jiji-activities` re-read focused state from a
//! compositor whose focus the menu's fuzzel has already stolen.
//!
//! Argv ordering: the name positional precedes the `--window` flag, matching
//! the `jiji-activities move-window` subcommand signature.
use crate::registry::VerbArgs;
use crate::snapshot::Snapshot;

pub fn run(snapshot: &Snapshot, args: &VerbArgs) -> anyhow::Result<()> {
    // Bail before any branching: a missing focused window is always an error
    // regardless of whether a name was supplied.
    let window = snapshot
        .focused_window
        .ok_or_else(|| anyhow::anyhow!("no focused window at launch"))?;
    let flag = format!("--window={window}");
    // Normalize: empty or whitespace-only positional is treated the same as
    // absent — routes to the jiji-activities picker rather than dispatching
    // `jiji-activities move-window "" --window=<id>`.
    let supplied = args
        .first
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    match supplied {
        Some(name) => {
            crate::proc::run_capture("jiji-activities", &["move-window", name, flag.as_str()])?
        }
        None => crate::proc::run_capture("jiji-activities", &["move-window", flag.as_str()])?,
    };
    Ok(())
}
