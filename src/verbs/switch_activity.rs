use crate::snapshot::Snapshot;

/// Passthrough verb: shell out to `jiji-activities switch`. (Filled in Task 7.)
pub fn run(_snapshot: &Snapshot) -> anyhow::Result<()> {
    anyhow::bail!("not yet implemented")
}
