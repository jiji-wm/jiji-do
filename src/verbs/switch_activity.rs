//! Passthrough verb: delegate to `jiji-activities switch [<name>]`. When a
//! name is supplied it is forwarded directly, bypassing the picker in
//! jiji-activities. Gated on FORK + NIRI_ACTIVITIES by the registry, so by
//! the time this runs those deps are guaranteed present.

use crate::registry::VerbArgs;
use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot, args: &VerbArgs) -> anyhow::Result<()> {
    // Normalize: empty or whitespace-only positional is treated the same as
    // absent — routes to the jiji-activities picker rather than dispatching
    // `jiji-activities switch ""`.
    let supplied = args
        .first
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    // No snapshot needed: `jiji-activities switch` resolves its own target via
    // its own picker when no name is supplied. We just hand off.
    match supplied {
        Some(name) => crate::proc::run_capture("jiji-activities", &["switch", name])?,
        None => crate::proc::run_capture("jiji-activities", &["switch"])?,
    };
    Ok(())
}
