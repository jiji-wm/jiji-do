//! Shell completions for jiji-do.
//!
//! [`run`] generates static completions from the clap surface and writes them
//! to stdout. For fish, a dynamic name-completion augmentation is appended
//! after the `clap_complete` base. Bash, zsh, elvish, and PowerShell receive
//! the static base only; dynamic variants for those shells are out of scope
//! until there is concrete demand.
//!
//! ## Dynamic completion table
//!
//! The augmentation is driven by [`FISH_DYNAMIC`], a `(verb, slot, candidates
//! command)` table. Each entry fires iff the user is completing exactly the
//! Nth positional of that verb (slot-1 positionals already typed), where N is
//! the 1-based slot number. The candidates command is a shell expression
//! invoked at tab-completion time to enumerate live names.
//!
//! Three position-aware helper functions are emitted before the table-driven
//! `complete` lines:
//!
//! - `__jiji_do_positionals` — echoes (one per line) the positional tokens
//!   typed after the subcommand, skipping `-*` flags.
//! - `__jiji_do_positional_count_is N` — true iff exactly N positionals have
//!   been typed before the current word; call sites pass `slot - 1` so this
//!   fires exactly when the user is entering positional slot `slot` (combined
//!   with `__fish_jiji_do_using_subcommand` to restrict each entry to its
//!   exact slot).
//! - `__jiji_do_first_positional` — echoes the first positional, or nothing.
//!   Used by the slot-2 workspace-name entry in `switch-workspace-all` to
//!   scope candidates to the already-typed activity.
//!
//! The `__fish_jiji_do_using_subcommand` helper is clap_complete's own; it
//! parses global flags correctly, unlike the looser `__fish_seen_subcommand_from`.
//!
//! ## Exclusions
//!
//! - `create-activity` — the argument is a new name; completing against
//!   existing names would be misleading.
//! - `assign-workspace` and other unit variants — no positional argument.
//!   The picker handles multi-select internally; the CLI surface itself
//!   accepts no positionals.
//!
//! ## Known limitation
//!
//! The positional counter skips `-*` tokens but not a value belonging to a
//! value-taking flag. No dynamic verb currently has such a flag; adding one
//! would require teaching `__jiji_do_positionals` the flag's name.

use std::io::{self, Write};

use anyhow::Result;
use clap::CommandFactory;
use clap_complete::Shell;

use crate::cli::Cli;

/// Candidate-producing shell commands, invoked at fish tab-completion time.
/// `2>/dev/null` swallows the "niri socket unavailable" stderr path so a
/// stopped compositor yields zero candidates silently rather than visible
/// error noise during a tab press.
const FISH_ACTIVITY_NAMES_CMD: &str = "jiji-activities list --format=name 2>/dev/null";
/// Workspace completion candidates for the current activity. Rows are
/// `token\tdescription`; fish inserts the token and renders the description
/// grey. Unnamed workspaces complete by per-monitor index (focused output)
/// or `id:N` (other outputs), so an all-unnamed session produces a non-empty
/// menu instead of an empty one.
const FISH_WORKSPACE_REFS_CMD: &str = "jiji-do list-workspaces --complete 2>/dev/null";
/// Workspace completion candidates scoped to the activity the user already
/// typed as the first positional (extracted by the `__jiji_do_first_positional`
/// helper). Rows are `token\tdescription`; unnamed workspaces always use
/// `id:N` because the compositor rejects bare indices with `--activity`.
/// `2>/dev/null` swallows the "niri socket unavailable" path AND the
/// legitimate `unknown activity` exit-1 produced when the typed slot-1
/// value is not a recognised activity name — in both cases zero candidates
/// is the correct and silent outcome.
const FISH_WORKSPACE_REFS_IN_ACT_CMD: &str =
    "jiji-do list-workspaces --complete --activity (__jiji_do_first_positional) 2>/dev/null";

