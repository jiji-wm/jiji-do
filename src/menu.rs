//! fuzzel-backed pickers. `pick_one` writes newline-joined items to fuzzel's
//! stdin and returns the chosen line, or `None` on cancel.

use anyhow::Context;
use std::io::Write;
use std::process::{Command, Stdio};

/// fuzzel's built-in default window width (characters); also our floor.
const FUZZEL_MIN_WIDTH: usize = 30;

/// Upper bound so a pathologically long prompt or item (e.g. a window title
/// in the move-window pickers) cannot stretch the picker across the screen.
const FUZZEL_MAX_WIDTH: usize = 120;

/// Typing room reserved after the prompt — prompt and typed input share
/// fuzzel's single input row.
const PROMPT_TYPING_PAD: usize = 12;

/// Slack past the longest item for the selection highlight margin.
const ITEM_PAD: usize = 4;

/// `--width` (fuzzel's unit: characters) needed so neither the prompt row
/// nor any item is truncated by fuzzel's 30-character default: max of
/// prompt + typing room and longest item + slack, clamped to
/// [`FUZZEL_MIN_WIDTH`]..=[`FUZZEL_MAX_WIDTH`]. Counts `char`s, not bytes.
fn fuzzel_width(prompt: &str, items: &[String]) -> usize {
    let prompt_need = prompt.chars().count() + PROMPT_TYPING_PAD;
    let item_need = items
        .iter()
        .map(|i| i.chars().count() + ITEM_PAD)
        .max()
        .unwrap_or(0);
    prompt_need
        .max(item_need)
        .clamp(FUZZEL_MIN_WIDTH, FUZZEL_MAX_WIDTH)
}

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
    let width = fuzzel_width(prompt, items).to_string();
    let mut child = Command::new("fuzzel")
        .args(["--dmenu", "--prompt", prompt, "--width", &width])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawning fuzzel")?;
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(items.join("\n").as_bytes())
        .context("writing items to fuzzel stdin")?;
    let out = child.wait_with_output().context("waiting for fuzzel")?;
    let sel = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if out.status.success() {
        return Ok(if sel.is_empty() { None } else { Some(sel) });
    }
    match out.status.code() {
        Some(1) => Ok(None), // fuzzel exits 1 on user cancel
        other => anyhow::bail!(
            "fuzzel failed (exit {}): {}",
            other
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".into()),
            String::from_utf8_lossy(&out.stderr).trim()
        ),
    }
}

/// Spawn `fuzzel --dmenu --prompt <prompt>` with an empty stdin (sending EOF
/// immediately so fuzzel enters free-text mode with no candidate list), wait
/// for the user to type a name, and return it.
///
/// Shares the cancel-vs-failure discrimination shape with [`pick_one`]: only
/// fuzzel exit code 1 is treated as a clean cancel (`Ok(None)`); exit ≥2 or
/// signal termination propagates as an error. This mirrors the lesson from
/// `a0eaccc` that collapsed all non-success into `None` and masked real
/// failures.
///
/// The empty-stdin / free-text contract follows `jiji-activities`'s
/// `picker::single_select::prompt_name`: close the stdin pipe before calling
/// `wait_with_output` so fuzzel receives EOF and does not block waiting for
/// the candidate list.
///
/// **Return shape:**
/// - `Ok(Some(name))` — success exit, non-empty trimmed first line.
/// - `Ok(None)` — success exit with blank stdout (Enter without typing) OR
///   fuzzel exit code 1 (user cancelled). Both are clean no-ops for the
///   caller.
/// - `Err(_)` — exit ≥2 or signal termination.
pub fn prompt_name(prompt: &str) -> anyhow::Result<Option<String>> {
    let width = fuzzel_width(prompt, &[]).to_string();
    let mut child = Command::new("fuzzel")
        .args(["--dmenu", "--prompt", prompt, "--width", &width])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawning fuzzel")?;
    // Drop stdin immediately to send EOF — fuzzel --dmenu reads the candidate
    // list to EOF before drawing the prompt; empty stdin = free-text prompt.
    drop(child.stdin.take());
    let out = child.wait_with_output().context("waiting for fuzzel")?;
    if out.status.success() {
        let name = String::from_utf8_lossy(&out.stdout)
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        return Ok(if name.is_empty() { None } else { Some(name) });
    }
    match out.status.code() {
        Some(1) => Ok(None), // fuzzel exits 1 on user cancel
        other => anyhow::bail!(
            "fuzzel failed (exit {}): {}",
            other
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".into()),
            String::from_utf8_lossy(&out.stderr).trim()
        ),
    }
}

