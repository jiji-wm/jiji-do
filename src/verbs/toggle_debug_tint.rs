//! Native verb: toggle the debug tint overlay via `niri msg action toggle-debug-tint`.
//! Immediate-dispatch diagnostic verb; requires only NIRI_SOCKET.

use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot, _arg: Option<&str>) -> anyhow::Result<()> {
    crate::niri::run_action("toggle-debug-tint")
}
