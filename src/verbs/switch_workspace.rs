//! Native verb: pick a workspace via fuzzel and focus it.

use crate::snapshot::Snapshot;
use crate::{menu, niri};

pub fn run(_snapshot: &Snapshot) -> anyhow::Result<()> {
    let choices = niri::workspace_choices()?;
    let labels: Vec<String> = choices.iter().map(|c| c.label.clone()).collect();
    let Some(picked) = menu::pick_one("workspace", &labels)? else {
        return Ok(()); // cancelled — exit 0, no dispatch
    };
    let id = choices
        .iter()
        .find(|c| c.label == picked)
        .map(|c| c.id)
        .ok_or_else(|| anyhow::anyhow!("picker returned unknown label: {picked}"))?;
    niri::focus_workspace(id)
}
