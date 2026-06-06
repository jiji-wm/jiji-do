//! Native verb: pick a monitor via fuzzel and focus it.
//!
//! Bails before spawning fuzzel when no outputs are available — a runtime data
//! condition, not a capability miss (exit 1, NOT 69).

use crate::snapshot::Snapshot;
use crate::{menu, niri};

pub fn run(_snapshot: &Snapshot, _args: &crate::registry::VerbArgs) -> anyhow::Result<()> {
    let choices = niri::output_choices()?;
    if choices.is_empty() {
        anyhow::bail!("no outputs available");
    }
    let labels: Vec<String> = choices.iter().map(|c| c.label.clone()).collect();
    let Some(picked) = menu::pick_one("Focus monitor: ", &labels)? else {
        return Ok(()); // cancelled — exit 0, no dispatch
    };
    let connector = menu::resolve_by_label(&choices, &picked, |c| c.label.as_str())?
        .connector
        .as_str();
    niri::focus_monitor(connector)
}
