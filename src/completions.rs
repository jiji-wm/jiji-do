//! Shell completions for jiji-do.
//!
//! [`run`] generates static completions from the clap surface and writes them
//! to stdout. For fish, a dynamic activity-name augmentation is appended after
//! the `clap_complete` base: tab-completing the positional argument of
//! `switch-activity`, `move-window-to-activity`, `move-workspace-to-activity`,
//! `remove-activity`, and `save-activity` shells back into
//! `jiji-activities list --format=name` for live candidates. Bash, zsh,
//! elvish, and PowerShell receive the static base only; dynamic variants for
//! those shells are out of scope until there is concrete demand.
//!
//! Verbs deliberately absent from the dynamic set:
//!
//! - `create-activity` — the argument is a new name; completing against
//!   existing names would be misleading.
//! - `assign-workspace` — takes no positional argument. The picker handles
//!   multi-select internally; the CLI surface itself is a unit variant. Any
//!   completion at `assign-workspace <TAB>` is wrong.
//! - `switch-activity-previous`, `move-window-here`, `list-activities`,
//!   `completions` — no activity-name positional.
//!
//! ## Position-aware conditions
//!
//! The augmentation uses two helper functions to fire only where activity
//! names are accepted:
//!
//! - `__fish_jiji_do_using_subcommand <name>` — clap_complete's own helper,
//!   true when the user is currently inside the named subcommand (parses
//!   global flags correctly, unlike the looser `__fish_seen_subcommand_from`).
//! - `__jiji_do_no_positional_yet` — emitted by this module; true when no
//!   positional arg has been provided after the subcommand. Combined with the
//!   using-subcommand check, this restricts completion to the *first*
//!   positional position for single-arg verbs.
//!
//! All current dynamic verbs accept exactly one positional, so the combined
//! condition is uniform. If a future verb accepts multiple activity-name
//! positionals (variadic), drop the `no_positional_yet` guard for that verb
//! so completion fires at every position.

use std::io::{self, Write};

use anyhow::Result;
use clap::CommandFactory;
use clap_complete::Shell;

use crate::cli::Cli;

/// Subcommands accepting exactly one activity-name positional. Completion
/// fires only at that position; after the user has typed a name, the
/// `__jiji_do_no_positional_yet` helper returns false and the completion
/// stops offering candidates.
const FISH_SINGLE_ARG_VERBS: [&str; 5] = [
    "switch-activity",
    "move-window-to-activity",
    "move-workspace-to-activity",
    "remove-activity",
    "save-activity",
];

/// Shell command invoked at fish tab-completion time to enumerate candidate
/// activity names. `2>/dev/null` swallows the "niri socket unavailable"
/// stderr path so a stopped compositor yields zero candidates silently
/// rather than producing visible error noise during a tab press.
const FISH_NAMES_CMD: &str = "jiji-activities list --format=name 2>/dev/null";

/// Generate shell completions for `shell` and write them to stdout.
///
/// For fish, a dynamic activity-name augmentation is appended after the
/// clap base (see module-level docs).
///
/// # Errors
///
/// Returns an error if stdout cannot be written.
pub fn run(shell: Shell) -> Result<()> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "jiji-do", &mut out);
    if matches!(shell, Shell::Fish) {
        emit_fish_dynamic(&mut out)?;
    }
    out.flush()?;
    Ok(())
}

fn emit_fish_dynamic<W: Write>(w: &mut W) -> io::Result<()> {
    writeln!(w)?;
    writeln!(w, "# Dynamic activity-name completion (position-aware).")?;
    writeln!(w)?;
    emit_no_positional_yet_helper(w)?;
    writeln!(w)?;
    for verb in FISH_SINGLE_ARG_VERBS {
        writeln!(
            w,
            "complete -c jiji-do \
             -n \"__fish_jiji_do_using_subcommand {verb}; \
             and __jiji_do_no_positional_yet\" \
             -f -a \"({FISH_NAMES_CMD})\"",
        )?;
    }
    // Global file-fallback suppression: `-f` alone (no `-a`) tells fish not
    // to offer filesystem completions for argument-less verbs. The per-verb
    // conditional `-f -a` lines above still fire for the dynamic set because
    // fish merges `complete` entries additively; this guard is last so it
    // does not shadow the conditional candidates.
    writeln!(w, "complete -c jiji-do -f")?;
    Ok(())
}

