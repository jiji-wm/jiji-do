//! Native verb: pick a bookmark via fuzzel and jump to it.
//!
//! Reads the bookmark inventory from `niri msg --json bookmarks` at dispatch
//! time (not from `Snapshot` — same rationale as `activity_names_mru`: the
//! inventory can change too often across a session for a launch-time
//! snapshot to stay accurate). An empty inventory bails before fuzzel opens
//! (exit 1, NOT 69). Cancel is a clean no-op (exit 0).

use crate::snapshot::Snapshot;
use crate::{menu, niri};

pub fn run(_snapshot: &Snapshot, _args: &crate::registry::VerbArgs) -> anyhow::Result<()> {
    let choices = niri::bookmark_choices()?;
    if choices.is_empty() {
        anyhow::bail!("no bookmarks");
    }
    let labels: Vec<String> = choices.iter().map(|c| c.label.clone()).collect();
    let Some(picked) = menu::pick_one("Jump to bookmark: ", &labels)? else {
        return Ok(()); // cancelled — exit 0, no dispatch
    };
    let id = menu::resolve_by_label(&choices, &picked, |c| c.label.as_str())?.id;
    niri::jump_to_bookmark(id)
}
