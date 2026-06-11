//! clap surface. Verbs are modelled as subcommands so clap_complete's fish
//! generator emits verb-name completions. The registry remains the single
//! source of truth for dispatch, labels, Category, capability sets,
//! `menu_visible`, and the dispatch fn pointer; this enum exists solely so
//! the shell completion generators know the verb names. `--debug` reports
//! capability filtering. The `completions` subcommand generates shell
//! completions and returns before any capability probe.

use crate::registry::VerbArgs;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "jiji-do", version, about = "Helix-style launcher for jiji")]
pub struct Cli {
    /// Print resolved capabilities and per-verb kept/filtered status, then exit.
    #[arg(long, global = true)]
    pub debug: bool,

    #[command(subcommand)]
    pub cmd: Option<Cmd>,
}

/// Top-level subcommands. Each verb variant corresponds to a registry entry
/// (see `Cmd::verb_name`); `Completions` emits shell completions and exits.
/// Clap derives kebab-case names automatically from CamelCase variants.
#[derive(Subcommand, Debug)]
pub enum Cmd {
    // ---- Workspace verbs ----
    /// Switch to a workspace (picker, filtered to the current activity;
    /// or directly when a reference is supplied).
    SwitchWorkspace {
        /// Workspace reference: name, per-monitor index, or `id:N` (jiji
        /// only). Skips the fuzzel picker when provided.
        workspace: Option<String>,
    },
    /// Switch to a workspace in any activity (picker; args narrow or skip it).
    SwitchWorkspaceAll {
        /// Activity name: filters the picker to that activity, or — with a
        /// workspace reference — dispatches directly.
        activity: Option<String>,
        /// Workspace reference within the activity: name, per-monitor
        /// index, or `id:N`. Skips the picker entirely.
        workspace: Option<String>,
    },
    /// Focus the previously-active workspace.
    FocusWorkspacePrevious,
    /// Unset the name of the focused workspace.
    UnsetWorkspaceName,
    /// Rename the focused workspace (fuzzel free-text prompt).
    RenameWorkspace,
    /// List workspace names (current activity by default), one per line.
    ListWorkspaces {
        /// List the named activity's workspaces instead of the current
        /// activity's.
        #[arg(long)]
        activity: Option<String>,
        /// Emit completion-candidate rows (token, tab, description) instead
        /// of names. Plumbing for the fish dynamic completion — hidden from
        /// `--help` via `hide = true`. The static fish completions further strip
        /// the flag registration that clap_complete emits despite `hide = true`
        /// (see `completions::strip_hidden_complete_flag_fish`).
        #[arg(long, hide = true)]
        complete: bool,
    },

    /// Add a new workspace above the current one and focus it (fork only).
    AddWorkspaceUp,
    /// Add a new workspace below the current one and focus it (fork only).
    AddWorkspaceDown,
    /// Move the focused window to a new workspace above the current one (fork only).
    MoveWindowToNewWorkspaceUp {
        /// If set, controls whether focus follows the window to the new
        /// workspace. When omitted, the compositor default applies.
        #[arg(long, action = clap::ArgAction::Set)]
        focus: Option<bool>,
    },
    /// Move the focused window to a new workspace below the current one (fork only).
    MoveWindowToNewWorkspaceDown {
        /// If set, controls whether focus follows the window to the new
        /// workspace. When omitted, the compositor default applies.
        #[arg(long, action = clap::ArgAction::Set)]
        focus: Option<bool>,
    },

    // ---- Window verbs ----
    /// Open the compositor's window picker and show the result.
    PickWindow,

    // ---- Monitor verbs ----
    /// Focus a monitor (picker).
    FocusMonitor,
    /// Move the focused window to a monitor (picker).
    MoveWindowToMonitor,
    /// Move the focused column to a monitor (picker).
    MoveColumnToMonitor,
    /// Move the focused workspace to a monitor (picker).
    MoveWorkspaceToMonitor,

    // ---- Mode verbs ----
    /// Toggle the compositor debug tint.
    ToggleDebugTint,

