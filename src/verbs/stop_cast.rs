//! Native verb: pick an active screencast session via fuzzel and stop it.
//!
//! Bails before spawning fuzzel when no active casts are found — a runtime
//! data condition, not a capability miss (exit 1, NOT 69).

use crate::snapshot::Snapshot;
use crate::{menu, niri};

pub fn run(_snapshot: &Snapshot, _args: &crate::registry::VerbArgs) -> anyhow::Result<()> {
    let choices = niri::cast_choices()?;
    if choices.is_empty() {
        anyhow::bail!("no active casts");
    }
    let labels: Vec<String> = choices.iter().map(|c| c.label.clone()).collect();
    let Some(picked) = menu::pick_one("Stop cast: ", &labels)? else {
        return Ok(()); // cancelled — exit 0, no dispatch
    };
    let session_id = menu::resolve_by_label(&choices, &picked, |c| c.label.as_str())?.session_id;
    niri::stop_cast(session_id)
}
