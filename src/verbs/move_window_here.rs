//! Passthrough verb: move the launch-time focused window to the current
//! workspace here via `jiji-activities move-window-here --window=<id>`. Bails
//! before the subprocess fires when no window is focused at launch, so we
//! never let `jiji-activities` re-read focused state from a compositor whose
//! focus the menu's fuzzel has already stolen.
use crate::snapshot::Snapshot;

pub fn run(snapshot: &Snapshot) -> anyhow::Result<()> {
    let window = snapshot
        .focused_window
        .ok_or_else(|| anyhow::anyhow!("no focused window at launch"))?;
    let arg = format!("--window={window}");
    crate::proc::run_capture("jiji-activities", &["move-window-here", &arg])?;
    Ok(())
}