    // ---- Activity verbs ----
    /// Switch to an activity (picker, or directly when name is supplied).
    SwitchActivity {
        /// Activity name (skips the fuzzel picker when provided).
        verb_arg: Option<String>,
    },
    /// Switch to the previously-active activity.
    SwitchActivityPrevious,
    /// Move the focused window to an activity (picker, or directly when name is supplied).
    MoveWindowToActivity {
        /// Activity name (skips the fuzzel picker when provided).
        verb_arg: Option<String>,
    },
    /// Move a window from another activity to this workspace.
    MoveWindowHere,
    /// Move the focused workspace to an activity (picker, or directly when name is supplied).
    MoveWorkspaceToActivity {
        /// Activity name (skips the fuzzel picker when provided).
        verb_arg: Option<String>,
    },
    /// Assign the focused workspace to activities (picker).
    AssignWorkspace,
    /// Save the focused activity via jiji-activities, or save under a given name.
    SaveActivity {
        /// Activity name to save under (derives from focused activity when omitted).
        verb_arg: Option<String>,
    },
    /// List activities and print them to stdout.
    ListActivities,
    /// Create a new activity. Prompts for a name when omitted.
    CreateActivity {
        /// Activity name (skips the fuzzel prompt when provided).
        verb_arg: Option<String>,
    },
    /// Remove an activity. Opens a picker when name is omitted.
    RemoveActivity {
        /// Activity name (skips the fuzzel picker when provided).
        verb_arg: Option<String>,
    },
    /// Rename an activity: pick the target via fuzzel, then prompt for the new name.
    RenameActivity,

    // ---- System verbs ----
    /// Reload the compositor config file.
    ReloadConfig,
    /// Power on all monitors.
    PowerOnMonitors,
    /// Open the compositor's color picker, copy the result, and show a notification.
    PickColor,
    /// Quit jiji (with a fuzzel confirm).
    Quit,
    /// Power off all monitors (with a fuzzel confirm).
    PowerOffMonitors,
    /// Stop an active screencast session (picker).
    StopCast,

    // ---- Meta ----
    /// Emit shell completions for jiji-do and exit.
    Completions {
        /// Shell to generate completions for.
        shell: clap_complete::Shell,
    },
}

