//! Native verb: pick a window via the compositor's built-in picker.
//!
//! `niri msg pick-window` returns a human-readable summary of the picked
//! window. The result is routed to a desktop notification via `notify-send`;
//! if `notify-send` is absent or fails, the summary is printed to stdout so
//! the value is never lost. The pick itself failing (user cancels, niri
//! unavailable) propagates as an error — only the routing is best-effort.

use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot, _arg: Option<&str>) -> anyhow::Result<()> {
    let info = crate::niri::pick_window()?;
    if !crate::proc::run_best_effort("notify-send", &["Picked window", info.trim()], None) {
        print!("{info}");
    }
    Ok(())
}
