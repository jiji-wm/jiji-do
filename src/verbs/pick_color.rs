//! Native verb: pick a color via the compositor's built-in color picker.
//!
//! `niri msg pick-color` returns a human-readable color string. The clipboard
//! (`wl-copy`) is the retrievable sink: the value lands there so the user can
//! paste it. A notification (`notify-send`) is emitted as an ephemeral
//! best-effort signal regardless of the clipboard outcome. If the clipboard
//! write fails, stdout receives the color as a fallback so the value is never
//! lost. The pick itself failing (user cancels, compositor unavailable)
//! propagates as an error — only the routing is best-effort.

use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot, _arg: Option<&str>) -> anyhow::Result<()> {
    let color = crate::niri::pick_color()?;
    let copied = crate::proc::run_best_effort("wl-copy", &[], Some(color.trim()));
    crate::proc::run_best_effort("notify-send", &["Picked color", color.trim()], None);
    if !copied {
        print!("{color}");
    }
    Ok(())
}
