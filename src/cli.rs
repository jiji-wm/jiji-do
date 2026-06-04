//! clap surface. Verbs are modelled as subcommands so clap_complete's fish
//! generator emits verb-name completions. The registry remains the single
//! source of truth for dispatch, labels, Category, capability sets,
//! `menu_visible`, and the dispatch fn pointer; this enum exists solely so
//! the shell completion generators know the verb names. `--debug` reports
//! capability filtering. The `completions` subcommand generates shell
//! completions and returns before any capability probe.

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
    /// Switch to a workspace (picker; filtered to the current activity).
    SwitchWorkspace,
    /// Switch to a workspace in any activity (picker; switches activity too).
    SwitchWorkspaceAll,
    /// Focus the previously-active workspace.
    FocusWorkspacePrevious,
    /// Unset the name of the focused workspace.
    UnsetWorkspaceName,
    /// Rename the focused workspace (fuzzel free-text prompt).
    RenameWorkspace,

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
            Cmd::SwitchWorkspace => Some("switch-workspace"),
            Cmd::SwitchWorkspaceAll => Some("switch-workspace-all"),
            Cmd::FocusWorkspacePrevious => Some("focus-workspace-previous"),
            Cmd::UnsetWorkspaceName => Some("unset-workspace-name"),
            Cmd::RenameWorkspace => Some("rename-workspace"),
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

    /// Returns the optional positional argument for the six name-bearing variants
    /// (`SwitchActivity`, `MoveWindowToActivity`, `MoveWorkspaceToActivity`,
    /// `SaveActivity`, `CreateActivity`, `RemoveActivity`), `None` for all others.
    pub fn verb_arg(&self) -> Option<&str> {
        match self {
            Cmd::SwitchActivity { verb_arg } => verb_arg.as_deref(),
            Cmd::MoveWindowToActivity { verb_arg } => verb_arg.as_deref(),
            Cmd::MoveWorkspaceToActivity { verb_arg } => verb_arg.as_deref(),
            Cmd::SaveActivity { verb_arg } => verb_arg.as_deref(),
            Cmd::CreateActivity { verb_arg } => verb_arg.as_deref(),
            Cmd::RemoveActivity { verb_arg } => verb_arg.as_deref(),
            _ => None,
        }
    }
}
