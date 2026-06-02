//! Passthrough verb: move the launch-time focused workspace to a picker-chosen
//! (or directly-named) activity via `jiji-activities move-workspace [<name>]
//! --workspace=<id>`. Bails before the subprocess fires when no workspace is
//! focused at launch.
//!
//! Argv ordering: the name positional precedes the `--workspace` flag, matching
//! the `jiji-activities move-workspace` subcommand signature.
use crate::snapshot::Snapshot;

pub fn run(snapshot: &Snapshot, arg: Option<&str>) -> anyhow::Result<()> {
    // Bail before any branching: a missing focused workspace is always an error
    // regardless of whether a name was supplied.
    let workspace = snapshot
        .focused_workspace
        .ok_or_else(|| anyhow::anyhow!("no focused workspace at launch"))?;
    let flag = format!("--workspace={workspace}");
    // Normalize: empty or whitespace-only positional is treated the same as
    // absent — routes to the jiji-activities picker rather than dispatching
    // `jiji-activities move-workspace "" --workspace=<id>`.
    let supplied = arg.map(str::trim).filter(|s| !s.is_empty());
    match supplied {
        Some(name) => {
            crate::proc::run_capture("jiji-activities", &["move-workspace", name, flag.as_str()])?
        }
        None => crate::proc::run_capture("jiji-activities", &["move-workspace", flag.as_str()])?,
    };
    Ok(())
}
