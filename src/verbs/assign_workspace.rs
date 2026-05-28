//! Passthrough verb: assign the launch-time focused workspace to activities
//! via `jiji-activities assign-workspace --workspace=<id>`. Bails before the
//! subprocess fires when no workspace is focused at launch.
use crate::snapshot::Snapshot;

pub fn run(snapshot: &Snapshot) -> anyhow::Result<()> {
    let workspace = snapshot
        .focused_workspace
        .ok_or_else(|| anyhow::anyhow!("no focused workspace at launch"))?;
    let arg = format!("--workspace={workspace}");
    crate::proc::run_capture("jiji-activities", &["assign-workspace", &arg])?;
    Ok(())
}
