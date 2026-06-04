//! Passthrough verb: remove an existing activity. The direct-CLI form skips
//! the picker when a name is supplied as a positional arg; the menu path reads
//! the activity inventory from `niri msg --json activities` at dispatch time
//! and picks from it via fuzzel (MRU-ordered, same as the rename picker). An
//! empty inventory bails before fuzzel opens.
//! Snapshot is unused — the verb does not act on the focused context.

use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot, arg: Option<&str>) -> anyhow::Result<()> {
    // Normalize: empty or whitespace-only positional is treated the same as
    // absent — routes to the picker rather than dispatching `jiji-activities
    // remove ""`.
    let supplied = arg.map(str::trim).filter(|s| !s.is_empty());
    match supplied {
        Some(name) => {
            crate::proc::run_capture("jiji-activities", &["remove", name])?;
            Ok(())
        }
        None => {
            let names = crate::niri::activity_names_mru()?;
            if names.is_empty() {
                anyhow::bail!("no activities to remove");
            }
            match crate::menu::pick_one("Remove activity: ", &names)? {
                Some(picked) => {
                    crate::proc::run_capture("jiji-activities", &["remove", &picked])?;
                    Ok(())
                }
                None => Ok(()), // fuzzel cancel — clean no-op, exit 0
            }
        }
    }
}
