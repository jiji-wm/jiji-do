//! Passthrough verb: delegate to `jiji-activities switch` (which runs its own
//! picker). Gated on FORK + NIRI_ACTIVITIES by the registry, so by the time
//! this runs those deps are guaranteed present.

use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot) -> anyhow::Result<()> {
    // No snapshot needed: `jiji-activities switch` resolves its own target via
    // its own picker. We just hand off.
    crate::proc::run_capture("jiji-activities", &["switch"])?;
    Ok(())
}
