//! Direct-CLI-only data verb: print workspace names or completion-candidate
//! rows, one per line.
//!
//! Default scope is the current activity (matching the switch-workspace
//! picker scope); `--activity <name>` lists that activity's workspaces
//! instead. Named workspaces only — unnamed workspaces have no typeable
//! reference to offer (they stay reachable through index passthrough).
//! Primary consumer: the fish completion candidates commands; also useful
//! standalone for scripting.
//!
//! With `--complete`, the verb emits `token\tdescription` candidate rows
//! for the fish dynamic completion instead of bare names. Tokens follow the
//! typed-reference rules: name when set, per-monitor index on the focused
//! output, `id:N` on other outputs and always under `--activity` (the
//! compositor rejects bare indices combined with an activity qualifier).
//! Descriptions carry `idx N · id:N · <output> · <title>` so an all-unnamed
//! session can tell rows apart. The windows payload is read only in this mode.
//!
//! The activities payload is fetched only for `--activity`, so the default
//! form works on vanilla niri. The flag form requires the jiji compositor
//! (`activities` is a jiji-only request); on vanilla niri the subprocess
//! fails and the error propagates with the compositor's own message.

use crate::niri;
use crate::registry::VerbArgs;
use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot, args: &VerbArgs) -> anyhow::Result<()> {
    let activity = args
        .first
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    // `second` is the --complete presence sentinel set by the CLI mapping.
    let complete = args.second.is_some();
    let lines = match (activity, complete) {
        (Some(act), false) => niri::workspace_names_in_activity(act)?,
        (None, false) => niri::workspace_names()?,
        (Some(act), true) => niri::complete_rows_in_activity(act)?,
        (None, true) => niri::complete_rows()?,
    };
    for line in lines {
        println!("{line}");
    }
    Ok(())
}
