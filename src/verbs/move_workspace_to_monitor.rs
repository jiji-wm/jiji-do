//! Native verb: pick a monitor via fuzzel and move the focused workspace to it.
//!
//! No `--reference` flag is passed — the compositor defaults to the focused
//! workspace. The prompt identifies the focused workspace (snapshot idx/name)
//! so the user sees which workspace is about to move; display context only.
//! Bails before spawning fuzzel when no outputs are available
//! (exit 1, NOT 69).

use crate::snapshot::Snapshot;
use crate::{menu, niri};
use std::fmt::Write;

pub fn run(snapshot: &Snapshot, _args: &crate::registry::VerbArgs) -> anyhow::Result<()> {
    let choices = niri::output_choices()?;
    if choices.is_empty() {
        anyhow::bail!("no outputs available");
    }
    let labels: Vec<String> = choices.iter().map(|c| c.label.clone()).collect();
    let mut prompt = String::from("Move workspace");
    if let Some(idx) = snapshot.focused_workspace_idx {
        let _ = write!(prompt, " {idx}");
    }
    if let Some(name) = snapshot.focused_workspace_name.as_deref() {
        let _ = write!(prompt, " \"{name}\"");
    }
    prompt.push_str(" to monitor: ");
    let Some(picked) = menu::pick_one(&prompt, &labels)? else {
        return Ok(()); // cancelled — exit 0, no dispatch
    };
    let connector = menu::resolve_by_label(&choices, &picked, |c| c.label.as_str())?
        .connector
        .as_str();
    niri::move_workspace_to_monitor(connector)
}