/// Emits a fish helper that returns true iff no positional argument has been
/// provided after the subcommand. Uses `commandline -opc` (tokens before
/// cursor, excluding the current word being completed) and counts non-flag
/// tokens after the first non-flag token (the subcommand).
fn emit_no_positional_yet_helper<W: Write>(w: &mut W) -> io::Result<()> {
    writeln!(
        w,
        "function __jiji_do_no_positional_yet\n    \
             set -l tokens (commandline -opc)\n    \
             set -e tokens[1]\n    \
             set -l found_subcommand 0\n    \
             set -l positional_count 0\n    \
             for tok in $tokens\n        \
                 if string match -q -- '-*' $tok\n            \
                     continue\n        \
                 end\n        \
                 if test $found_subcommand -eq 0\n            \
                     set found_subcommand 1\n            \
                     continue\n        \
                 end\n        \
                 set positional_count (math $positional_count + 1)\n    \
             end\n    \
             test $positional_count -eq 0\n\
         end",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both fish and bash completions must enumerate verb names, because verbs
    /// are subcommands. This pins that the static base is non-empty and
    /// includes at least one known verb.
    #[test]
    fn completions_are_non_empty_and_contain_known_verb() {
        let mut fish_buf = Vec::new();
        clap_complete::generate(Shell::Fish, &mut Cli::command(), "jiji-do", &mut fish_buf);
        let fish_output =
            String::from_utf8(fish_buf).expect("fish completions must be valid UTF-8");
        assert!(
            fish_output.contains("switch-activity"),
            "fish completions must enumerate the registered verb switch-activity"
        );

        let mut bash_buf = Vec::new();
        clap_complete::generate(Shell::Bash, &mut Cli::command(), "jiji-do", &mut bash_buf);
        let bash_output =
            String::from_utf8(bash_buf).expect("bash completions must be valid UTF-8");
        assert!(
            bash_output.contains("switch-activity"),
            "bash completions must enumerate the registered verb switch-activity"
        );
    }

    fn fish_dynamic_bytes() -> Vec<u8> {
        let mut buf = Vec::new();
        emit_fish_dynamic(&mut buf).expect("write to Vec");
        buf
    }

    #[test]
    fn fish_dynamic_guards_every_verb_with_no_positional_yet() {
        // Every current dynamic verb takes exactly one positional name, so
        // the combined position guard applies uniformly. This pins the
        // correct helper name (`__fish_jiji_do_using_subcommand` +
        // `__jiji_do_no_positional_yet`) against the jiji-activities-namespaced
        // helpers that would produce a silently-dead condition.
        let out = String::from_utf8(fish_dynamic_bytes()).unwrap();
        for verb in FISH_SINGLE_ARG_VERBS {
            let needle = format!(
                "__fish_jiji_do_using_subcommand {verb}; \
                 and __jiji_do_no_positional_yet"
            );
            assert!(
                out.contains(&needle),
                "verb `{verb}` missing combined position guard:\n{out}",
            );
        }
    }

    #[test]
    fn fish_dynamic_does_not_emit_line_for_create_activity() {
        // `create-activity` takes a new name; completing against existing
        // names would be misleading. Guards against an accidental addition
        // of "create-activity" to `FISH_SINGLE_ARG_VERBS`.
        let out = String::from_utf8(fish_dynamic_bytes()).unwrap();
        assert!(
            !out.contains("__fish_jiji_do_using_subcommand create-activity"),
            "fish dynamic output must not include `create-activity`:\n{out}",
        );
    }

    #[test]
    fn fish_dynamic_does_not_emit_line_for_assign_workspace() {
        // `assign-workspace` is a unit variant — no positional name.
        // The picker handles multi-select internally; tab-completing at
        // `assign-workspace <TAB>` would offer activity names where none
        // are accepted.
        let out = String::from_utf8(fish_dynamic_bytes()).unwrap();
        assert!(
            !out.contains("__fish_jiji_do_using_subcommand assign-workspace"),
            "fish dynamic output must not include `assign-workspace` \
             (it is a unit variant, picker-only):\n{out}",
        );
    }

    #[test]
    fn fish_dynamic_uses_list_format_name_for_candidates() {
        // Pins the source-of-truth wire: candidates must come from
        // `jiji-activities list --format=name` so a rename of the CLI
        // surface that breaks this contract fails loudly here.
        let out = String::from_utf8(fish_dynamic_bytes()).unwrap();
        assert!(
            out.contains("(jiji-activities list --format=name 2>/dev/null)"),
            "fish dynamic output must invoke `list --format=name`:\n{out}",
        );
    }

    #[test]
    fn fish_dynamic_defines_no_positional_yet_helper() {
        // The helper function definition must be emitted before the
        // `complete` lines that reference it; otherwise fish would log a
        // "unknown function" warning on every tab press.
        let out = String::from_utf8(fish_dynamic_bytes()).unwrap();
        assert!(
            out.contains("function __jiji_do_no_positional_yet"),
            "fish dynamic output must define the position-guard helper:\n{out}",
        );
        let helper_pos = out.find("function __jiji_do_no_positional_yet").unwrap();
        let first_use_pos = out.find("and __jiji_do_no_positional_yet").unwrap();
        assert!(
            helper_pos < first_use_pos,
            "helper function must be defined before first use",
        );
    }

    #[test]
    fn fish_dynamic_emits_global_file_fallback_guard() {
        // The global `complete -c jiji-do -f` line must be present and must
        // come after at least one per-verb conditional `-f -a` line, so that
        // the global guard does not shadow the activity-name candidates.
        let out = String::from_utf8(fish_dynamic_bytes()).unwrap();
        assert!(
            out.contains("complete -c jiji-do -f\n") || out.ends_with("complete -c jiji-do -f"),
            "fish dynamic output must contain the global file-fallback guard:\n{out}",
        );
        // A per-verb line must also be present (guards against a regression
        // where only the global guard was emitted with no dynamic candidates).
        assert!(
            out.contains("-f -a \"(jiji-activities list --format=name 2>/dev/null)\""),
            "fish dynamic output must contain per-verb conditional `-f -a` lines:\n{out}",
        );
        // The global guard must appear after the first per-verb line.
        let first_verb_pos = out
            .find("-f -a \"(jiji-activities list --format=name 2>/dev/null)\"")
            .unwrap();
        let global_guard_pos = out.rfind("complete -c jiji-do -f").unwrap();
        assert!(
            global_guard_pos > first_verb_pos,
            "global `-f` guard must appear after the per-verb conditional lines",
        );
    }
}
