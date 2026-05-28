//! Passthrough verb: remove an existing activity. The direct-CLI form skips
//! the picker when a name is supplied as a positional arg; the menu path reads
//! the activity inventory from `niri msg --json activities` at dispatch time
//! and picks from it via fuzzel. An empty inventory bails before fuzzel opens.
//! Snapshot is unused — the verb does not act on the focused context.

use crate::snapshot::Snapshot;

/// Minimal projection of `niri msg --json activities` — only the name field is
/// needed; `is_active` is intentionally ignored (the picker shows all activities,
/// including the currently active one).
#[derive(serde::Deserialize)]
struct NameBrief {
    name: String,
}

pub fn run(_snapshot: &Snapshot, arg: Option<&str>) -> anyhow::Result<()> {
    // Normalize: empty or whitespace-only positional is treated the same as
    // absent — routes to the picker rather than dispatching `jiji-activities
    // remove ""`.
    let supplied = arg.map(str::trim).filter(|s| !s.is_empty());
    match supplied {
        Some(name) => {
            crate::proc::run_capture("jiji-activities", &["remove", name])?;
            Ok(())
        }
        None => {
            // Read the inventory at dispatch time (not from Snapshot — activities
            // state can change between launch and menu selection).
            let json = crate::proc::run_capture("niri", &["msg", "--json", "activities"])?;
            let briefs: Vec<NameBrief> = serde_json::from_str(&json)
                .map_err(|e| anyhow::anyhow!("parsing activities JSON: {e}"))?;
            if briefs.is_empty() {
                anyhow::bail!("no activities to remove");
            }
            let names: Vec<String> = briefs.into_iter().map(|b| b.name).collect();
            match crate::menu::pick_one("activity", &names)? {
                Some(picked) => {
                    crate::proc::run_capture("jiji-activities", &["remove", &picked])?;
                    Ok(())
                }
                None => Ok(()), // fuzzel cancel — clean no-op, exit 0
            }
        }
    }
}
