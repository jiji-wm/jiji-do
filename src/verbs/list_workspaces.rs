//! Direct-CLI-only data verb: print workspace names, one per line.
//!
//! Default scope is the current activity (matching the switch-workspace
//! picker scope); `--activity <name>` lists that activity's workspaces
//! instead. Named workspaces only — unnamed workspaces have no typeable
//! reference to offer (they stay reachable through index passthrough).
//! Primary consumer: the fish completion candidates commands; also useful
//! standalone for scripting.
//!
//! The activities payload is fetched only for `--activity`, so the default
//! form works on vanilla niri. The flag form requires the jiji compositor
//! (`activities` is a jiji-only request); on vanilla niri the subprocess
//! fails and the error propagates with the compositor's own message.

use crate::niri;
use crate::registry::VerbArgs;
use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot, args: &VerbArgs) -> anyhow::Result<()> {
    let names = match args
        .first
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(activity) => niri::workspace_names_in_activity(activity)?,
        None => niri::workspace_names()?,
    };
    for name in names {
        println!("{name}");
    }
    Ok(())
}
