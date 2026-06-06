//! Native verb: power on all monitors via `niri msg action power-on-monitors`.
//! Immediate-dispatch: no picker, requires only NIRI_SOCKET.

use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot, _args: &crate::registry::VerbArgs) -> anyhow::Result<()> {
    crate::niri::run_action("power-on-monitors")
}
