//! Passthrough verb: rename an existing activity. Reads the activity inventory
//! from `niri msg --json activities` at dispatch time, picks the target via
//! fuzzel, then prompts for the new name via a free-text fuzzel prompt.
//! Both cancel points (`pick_one` and `prompt_name`) are clean no-ops (exit 0),
//! as is a blank Enter on the name prompt (empty stdout, exit 0).
//! An empty inventory bails before fuzzel opens (exit 1, NOT 69).
//! Snapshot is unused — the verb does not act on the focused context.

use crate::snapshot::Snapshot;

/// Minimal projection of `niri msg --json activities` — only the name field is
/// needed; `is_active` is intentionally ignored (the picker shows all activities,
/// including the currently active one).
#[derive(serde::Deserialize)]
struct NameBrief {
    name: String,
}

pub fn run(_snapshot: &Snapshot, _arg: Option<&str>) -> anyhow::Result<()> {
    // Read the inventory at dispatch time (not from Snapshot — activities
    // state can change between launch and menu selection).
    let json = crate::proc::run_capture("niri", &["msg", "--json", "activities"])?;
    let briefs: Vec<NameBrief> =
        serde_json::from_str(&json).map_err(|e| anyhow::anyhow!("parsing activities JSON: {e}"))?;
    if briefs.is_empty() {
        anyhow::bail!("no activities to rename");
    }
    let names: Vec<String> = briefs.into_iter().map(|b| b.name).collect();

    // Step 1: pick the target activity.
    let target = match crate::menu::pick_one("activity", &names)? {
        Some(picked) => picked,
        None => return Ok(()), // fuzzel cancel — clean no-op, exit 0
    };

    // Step 2: prompt for the new name.
    let new_name = match crate::menu::prompt_name("New activity name: ")? {
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
