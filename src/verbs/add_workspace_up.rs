//! Native verb: insert a new workspace above the current one and focus it.
//!
//! Dispatches `niri msg action add-workspace-up` (zero arguments; the
//! compositor always focuses the new workspace). Fork-only action — upstream
//! niri's clap parser rejects the subcommand and the subprocess fails loudly.
//!
//! The new workspace is ephemeral: the compositor prunes it on the next focus
//! change if it is left empty and unnamed. Populate or name it to keep it.

use crate::registry::VerbArgs;
use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot, _args: &VerbArgs) -> anyhow::Result<()> {
    crate::niri::run_action("add-workspace-up")
}
