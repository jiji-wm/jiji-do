//! Confirm-gated verb: quit the compositor. A fuzzel confirm dialog asks the
//! user before dispatching `niri msg action quit --skip-confirmation`. Cancel
//! or "No" → clean exit 0, no action dispatched. The fuzzel confirm replaces
//! the compositor's own confirmation dialog, so `--skip-confirmation` is always
//! passed.

use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot, _args: &crate::registry::VerbArgs) -> anyhow::Result<()> {
    if crate::menu::confirm("Quit jiji?")? {
        crate::niri::quit_skip_confirmation()
    } else {
        Ok(())
    }
}
