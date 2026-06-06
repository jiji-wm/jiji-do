//! Freeform-name verb: create a new activity. The direct-CLI path skips the
//! prompt when a name is supplied as a positional arg; the menu path prompts
//! via `fuzzel --dmenu` with an empty stdin (free-text mode). Snapshot is
//! unused — the verb does not act on the focused context.

use crate::registry::VerbArgs;
use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot, args: &VerbArgs) -> anyhow::Result<()> {
    // Normalize: empty or whitespace-only positional is treated the same as
    // absent — routes to the prompt rather than dispatching `jiji-activities
    // create ""`.
    let supplied = args
        .first
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    match supplied {
        Some(name) => {
            crate::proc::run_capture("jiji-activities", &["create", name])?;
            Ok(())
        }
        None => match crate::menu::prompt_name("Activity name: ")? {
            Some(typed) => {
                crate::proc::run_capture("jiji-activities", &["create", &typed])?;
                Ok(())
            }
            None => Ok(()), // cancel or empty Enter — clean no-op, exit 0
        },
    }
}
