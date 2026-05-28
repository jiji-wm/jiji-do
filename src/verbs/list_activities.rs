//! Direct-CLI-only data verb; shells to `jiji-activities list` and forwards
//! stdout verbatim. Snapshot unused — the inventory is read fresh from the
//! compositor inside `jiji-activities`.

use crate::snapshot::Snapshot;

pub fn run(_snapshot: &Snapshot) -> anyhow::Result<()> {
    let stdout = crate::proc::run_capture("jiji-activities", &["list"])?;
    // `print!` not `println!`: jiji-activities list already terminates each row
    // with a newline (one per row via writeln!); println! would emit a spurious
    // blank line after the last entry. The empty-activities case writes nothing,
    // where print! correctly emits nothing.
    print!("{stdout}");
    Ok(())
}
