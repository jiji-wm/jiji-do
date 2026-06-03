//! Confirm-gated verb: power off all monitors. A fuzzel confirm dialog asks the
//! user before dispatching `niri msg action power-off-monitors`. Cancel or "No"
//! → clean exit 0, no action dispatched.

use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot, _arg: Option<&str>) -> anyhow::Result<()> {
    if crate::menu::confirm("Power off monitors?")? {
        crate::niri::run_action("power-off-monitors")
    } else {
        Ok(())
    }
}