/// Dynamic completion table: `(verb, 1-based positional slot, candidates
/// command)`. A slot-N entry fires iff the user is completing the Nth
/// positional of that verb — exactly N−1 positionals already typed.
///
/// Excluded on purpose: `create-activity` (argument is a NEW name —
/// completing existing names would mislead) and every unit variant
/// (`assign-workspace` etc. — no positional exists; the picker handles
/// selection internally).
///
/// Known limitation: the positional counter skips `-*` tokens but not a
/// value belonging to a value-taking flag. No dynamic verb has such a flag
/// today; adding one requires teaching `__jiji_do_positionals` its name.
const FISH_DYNAMIC: &[(&str, u8, &str)] = &[
    ("switch-activity", 1, FISH_ACTIVITY_NAMES_CMD),
    ("move-window-to-activity", 1, FISH_ACTIVITY_NAMES_CMD),
    ("move-workspace-to-activity", 1, FISH_ACTIVITY_NAMES_CMD),
    ("remove-activity", 1, FISH_ACTIVITY_NAMES_CMD),
    ("save-activity", 1, FISH_ACTIVITY_NAMES_CMD),
    ("switch-workspace", 1, FISH_WORKSPACE_REFS_CMD),
    ("switch-workspace-all", 1, FISH_ACTIVITY_NAMES_CMD),
    ("switch-workspace-all", 2, FISH_WORKSPACE_REFS_IN_ACT_CMD),
];

/// Generate shell completions for `shell` and write them to stdout.
///
/// For fish, the clap_complete base is post-processed to strip the static
/// `-l complete` flag registration (the `--complete` flag on `list-workspaces`
/// is plumbing hidden from `--help` via `hide = true`, but clap_complete does
/// not honour `hide` in the generated text). A dynamic name-completion
/// augmentation is then appended (see module-level docs).
///
/// # Errors
///
/// Returns an error if stdout cannot be written.
pub fn run(shell: Shell) -> Result<()> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut cmd = Cli::command();
    if matches!(shell, Shell::Fish) {
        // Collect the clap base into a buffer, strip the hidden `--complete`
        // flag registration, then flush the cleaned text.
        let mut buf = Vec::new();
        clap_complete::generate(shell, &mut cmd, "jiji-do", &mut buf);
        let raw = String::from_utf8(buf).map_err(|e| anyhow::anyhow!("{e}"))?;
        let stripped = strip_hidden_complete_flag_fish(&raw);
        out.write_all(stripped.as_bytes())?;
        emit_fish_dynamic(&mut out)?;
    } else {
        clap_complete::generate(shell, &mut cmd, "jiji-do", &mut out);
    }
    out.flush()?;
    Ok(())
}

