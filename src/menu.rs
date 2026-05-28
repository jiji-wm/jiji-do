//! fuzzel-backed pickers. `pick_one` writes newline-joined items to fuzzel's
//! stdin and returns the chosen line, or `None` on cancel.

use anyhow::Context;
use std::io::Write;
use std::process::{Command, Stdio};

/// Spawn `fuzzel --dmenu --prompt <prompt>`, feed `items`, return the selected
/// line. `None` = user cancelled (fuzzel exits 1 with empty stdout).
///
/// The "fuzzel missing" case is handled upstream: `switch-workspace` and other
/// verbs that call `pick_one` directly `require` `FUZZEL` (so `main` rejects
/// them with exit 69 before dispatch), and the no-arg menu path gates on
/// `FUZZEL` in `main`. A spawn failure here is the rare "on `$PATH` at probe
/// time but unexecutable now" edge — a generic error (exit 1) is acceptable
/// for it.
pub fn pick_one(prompt: &str, items: &[String]) -> anyhow::Result<Option<String>> {
    let mut child = Command::new("fuzzel")
        .args(["--dmenu", "--prompt", prompt])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("spawning fuzzel")?;
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(items.join("\n").as_bytes())?;
    let out = child.wait_with_output()?;
    let sel = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if out.status.success() && !sel.is_empty() {
        Ok(Some(sel))
    } else {
        Ok(None) // cancelled
    }
}

use crate::registry::Verb;

/// Render the verb menu: fuzzel over enabled verbs' labels, return the chosen
/// verb. `None` on cancel.
pub fn render_menu(enabled: &[&'static Verb]) -> anyhow::Result<Option<&'static Verb>> {
    let labels: Vec<String> = enabled.iter().map(|v| v.label.to_string()).collect();
    let Some(picked) = pick_one("jiji-do", &labels)? else {
        return Ok(None);
    };
    Ok(enabled.iter().copied().find(|v| v.label == picked))
}
