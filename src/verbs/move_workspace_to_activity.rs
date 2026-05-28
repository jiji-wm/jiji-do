//! Passthrough verb: move the launch-time focused workspace to a picker-chosen
//! activity via `jiji-activities move-workspace --workspace=<id>`. Bails
//! before the subprocess fires when no workspace is focused at launch.
use crate::snapshot::Snapshot;

pub fn run(snapshot: &Snapshot, _arg: Option<&str>) -> anyhow::Result<()> {
    let workspace = snapshot
        .focused_workspace
        .ok_or_else(|| anyhow::anyhow!("no focused workspace at launch"))?;
    let arg = format!("--workspace={workspace}");
    crate::proc::run_capture("jiji-activities", &["move-workspace", &arg])?;
    Ok(())
}
