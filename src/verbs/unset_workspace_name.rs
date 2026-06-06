//! Native verb: unset the focused workspace name via `niri msg action unset-workspace-name`.
//! No reference arg is passed — the action defaults to the focused workspace.
//! Immediate-dispatch: no picker, requires only NIRI_SOCKET.

use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot, _args: &crate::registry::VerbArgs) -> anyhow::Result<()> {
    crate::niri::run_action("unset-workspace-name")
}
