//! Thin subprocess helpers, kept separate so the rest of the crate depends on
//! a tiny seam rather than `std::process` directly.

use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

/// Walks `$PATH` looking for an executable named `bin`. Returns the first hit.
/// Empty `$PATH` components are skipped (a leading/trailing `:` must not match
/// the current directory).
pub fn which(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .filter(|p| !p.as_os_str().is_empty())
        .map(|dir| dir.join(bin))
        .find(|cand| cand.is_file() && is_executable(cand))
}

#[cfg(unix)]
fn is_executable(p: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Runs `cmd args...`, capturing stdout. Returns the raw stdout on exit 0;
/// an error otherwise (with stderr in the message).
pub fn run_capture<S: AsRef<OsStr>>(cmd: &str, args: &[S]) -> anyhow::Result<String> {
    let out = Command::new(cmd).args(args).output()?;
    if !out.status.success() {
        anyhow::bail!(
            "{cmd} exited {}: {}",
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8(out.stdout)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn which_finds_sh() {
        // `sh` is on PATH on any Unix dev/CI box.
        assert!(which("sh").is_some());
    }

    #[test]
    fn which_misses_nonexistent() {
        assert!(which("definitely-not-a-real-binary-xyzzy").is_none());
    }
}
