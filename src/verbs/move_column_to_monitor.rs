//! Native verb: pick a monitor via fuzzel and move the focused column to it.
//!
//! Bails before spawning fuzzel when no outputs are available (exit 1, NOT 69).

use crate::snapshot::Snapshot;
use crate::{menu, niri};

pub fn run(_snapshot: &Snapshot, _arg: Option<&str>) -> anyhow::Result<()> {
    let choices = niri::output_choices()?;
    if choices.is_empty() {
        anyhow::bail!("no outputs available");
    }
    let labels: Vec<String> = choices.iter().map(|c| c.label.clone()).collect();
    let Some(picked) = menu::pick_one("monitor", &labels)? else {
        return Ok(()); // cancelled — exit 0, no dispatch
    };
    let connector = menu::resolve_by_label(&choices, &picked, |c| c.label.as_str())?
        .connector
        .as_str();
    niri::move_column_to_monitor(connector)
}
