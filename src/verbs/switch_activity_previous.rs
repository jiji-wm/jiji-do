//! Passthrough verb: toggle to the previously-active activity via
//! `jiji-activities switch-previous`. Pure toggle — no picker, no snapshot.
use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot, _arg: Option<&str>) -> anyhow::Result<()> {
    crate::proc::run_capture("jiji-activities", &["switch-previous"])?;
    Ok(())
}
