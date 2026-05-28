//! Native verb: focus the previous workspace via `niri msg action focus-workspace-previous`.
//! Immediate-dispatch: no picker, so requires only NIRI_SOCKET.

use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot) -> anyhow::Result<()> {
    crate::niri::run_action("focus-workspace-previous")
}
