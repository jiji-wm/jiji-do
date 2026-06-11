//! Native verb: move the focused window to a new workspace above the current one.
//!
//! Dispatches `niri msg action move-window-to-new-workspace-up`, optionally
//! followed by `--focus <bool>` when `args.focus` is `Some`. Fork-only action —
//! upstream niri's clap parser rejects the subcommand and the subprocess fails
//! loudly.
//!
//! When `--focus` is omitted (the default, `args.focus = None`), the compositor
//! default applies. This binary never copies the compositor's `default_value_t`
//! so it remains decoupled from any future retuning of that default.

use crate::niri;
use crate::registry::VerbArgs;
use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot, args: &VerbArgs) -> anyhow::Result<()> {
    niri::move_window_to_new_workspace_up(args.focus)
}
