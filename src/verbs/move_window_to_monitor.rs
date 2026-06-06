//! Native verb: pick a monitor via fuzzel and move the focused window to it.
//!
//! No `--id` flag is passed — the compositor defaults to the focused window.
//! The prompt names the focused window (snapshot title, truncated) so the
//! user sees which window is about to move; display context only.
//! Bails before spawning fuzzel when no outputs are available (exit 1, NOT 69).

use crate::snapshot::Snapshot;
use crate::{menu, niri};

pub fn run(snapshot: &Snapshot, _args: &crate::registry::VerbArgs) -> anyhow::Result<()> {
    let choices = niri::output_choices()?;
    if choices.is_empty() {
        anyhow::bail!("no outputs available");
    }
    let labels: Vec<String> = choices.iter().map(|c| c.label.clone()).collect();
    let prompt = match snapshot.focused_window_title.as_deref() {
        Some(title) => {
            // Truncate on a char boundary — titles can be arbitrarily long
            // and the prompt shares the row with the typed filter.
            let short: String = title.chars().take(40).collect();
            let ellipsis = if title.chars().count() > 40 {
                "…"
            } else {
                ""
            };
            format!("Move \"{short}{ellipsis}\" to monitor: ")
        }
        None => "Move window to monitor: ".to_string(),
    };
    let Some(picked) = menu::pick_one(&prompt, &labels)? else {
        return Ok(()); // cancelled — exit 0, no dispatch
    };
    let connector = menu::resolve_by_label(&choices, &picked, |c| c.label.as_str())?
        .connector
        .as_str();
    niri::move_window_to_monitor(connector)
}