impl Cmd {
    /// Returns the canonical registry name for verb variants, `None` for
    /// `Completions`. This is the bridge from the parsed subcommand to
    /// `registry::find`.
    pub fn verb_name(&self) -> Option<&'static str> {
        match self {
            Cmd::SwitchWorkspace { .. } => Some("switch-workspace"),
            Cmd::SwitchWorkspaceAll { .. } => Some("switch-workspace-all"),
            Cmd::FocusWorkspacePrevious => Some("focus-workspace-previous"),
            Cmd::UnsetWorkspaceName => Some("unset-workspace-name"),
            Cmd::RenameWorkspace => Some("rename-workspace"),
            Cmd::ListWorkspaces { .. } => Some("list-workspaces"),
            Cmd::AddWorkspaceUp => Some("add-workspace-up"),
            Cmd::AddWorkspaceDown => Some("add-workspace-down"),
            Cmd::MoveWindowToNewWorkspaceUp { .. } => Some("move-window-to-new-workspace-up"),
            Cmd::MoveWindowToNewWorkspaceDown { .. } => Some("move-window-to-new-workspace-down"),
            Cmd::PickWindow => Some("pick-window"),
            Cmd::FocusMonitor => Some("focus-monitor"),
            Cmd::MoveWindowToMonitor => Some("move-window-to-monitor"),
            Cmd::MoveColumnToMonitor => Some("move-column-to-monitor"),
            Cmd::MoveWorkspaceToMonitor => Some("move-workspace-to-monitor"),
            Cmd::ToggleDebugTint => Some("toggle-debug-tint"),
            Cmd::SwitchActivity { .. } => Some("switch-activity"),
            Cmd::SwitchActivityPrevious => Some("switch-activity-previous"),
            Cmd::MoveWindowToActivity { .. } => Some("move-window-to-activity"),
            Cmd::MoveWindowHere => Some("move-window-here"),
            Cmd::MoveWorkspaceToActivity { .. } => Some("move-workspace-to-activity"),
            Cmd::AssignWorkspace => Some("assign-workspace"),
            Cmd::SaveActivity { .. } => Some("save-activity"),
            Cmd::ListActivities => Some("list-activities"),
            Cmd::CreateActivity { .. } => Some("create-activity"),
            Cmd::RemoveActivity { .. } => Some("remove-activity"),
            Cmd::RenameActivity => Some("rename-activity"),
            Cmd::ReloadConfig => Some("reload-config"),
            Cmd::PowerOnMonitors => Some("power-on-monitors"),
            Cmd::PickColor => Some("pick-color"),
            Cmd::Quit => Some("quit"),
            Cmd::PowerOffMonitors => Some("power-off-monitors"),
            Cmd::StopCast => Some("stop-cast"),
            Cmd::Completions { .. } => None,
        }
    }

    /// Maps the variant's positional fields into the uniform [`VerbArgs`]
    /// passed to every dispatch fn. Single-arg variants fill `first`;
    /// two-arg variants (`SwitchWorkspaceAll`) fill both `first` and `second`;
    /// no-positional variants produce the all-`None` default.
    pub fn verb_args(&self) -> VerbArgs {
        match self {
            Cmd::SwitchActivity { verb_arg }
            | Cmd::MoveWindowToActivity { verb_arg }
            | Cmd::MoveWorkspaceToActivity { verb_arg }
            | Cmd::SaveActivity { verb_arg }
            | Cmd::CreateActivity { verb_arg }
            | Cmd::RemoveActivity { verb_arg } => VerbArgs {
                first: verb_arg.clone(),
                ..Default::default()
            },
            Cmd::ListWorkspaces { activity, complete } => VerbArgs {
                first: activity.clone(),
                complete: *complete,
                ..Default::default()
            },
            Cmd::SwitchWorkspace { workspace } => VerbArgs {
                first: workspace.clone(),
                ..Default::default()
            },
            Cmd::SwitchWorkspaceAll {
                activity,
                workspace,
            } => VerbArgs {
                first: activity.clone(),
                second: workspace.clone(),
                ..Default::default()
            },
            Cmd::MoveWindowToNewWorkspaceUp { focus }
            | Cmd::MoveWindowToNewWorkspaceDown { focus } => VerbArgs {
                focus: *focus,
                ..Default::default()
            },
            _ => VerbArgs::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verb_args_maps_single_arg_variants_to_first() {
        let cmd = Cmd::SwitchActivity {
            verb_arg: Some("work".into()),
        };
        assert_eq!(
            cmd.verb_args(),
            VerbArgs {
                first: Some("work".into()),
                ..Default::default()
            }
        );
    }

    #[test]
    fn verb_args_defaults_for_unit_variants() {
        assert_eq!(Cmd::ReloadConfig.verb_args(), VerbArgs::default());
        assert_eq!(Cmd::AssignWorkspace.verb_args(), VerbArgs::default());
    }

    #[test]
    fn verb_args_maps_two_positional_variant_to_both_slots() {
        let cmd = Cmd::SwitchWorkspaceAll {
            activity: Some("home".into()),
            workspace: Some("mail".into()),
        };
        assert_eq!(
            cmd.verb_args(),
            VerbArgs {
                first: Some("home".into()),
                second: Some("mail".into()),
                ..Default::default()
            }
        );
    }

    /// `MoveWindowToNewWorkspaceUp/Down { focus: Some(false) }` maps to
    /// `VerbArgs { focus: Some(false), .. }`.
    #[test]
    fn verb_args_maps_focus_flag_into_typed_slot() {
        let explicit_false = Cmd::MoveWindowToNewWorkspaceUp { focus: Some(false) };
        assert_eq!(
            explicit_false.verb_args(),
            VerbArgs {
                focus: Some(false),
                ..Default::default()
            }
        );

        let explicit_true = Cmd::MoveWindowToNewWorkspaceDown { focus: Some(true) };
        assert_eq!(
            explicit_true.verb_args(),
            VerbArgs {
                focus: Some(true),
                ..Default::default()
            }
        );

        // flag absent → None
        let absent = Cmd::MoveWindowToNewWorkspaceUp { focus: None };
        assert_eq!(absent.verb_args(), VerbArgs::default());
    }

    /// `ListWorkspaces` with `--complete` maps to `complete: true` in `VerbArgs`,
    /// not a string sentinel in `second`. Without `--complete`, `complete` is `false`.
    #[test]
    fn verb_args_maps_list_workspaces_to_typed_complete_field() {
        // --complete flag set
        let with_complete = Cmd::ListWorkspaces {
            activity: None,
            complete: true,
        };
        assert_eq!(
            with_complete.verb_args(),
            VerbArgs {
                complete: true,
                ..Default::default()
            }
        );

        // no --complete flag
        let without_complete = Cmd::ListWorkspaces {
            activity: Some("work".into()),
            complete: false,
        };
        assert_eq!(
            without_complete.verb_args(),
            VerbArgs {
                first: Some("work".into()),
                complete: false,
                ..Default::default()
            }
        );
    }
}