/// Remove the static `-l complete` flag-registration line that clap_complete
/// emits for the hidden `--complete` flag on `list-workspaces`. The flag is
/// declared with `hide = true` so it does not appear in `--help`, but
/// clap_complete ignores `hide` when generating text.
///
/// The strip targets lines of the form:
///   `complete … -l complete -d '…'`
/// and must NOT remove lines that merely contain the string `--complete`
/// inside a command-substitution argument (those are legitimate dynamic
/// candidate commands, e.g. `(jiji-do list-workspaces --complete …)`).
///
/// The discriminator: a static flag registration contains ` -l complete`
/// followed by a space or end-of-line (as a bare long-flag token), whereas
/// the dynamic command strings contain `--complete` (double-dash) inside a
/// quoted command substitution. Stripping ` -l complete ` (with surrounding
/// spaces / end-of-line) is safe and will not accidentally remove the dynamic
/// lines.
fn strip_hidden_complete_flag_fish(src: &str) -> String {
    src.lines()
        .filter(|line| {
            // Retain this line unless it is the static `-l complete` flag registration.
            // The flag registration has ` -l complete ` (space-bounded) or
            // ` -l complete` at end-of-line; the dynamic candidate lines use
            // `--complete` (two dashes) inside a parenthesised command string.
            let is_flag_registration = line.contains(" -l complete ")
                || line.ends_with(" -l complete")
                || line.contains(" -l complete\t");
            !is_flag_registration
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn emit_fish_dynamic<W: Write>(w: &mut W) -> io::Result<()> {
    writeln!(w)?;
    writeln!(w, "# Dynamic name completion (position-aware).")?;
    writeln!(w)?;
    emit_positional_helpers(w)?;
    writeln!(w)?;
    for (verb, slot, cmd) in FISH_DYNAMIC {
        writeln!(
            w,
            "complete -c jiji-do \
             -n \"__fish_jiji_do_using_subcommand {verb}; \
             and __jiji_do_positional_count_is {}\" \
             -f -a \"({cmd})\"",
            slot - 1,
        )?;
    }
    // Flag-value completion: `list-workspaces --activity <TAB>` offers
    // activity names. `-x` = the flag takes a required argument completed
    // exclusively from the candidate list.
    writeln!(
        w,
        "complete -c jiji-do \
         -n \"__fish_jiji_do_using_subcommand list-workspaces\" \
         -l activity -x -a \"({FISH_ACTIVITY_NAMES_CMD})\"",
    )?;
    // Global file-fallback suppression: `-f` alone (no `-a`) tells fish not
    // to offer filesystem completions for argument-less verbs. The per-verb
    // conditional `-f -a` lines above still fire because fish merges
    // `complete` entries additively; this guard is last so it does not
    // shadow the conditional candidates.
    writeln!(w, "complete -c jiji-do -f")?;
    Ok(())
}

/// Emits the fish positional helpers:
///
/// - `__jiji_do_positionals` — echoes (one per line) the positional tokens
///   typed after the subcommand, skipping `-*` flags. Uses
///   `commandline -opc` (tokens before cursor, excluding the in-progress
///   word).
/// - `__jiji_do_positional_count_is N` — true iff exactly N positionals
///   have been typed before the current word. Call sites pass `slot - 1`,
///   so this fires exactly when the user is entering positional slot `slot`.
/// - `__jiji_do_first_positional` — echoes the first positional (the typed
///   activity for slot-2 workspace candidates), or nothing.
fn emit_positional_helpers<W: Write>(w: &mut W) -> io::Result<()> {
    writeln!(
        w,
        "function __jiji_do_positionals\n    \
             set -l tokens (commandline -opc)\n    \
             set -e tokens[1]\n    \
             set -l found_subcommand 0\n    \
             for tok in $tokens\n        \
                 if string match -q -- '-*' $tok\n            \
                     continue\n        \
                 end\n        \
                 if test $found_subcommand -eq 0\n            \
                     set found_subcommand 1\n            \
                     continue\n        \
                 end\n        \
                 echo $tok\n    \
             end\n\
         end\n\
         \n\
         function __jiji_do_positional_count_is\n    \
             test (count (__jiji_do_positionals)) -eq $argv[1]\n\
         end\n\
         \n\
         function __jiji_do_first_positional\n    \
             set -l pos (__jiji_do_positionals)\n    \
             if set -q pos[1]\n        \
                 echo $pos[1]\n    \
             end\n\
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
    fn fish_dynamic_guards_every_table_entry_with_slot_count() {
        // Each (verb, slot) entry must fire iff exactly slot-1 positionals
        // are already typed. Pins the helper names against the
        // jiji-activities-namespaced variants (silently-dead condition).
        let out = String::from_utf8(fish_dynamic_bytes()).unwrap();
        for (verb, slot, _) in FISH_DYNAMIC {
            let needle = format!(
                "__fish_jiji_do_using_subcommand {verb}; \
                 and __jiji_do_positional_count_is {}",
                slot - 1
            );
            assert!(
                out.contains(&needle),
                "entry `{verb}` slot {slot} missing combined position guard:\n{out}",
            );
        }
    }

    #[test]
    fn fish_dynamic_completes_workspace_names_for_switch_workspace() {
        let out = String::from_utf8(fish_dynamic_bytes()).unwrap();
        assert!(
            out.contains("(jiji-do list-workspaces --complete 2>/dev/null)"),
            "switch-workspace candidates must come from list-workspaces --complete:\n{out}",
        );
    }

    #[test]
    fn fish_dynamic_scopes_second_slot_to_typed_activity() {
        // switch-workspace-all's workspace slot must pass the already-typed
        // activity through to list-workspaces --complete --activity.
        let out = String::from_utf8(fish_dynamic_bytes()).unwrap();
        assert!(
            out.contains(
                "(jiji-do list-workspaces --complete --activity (__jiji_do_first_positional) 2>/dev/null)"
            ),
            "slot-2 candidates must be scoped by the first positional:\n{out}",
        );
    }

    #[test]
    fn fish_dynamic_completes_activity_flag_value_for_list_workspaces() {
        let out = String::from_utf8(fish_dynamic_bytes()).unwrap();
        // Pin the full fragment: the subcommand guard, the long-form flag, and
        // the candidate source — a future rename of any piece breaks the
        // completion silently unless this assertion catches it.
        assert!(
            out.contains(
                "-n \"__fish_jiji_do_using_subcommand list-workspaces\" \
                 -l activity -x -a \"(jiji-activities list --format=name 2>/dev/null)\""
            ),
            "list-workspaces --activity values must complete activity names via \
             `jiji-activities list --format=name`:\n{out}",
        );
    }

    #[test]
    fn fish_dynamic_defines_helpers_before_first_use() {
        let out = String::from_utf8(fish_dynamic_bytes()).unwrap();
        for helper in [
            "__jiji_do_positionals",
            "__jiji_do_positional_count_is",
            "__jiji_do_first_positional",
        ] {
            let def = out
                .find(&format!("function {helper}"))
                .unwrap_or_else(|| panic!("helper {helper} not defined:\n{out}"));
            let first_use = out[def + helper.len() + 9..]
                .find(helper)
                .map(|p| p + def + helper.len() + 9);
            if let Some(use_pos) = first_use {
                assert!(def < use_pos, "{helper} must be defined before use");
            }
        }
    }

    #[test]
    fn fish_dynamic_does_not_emit_line_for_create_activity() {
        // `create-activity` takes a new name; completing against existing
        // names would be misleading. Guards against an accidental addition
        // of "create-activity" to `FISH_DYNAMIC`.
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

    /// The generated fish completions must not contain a static `-l complete`
    /// flag registration. clap_complete ignores `hide = true`; `run()` strips
    /// the line via `strip_hidden_complete_flag_fish`. The dynamic candidate
    /// commands (`--complete` inside parenthesised command substitutions like
    /// `(jiji-do list-workspaces --complete …)`) are emitted by `emit_fish_dynamic`
    /// and must survive — the pin distinguishes the two patterns.
    #[test]
    fn fish_completions_do_not_expose_hidden_complete_flag() {
        // Verify the strip function removes the static flag registration from
        // the raw clap-generated base.
        let mut buf = Vec::new();
        clap_complete::generate(Shell::Fish, &mut Cli::command(), "jiji-do", &mut buf);
        let raw = String::from_utf8(buf).unwrap();
        let stripped = strip_hidden_complete_flag_fish(&raw);
        for line in stripped.lines() {
            assert!(
                !line.contains(" -l complete ")
                    && !line.ends_with(" -l complete")
                    && !line.contains(" -l complete\t"),
                "static `-l complete` flag registration must not appear in stripped output: {line:?}"
            );
        }
        // Confirm the strip function is needed: the raw clap output does contain
        // the problematic registration before stripping.
        assert!(
            raw.contains(" -l complete ") || raw.ends_with(" -l complete"),
            "clap_complete must emit the hidden flag (so the strip is load-bearing): raw output did not contain ` -l complete`"
        );
        // Verify that the dynamic `emit_fish_dynamic` output (the other half of
        // the fish completions) retains the `--complete` candidate commands.
        let dynamic = String::from_utf8(fish_dynamic_bytes()).unwrap();
        assert!(
            dynamic.contains("list-workspaces --complete"),
            "dynamic candidate commands mentioning --complete must survive (not stripped): {dynamic}",
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
