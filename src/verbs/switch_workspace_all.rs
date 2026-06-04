//! Native verb: pick any workspace across all activities and jump to it.
//!
//! One picker row per (activity, workspace) membership — the activity
//! prefix decides the landing activity for shared/sticky workspaces.
//! Bails before spawning fuzzel when the inventory is empty (runtime data
//! condition, not a capability miss: exit 1, NOT 69).

use crate::snapshot::Snapshot;
use crate::{menu, niri};

pub fn run(_snapshot: &Snapshot, _arg: Option<&str>) -> anyhow::Result<()> {
    let rows = niri::all_workspace_rows()?;
    if rows.is_empty() {
        anyhow::bail!("no workspaces found");
    }
    let labels: Vec<String> = rows.iter().map(|r| r.label.clone()).collect();
    let Some(picked) = menu::pick_one("Switch to workspace (all activities): ", &labels)? else {
        return Ok(()); // cancelled — exit 0, no dispatch
    };
    let row = menu::resolve_by_label(&rows, &picked, |r| r.label.as_str())?;
    niri::focus_workspace_in_activity(&row.activity_name, row.ws_id)
}
