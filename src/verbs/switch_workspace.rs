//! Native verb: pick a workspace via fuzzel and focus it.

use crate::snapshot::Snapshot;
use crate::{menu, niri};

pub fn run(_snapshot: &Snapshot, _arg: Option<&str>) -> anyhow::Result<()> {
    let choices = niri::workspace_choices()?;
    let labels: Vec<String> = choices.iter().map(|c| c.label.clone()).collect();
    let Some(picked) = menu::pick_one("Switch to workspace: ", &labels)? else {
        return Ok(()); // cancelled — exit 0, no dispatch
    };
    let id = menu::resolve_by_label(&choices, &picked, |c| c.label.as_str())?.id;
    niri::focus_workspace(id)
}
