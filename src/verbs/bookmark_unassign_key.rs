//! Native verb: unassign a bookmark's key. Filters the bookmark inventory to
//! key-bearing entries before offering the picker; a filtered set with no
//! keyed bookmarks bails before fuzzel opens (exit 1, NOT 69). Cancel is a
//! clean no-op (exit 0).

use crate::snapshot::Snapshot;
use crate::{menu, niri};

pub fn run(_snapshot: &Snapshot, _args: &crate::registry::VerbArgs) -> anyhow::Result<()> {
    let choices: Vec<_> = niri::bookmark_choices()?
        .into_iter()
        .filter(|c| c.key.is_some())
        .collect();
    if choices.is_empty() {
        anyhow::bail!("no bookmarks with an assigned key");
    }
    let labels: Vec<String> = choices.iter().map(|c| c.label.clone()).collect();
    let Some(picked) = menu::pick_one("Unassign bookmark key: ", &labels)? else {
        return Ok(()); // cancelled — exit 0, no dispatch
    };
    let id = menu::resolve_by_label(&choices, &picked, |c| c.label.as_str())?.id;
    niri::unassign_bookmark_key(id)
}
