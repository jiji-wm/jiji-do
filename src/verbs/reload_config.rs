//! Native verb: reload the compositor config via `niri msg action load-config-file`.
//! Immediate-dispatch: no picker, requires only NIRI_SOCKET.

use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot, _arg: Option<&str>) -> anyhow::Result<()> {
    crate::niri::reload_config()
}