/// Spawn `fuzzel --dmenu --prompt <prompt>`, present two choices `No` and `Yes`
/// (No first — never default to Yes), and return whether the user confirmed.
///
/// The affirmative is an explicit allowlisted match: only the exact trimmed
/// text `"Yes"` is treated as confirmation. Any other selection (including
/// `"No"`, blank, or free-text echoed by fuzzel) returns `Ok(false)`. This
/// strictness prevents an unexpected fuzzel echo from ever being read as consent.
///
/// Cancel-vs-failure discrimination follows the same shape as [`prompt_name`]:
/// only fuzzel exit code 1 (user cancelled — Escape or close) is treated as a
/// clean `Ok(false)`; exit ≥2 or signal termination propagates as an error.
///
/// **Return shape:**
/// - `Ok(true)` — success exit and stdout trims to exactly `"Yes"`.
/// - `Ok(false)` — success exit with any other selection, OR fuzzel exit code 1
///   (cancel / Escape). Both are clean no-ops for the caller.
/// - `Err(_)` — exit ≥2 or signal termination.
pub fn confirm(prompt: &str) -> anyhow::Result<bool> {
    // Items are just No/Yes; the (often long) confirm prompt drives width.
    let width = fuzzel_width(prompt, &[]).to_string();
    let mut child = Command::new("fuzzel")
        .args(["--dmenu", "--prompt", prompt, "--width", &width])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawning fuzzel")?;
    // BrokenPipe means fuzzel exited before reading stdin — fall through to
    // wait_with_output so the exit code governs the outcome, not the write error.
    // The taken ChildStdin handle drops at the end of this if-let expression,
    // closing stdin (sending EOF) before wait_with_output in all paths — the same
    // EOF mechanism prompt_name makes explicit with drop(). A future refactor that
    // binds this handle to a longer-lived `let` would deadlock fuzzel waiting for
    // more candidates.
    if let Err(e) = child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(b"No\nYes")
        && e.kind() != std::io::ErrorKind::BrokenPipe
    {
        return Err(e).context("writing choices to fuzzel stdin");
    }
    let out = child.wait_with_output().context("waiting for fuzzel")?;
    if out.status.success() {
        let sel = String::from_utf8_lossy(&out.stdout)
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        return Ok(sel == "Yes");
    }
    match out.status.code() {
        Some(1) => Ok(false), // fuzzel exits 1 on user cancel
        other => anyhow::bail!(
            "fuzzel failed (exit {}): {}",
            other
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".into()),
            String::from_utf8_lossy(&out.stderr).trim()
        ),
    }
}

/// Resolve a label echoed by the picker back to the matching choice.
///
/// Scans `choices` for the entry whose label (extracted by `label_fn`) equals
/// `picked`, and returns a reference to it. Returns `Err` when no entry
/// matches — a future refactor to a silent default would break callers and
/// leave the connector/id unresolved.
///
/// This is the shared resolver for all picker-based verb families (output
/// picker, workspace picker) that follow the pattern:
///
/// 1. Build a `Vec<T>` of choices.
/// 2. Feed labels into `pick_one`.
/// 3. Resolve the echoed label back to a `&T` before dispatching the action.
pub fn resolve_by_label<'a, T>(
    choices: &'a [T],
    picked: &str,
    label_fn: impl Fn(&T) -> &str,
) -> anyhow::Result<&'a T> {
    choices
        .iter()
        .find(|c| label_fn(c) == picked)
        .ok_or_else(|| anyhow::anyhow!("picker returned unknown label: {picked}"))
}

use crate::registry::Verb;

/// Render the verb menu: fuzzel over enabled verbs' labels, return the chosen
/// verb. `None` on cancel.
pub fn render_menu(enabled: &[&'static Verb]) -> anyhow::Result<Option<&'static Verb>> {
    let labels: Vec<String> = enabled.iter().map(|v| v.label.to_string()).collect();
    // Trailing space: fuzzel renders the prompt verbatim, flush against the
    // typed text — without it the input abuts "jiji-do".
    let Some(picked) = pick_one("jiji-do ", &labels)? else {
        return Ok(None);
    };
    Ok(enabled.iter().copied().find(|v| v.label == picked))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzel_width_floors_at_fuzzel_default_for_short_content() {
        assert_eq!(fuzzel_width("> ", &["a".to_owned()]), FUZZEL_MIN_WIDTH);
    }

    #[test]
    fn fuzzel_width_grows_with_long_prompt() {
        let prompt = "Move window \"some quite long focused window title\" to:";
        let expected = prompt.chars().count() + PROMPT_TYPING_PAD;
        assert_eq!(fuzzel_width(prompt, &[]), expected);
    }

    #[test]
    fn fuzzel_width_grows_with_longest_item() {
        let items = vec!["short".to_owned(), "x".repeat(60)];
        assert_eq!(fuzzel_width("> ", &items), 60 + ITEM_PAD);
    }

    #[test]
    fn fuzzel_width_caps_at_max() {
        // Window titles can be arbitrarily long; the picker must not
        // stretch across the screen.
        let prompt = "t".repeat(500);
        assert_eq!(fuzzel_width(&prompt, &[]), FUZZEL_MAX_WIDTH);
    }

    #[test]
    fn fuzzel_width_counts_chars_not_bytes() {
        // 40 multi-byte chars: width follows the char count (40 + pad),
        // not the byte count (which would overshoot toward the cap).
        let items = vec!["ž".repeat(40)];
        assert_eq!(fuzzel_width("> ", &items), 40 + ITEM_PAD);
    }
}
