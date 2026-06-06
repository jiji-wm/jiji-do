//! Native verb: jump to any workspace across activities — full picker,
//! activity-filtered picker, or direct dispatch, depending on the args.
//!
//! One picker row per (activity, workspace) membership — the activity
//! prefix decides the landing activity for shared/sticky workspaces.
//! Bails before spawning fuzzel when the (possibly filtered) inventory is
//! empty (runtime data condition, not a capability miss: exit 1, NOT 69) —
//! an unknown activity name lands in the same arm, since it matches no rows.

use crate::registry::VerbArgs;
use crate::snapshot::Snapshot;
use crate::{menu, niri};

pub fn run(_snapshot: &Snapshot, args: &VerbArgs) -> anyhow::Result<()> {
    // Normalize both slots: empty/whitespace-only behaves as absent.
    let activity = args
        .first
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let workspace = args
        .second
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    // Both supplied: direct user-typed passthrough, no picker.
    if let (Some(act), Some(ws)) = (activity, workspace) {
        return niri::focus_workspace_in_activity(act, ws);
    }

    let mut rows = niri::all_workspace_rows()?;
    let prompt = match activity {
        // Activity supplied: the picker shows only that activity's rows.
        Some(act) => {
            rows.retain(|r| r.activity_name == act);
            if rows.is_empty() {
                anyhow::bail!("no workspaces found in activity '{act}'");
            }
            format!("Switch to workspace ({act}): ")
        }
        None => {
            if rows.is_empty() {
                anyhow::bail!("no workspaces found");
            }
            "Switch to workspace (all activities): ".to_string()
        }
    };
    let labels: Vec<String> = rows.iter().map(|r| r.label.clone()).collect();
    let Some(picked) = menu::pick_one(&prompt, &labels)? else {
        return Ok(()); // cancelled — exit 0, no dispatch
    };
    let row = menu::resolve_by_label(&rows, &picked, |r| r.label.as_str())?;
    niri::focus_workspace_in_activity(&row.activity_name, &format!("id:{}", row.ws_id))
}
