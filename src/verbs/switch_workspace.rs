//! Native verb: focus a workspace — via the fuzzel picker, or directly when
//! a reference is supplied on the CLI.

use crate::registry::VerbArgs;
use crate::snapshot::Snapshot;
use crate::{menu, niri};

pub fn run(_snapshot: &Snapshot, args: &VerbArgs) -> anyhow::Result<()> {
    // Normalize: empty/whitespace-only positional routes to the picker.
    if let Some(reference) = args
        .first
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        // User-typed passthrough: name, index, or id:N — the compositor
        // resolves it and errors loudly on a miss.
        return niri::focus_workspace_typed(reference);
    }
    let choices = niri::workspace_choices()?;
    let labels: Vec<String> = choices.iter().map(|c| c.label.clone()).collect();
    let Some(picked) = menu::pick_one("Switch to workspace: ", &labels)? else {
        return Ok(()); // cancelled — exit 0, no dispatch
    };
    let choice = menu::resolve_by_label(&choices, &picked, |c| c.label.as_str())?;
    niri::focus_workspace(&choice.focus_reference())
}
