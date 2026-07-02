//! Two-stage verb: move a bookmark to a new position. Stage 1 picks the
//! bookmark to move; stage 2 offers one row per list position (reusing the
//! same position-prefixed labels) and the picked row's 1-based display
//! position `P` dispatches `move-bookmark --id <id> --pos <P-1>` (0-based,
//! compositor-clamped). Picking the bookmark's own row is a compositor-side
//! silent no-op — acceptable. Cancel at either stage is a clean no-op
//! (exit 0). Empty inventory bails before fuzzel opens (exit 1, NOT 69).

use crate::snapshot::Snapshot;
use crate::{menu, niri};

pub fn run(_snapshot: &Snapshot, _args: &crate::registry::VerbArgs) -> anyhow::Result<()> {
    let choices = niri::bookmark_choices()?;
    if choices.is_empty() {
        anyhow::bail!("no bookmarks");
    }
    let labels: Vec<String> = choices.iter().map(|c| c.label.clone()).collect();

    let Some(picked) = menu::pick_one("Move bookmark: ", &labels)? else {
        return Ok(()); // cancelled — exit 0, no dispatch
    };
    let source_id = menu::resolve_by_label(&choices, &picked, |c| c.label.as_str())?.id;

    let prompt = format!("Move \"{picked}\" to position: ");
    let Some(picked_pos) = menu::pick_one(&prompt, &labels)? else {
        return Ok(()); // cancelled at stage 2 — exit 0, no dispatch
    };
    let target_id = menu::resolve_by_label(&choices, &picked_pos, |c| c.label.as_str())?.id;
    let pos = choices
        .iter()
        .position(|c| c.id == target_id)
        .expect("target_id resolved from choices via resolve_by_label") as u32;
    niri::move_bookmark(source_id, pos)
}
