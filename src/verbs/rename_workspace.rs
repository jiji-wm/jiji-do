//! Free-text verb: rename the focused workspace. Prompts via fuzzel free-text
//! mode; empty input or cancel → clean exit 0, no action dispatched.

use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot, _arg: Option<&str>) -> anyhow::Result<()> {
    match crate::menu::prompt_name("Workspace name: ")? {
        Some(name) => crate::niri::set_workspace_name(&name),
        None => Ok(()), // cancel or empty Enter — clean no-op, exit 0
    }
}
