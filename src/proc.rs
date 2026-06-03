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

/// Run a soft-dependency subprocess, optionally piping `stdin`, and wait for
/// it to exit.
///
/// Returns `true` if the command was found, launched, and exited 0. Returns
/// `false` for any other outcome: a spawn error (binary absent, permission
/// denied, etc.) or a non-zero exit. This function never returns `Err` — a
/// missing or failing soft dependency must not fail the calling verb.
///
/// The caller is responsible for falling back to stdout or another routing
/// path when this returns `false`, so that the captured value is never lost.
pub fn run_best_effort(cmd: &str, args: &[&str], stdin: Option<&str>) -> bool {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = match Command::new(cmd)
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    if let Some(text) = stdin
        && let Some(mut handle) = child.stdin.take()
    {
        // Ignore write errors — if stdin closes early the command will
        // still be reaped below and the exit code will surface the failure.
        let _ = handle.write_all(text.as_bytes());
    }

    child.wait().map(|s| s.success()).unwrap_or(false)
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

    #[test]
    fn run_best_effort_returns_false_for_missing_binary() {
        assert!(!run_best_effort(
            "definitely-not-a-real-binary-xyzzy",
            &[],
            None
        ));
    }

    #[test]
    fn run_best_effort_returns_true_for_exit_zero() {
        assert!(run_best_effort("sh", &["-c", "exit 0"], None));
    }

    #[test]
    fn run_best_effort_returns_false_for_exit_nonzero() {
        assert!(!run_best_effort("sh", &["-c", "exit 1"], None));
    }

    #[test]
    fn run_best_effort_stdin_is_written_to_child() {
        // Confirm that the stdin pipe actually delivers bytes to the child.
        // `sh -c 'read x; [ "$x" = hello ] && exit 0 || exit 1'` returns
        // exit 0 only when the first line read from stdin equals "hello".
        assert!(run_best_effort(
            "sh",
            &["-c", r#"read x; [ "$x" = hello ] && exit 0 || exit 1"#],
            Some("hello"),
        ));
    }
}
