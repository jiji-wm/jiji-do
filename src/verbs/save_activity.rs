//! Passthrough verb: save the launch-time focused activity's layout under
//! its current name via `jiji-activities save <name>`. Bails before the
//! subprocess fires when no activity is focused at launch (None means
//! upstream niri (no FORK; capability-gated away from this verb) or — on
//! the fork — no `is_active` activity in the snapshot, both of which make
//! "save the focused activity" meaningless).
use crate::snapshot::Snapshot;

pub fn run(snapshot: &Snapshot) -> anyhow::Result<()> {
    let name = snapshot
        .focused_activity
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("no focused activity at launch"))?;
    crate::proc::run_capture("jiji-activities", &["save", name])?;
    Ok(())
}
