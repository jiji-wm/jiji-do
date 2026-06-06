//! Native verb: pick a window via the compositor's built-in picker.
//!
//! `niri msg pick-window` returns a human-readable summary of the picked
//! window. The summary is always printed to stdout (the retrievable home —
//! never gated on any other sink), copied to the clipboard via `wl-copy` so
//! it is pasteable even when launched from the menu, and announced via
//! `notify-send` as an ephemeral cue. Clipboard and notification are
//! best-effort soft deps; their failure must not fail the verb. The pick
//! itself failing (user cancels, niri unavailable) propagates as an error —
//! only the routing is best-effort.

use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot, _args: &crate::registry::VerbArgs) -> anyhow::Result<()> {
    let info = crate::niri::pick_window()?;
    print!("{info}");
    crate::proc::run_best_effort("wl-copy", &[], Some(info.trim()));
    crate::proc::run_best_effort("notify-send", &["Picked window", info.trim()], None);
    Ok(())
}
