//! Passthrough verb: save an activity's layout via `jiji-activities save
//! <name>`. When a name is supplied it is used directly (save-as), with no
//! requirement for a focused activity at launch. When no name is supplied the
//! focused activity's name is derived from the snapshot (and the verb bails if
//! none is focused — None means upstream niri (no FORK; capability-gated away)
//! or no `is_active` activity in the snapshot, both of which make "save the
//! focused activity" meaningless).
use crate::snapshot::Snapshot;

pub fn run(snapshot: &Snapshot, arg: Option<&str>) -> anyhow::Result<()> {
    // Normalize: empty or whitespace-only positional is treated the same as
    // absent — routes to the derive-from-focused path rather than dispatching
    // `jiji-activities save ""`, which would bypass the focused-activity derive.
    let supplied = arg.map(str::trim).filter(|s| !s.is_empty());
    match supplied {
        Some(name) => {
            // Direct save-as path: supplied name is authoritative; no focused-
            // activity requirement.
            crate::proc::run_capture("jiji-activities", &["save", name])?;
        }
        None => {
            // Derive-from-focused path: bail if no activity is focused.
            let name = snapshot
                .focused_activity
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("no focused activity at launch"))?;
            crate::proc::run_capture("jiji-activities", &["save", name])?;
        }
    }
    Ok(())
}
