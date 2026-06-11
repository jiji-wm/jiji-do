//! Direct-CLI-only data verb: print workspace names or completion-candidate
//! rows, one per line.
//!
//! Default scope (no flags): named workspaces of the current activity only.
//! Unnamed workspaces are omitted because they have no typeable name
//! reference; they remain reachable through per-monitor index passthrough.
//! `--activity <name>` restricts the scope to that activity's named
//! workspaces.
//!
//! With `--complete`, the verb switches to full completion-candidate mode and
//! emits `token\tdescription` rows for the fish dynamic completion, covering
//! all workspaces (named and unnamed). Tokens follow the typed-reference
//! rules: name when set, per-monitor index on the focused output, `id:N` on
//! other outputs — and always `id:N` under `--activity` (the compositor
//! rejects bare indices combined with an activity qualifier). Descriptions
//! carry `idx N · id:N · <output> · <title>` so an all-unnamed session can
//! tell rows apart. The windows payload is read only in `--complete` mode.
//!
//! Compositor compatibility: the plain form (`--activity` optional, no
//! `--complete`) works on vanilla niri — only workspaces and activities
//! payloads are read, and the activities payload is only fetched when
//! `--activity` is specified. The `--complete --activity <name>` combination
//! requires jiji because `activities` is a jiji-only IPC request; on vanilla
//! niri the subprocess fails and the error propagates. Plain `--complete`
//! (no `--activity`) reads only workspaces and windows — both are available
//! on vanilla niri — so it also works upstream.
//!
//! Primary consumer: the fish completion candidate commands in `completions.rs`;
//! also useful standalone for scripting.

use crate::niri;
use crate::registry::VerbArgs;
use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot, args: &VerbArgs) -> anyhow::Result<()> {
    let activity = args
        .first
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let complete = args.complete;
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
