//! Native verb: pick a color via the compositor's built-in color picker.
//!
//! `niri msg pick-color` returns a human-readable color string. The color is
//! always printed to stdout (the retrievable home — never gated on any other
//! sink), copied to the clipboard via `wl-copy` so the user can paste it, and
//! announced via `notify-send` as an ephemeral cue. Clipboard and
//! notification are best-effort soft deps; their failure must not fail the
//! verb. The pick itself failing (user cancels, compositor unavailable)
//! propagates as an error — only the routing is best-effort.

use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot, _arg: Option<&str>) -> anyhow::Result<()> {
    let color = crate::niri::pick_color()?;
    print!("{color}");
    crate::proc::run_best_effort("wl-copy", &[], Some(color.trim()));
    crate::proc::run_best_effort("notify-send", &["Picked color", color.trim()], None);
    Ok(())
}
