//! Passthrough verb: rename an existing activity. Reads the activity inventory
//! from `niri msg --json activities` at dispatch time, picks the target via
//! fuzzel (MRU-ordered — the current activity is the preselected first row),
//! then prompts for the new name via a free-text fuzzel prompt that names the
//! picked target.
//! Both cancel points (`pick_one` and `prompt_name`) are clean no-ops (exit 0),
//! as is a blank Enter on the name prompt (empty stdout, exit 0).
//! An empty inventory bails before fuzzel opens (exit 1, NOT 69).
//! Snapshot is unused — the verb does not act on the focused context.

use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot, _args: &crate::registry::VerbArgs) -> anyhow::Result<()> {
    let names = crate::niri::activity_names_mru()?;
    if names.is_empty() {
        anyhow::bail!("no activities to rename");
    }

    // Step 1: pick the target activity (MRU order — current activity first).
    let target = match crate::menu::pick_one("Rename activity: ", &names)? {
        Some(picked) => picked,
        None => return Ok(()), // fuzzel cancel — clean no-op, exit 0
    };

    // Step 2: prompt for the new name, naming the target being renamed.
    let prompt = format!("Rename activity \"{target}\" to: ");
    let new_name = match crate::menu::prompt_name(&prompt)? {
        Some(name) => name,
        None => return Ok(()), // cancel or empty Enter — clean no-op, exit 0
    };

    // Both inputs confirmed: dispatch rename. Argv order is load-bearing:
    // name positional first, then `--activity <target>` as two elements.
    crate::proc::run_capture(
        "jiji-activities",
        &["rename", &new_name, "--activity", &target],
    )?;
    Ok(())
}
