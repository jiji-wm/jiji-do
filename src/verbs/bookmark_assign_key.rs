//! Two-stage verb: assign a key to a bookmark. Stage 1 picks the bookmark;
//! stage 2 is a free-text fuzzel prompt naming the picked target, taking
//! syntax like "Mod+M". The compositor owns key syntax and collision
//! policy — an invalid or colliding key fails loudly via the subprocess's
//! non-zero exit; this verb does not pre-validate. Cancel at either stage,
//! or a blank Enter on the key prompt, is a clean no-op (exit 0). Empty
//! inventory bails before fuzzel opens (exit 1, NOT 69).

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

    let prompt = format!("Assign key to \"{}\" (e.g. Mod+M): ", choice.label);
    let Some(key) = menu::prompt_name(&prompt)? else {
        return Ok(()); // cancel or blank Enter — exit 0, no dispatch
    };
    niri::assign_bookmark_key(choice.id, &key)
}
