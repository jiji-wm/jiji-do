//! Free-text verb: rename the focused workspace. Prompts via fuzzel free-text
//! mode; empty input or cancel → clean exit 0, no action dispatched.
//!
//! The prompt identifies the rename target from the launch snapshot — current
//! name (when set), per-output index, and stable id — so the user sees which
//! workspace they are about to rename. Display context only; the dispatch
//! still targets the focused workspace (no `--reference` equivalent passed).

use crate::snapshot::Snapshot;
use std::fmt::Write;

/// Build the prompt from whatever focused-workspace context the snapshot has.
/// All-absent degrades to the bare form. Pure (unit-tested).
fn prompt_for(snapshot: &Snapshot) -> String {
    let mut p = String::from("Rename workspace");
    if let Some(idx) = snapshot.focused_workspace_idx {
        let _ = write!(p, " {idx}");
    }
    if let Some(name) = snapshot.focused_workspace_name.as_deref() {
        let _ = write!(p, " \"{name}\"");
    }
    if let Some(id) = snapshot.focused_workspace {
        let _ = write!(p, " (id {id})");
    }
    p.push_str(" to: ");
    p
}

pub fn run(snapshot: &Snapshot, _args: &crate::registry::VerbArgs) -> anyhow::Result<()> {
    match crate::menu::prompt_name(&prompt_for(snapshot))? {
        Some(name) => crate::niri::set_workspace_name(&name),
        None => Ok(()), // cancel or empty Enter — clean no-op, exit 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(idx: Option<u64>, name: Option<&str>, id: Option<u64>) -> Snapshot {
        Snapshot {
            focused_window: None,
            focused_window_title: None,
            focused_workspace: id,
            focused_workspace_idx: idx,
            focused_workspace_name: name.map(str::to_string),
            focused_output: None,
            focused_activity: None,
        }
    }

    #[test]
    fn prompt_with_full_context() {
        assert_eq!(
            prompt_for(&snap(Some(3), Some("web"), Some(21))),
            "Rename workspace 3 \"web\" (id 21) to: "
        );
    }

    #[test]
    fn prompt_unnamed_workspace_omits_name() {
        assert_eq!(
            prompt_for(&snap(Some(2), None, Some(7))),
            "Rename workspace 2 (id 7) to: "
        );
    }

    #[test]
    fn prompt_no_context_is_bare() {
        assert_eq!(prompt_for(&snap(None, None, None)), "Rename workspace to: ");
    }
}
