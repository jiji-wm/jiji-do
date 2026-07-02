//! Confirm-gated verb: remove a bookmark. Pick a bookmark via fuzzel, confirm
//! via `menu::confirm` naming the picked label, then dispatch on exact "Yes".
//! Cancel at either stage, or any answer other than exact "Yes" at the
//! confirm, is a clean no-op (exit 0). Empty inventory bails before fuzzel
//! opens (exit 1, NOT 69).

use crate::snapshot::Snapshot;
use crate::{menu, niri};

pub fn run(_snapshot: &Snapshot, _args: &crate::registry::VerbArgs) -> anyhow::Result<()> {
    let choices = niri::bookmark_choices()?;
    if choices.is_empty() {
        anyhow::bail!("no bookmarks");
    }
    let labels: Vec<String> = choices.iter().map(|c| c.label.clone()).collect();
    let Some(picked) = menu::pick_one("Remove bookmark: ", &labels)? else {
        return Ok(()); // cancelled — exit 0, no dispatch
    };
    let choice = menu::resolve_by_label(&choices, &picked, |c| c.label.as_str())?;
    if menu::confirm(&format!("Remove bookmark \"{}\"?", choice.label))? {
        niri::remove_bookmark(choice.id)
    } else {
        Ok(())
    }
}
