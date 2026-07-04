//! Verb: assign a key to a bookmark. Stage 1 picks the bookmark; stage 2
//! opens the compositor's interactive press-the-key capture prompt for the
//! picked target. Validation, collision handling, and cancel all resolve
//! in the compositor overlay — this process's exit 0 means the capture was
//! requested and acknowledged, not that a key was assigned. Cancel at
//! stage 1 is a clean no-op (exit 0). Empty inventory bails before fuzzel
//! opens (exit 1, NOT 69).

use crate::snapshot::Snapshot;
use crate::{menu, niri};

pub fn run(_snapshot: &Snapshot, _args: &crate::registry::VerbArgs) -> anyhow::Result<()> {
    let choices = niri::bookmark_choices()?;
    if choices.is_empty() {
        anyhow::bail!("no bookmarks");
    }
    let labels: Vec<String> = choices.iter().map(|c| c.label.clone()).collect();
    let Some(picked) = menu::pick_one("Assign key to bookmark: ", &labels)? else {
        return Ok(()); // cancelled — exit 0, no dispatch
    };
    let choice = menu::resolve_by_label(&choices, &picked, |c| c.label.as_str())?;

    niri::capture_bookmark_key(choice.id)
}
