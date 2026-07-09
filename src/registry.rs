//! The static verb registry and capability-based filtering. The registry is
//! the single source of truth for which verbs exist.

use crate::capabilities::Capabilities;
use crate::snapshot::Snapshot;
use crate::verbs;

/// Positional CLI arguments and typed flag fields forwarded to a verb's
/// dispatch fn. `first` and `second` mirror the verb's positional slots; both
/// are `None` for menu invocation and for verbs that take no positionals.
///
/// Producer-side invariant: `second` is `Some` only when `first` is `Some` —
/// clap's left-to-right positional fill guarantees this for multi-positional
/// verbs. Consumers may still defend against the degenerate state if they
/// require an explicit precondition.
///
/// Flag fields (`complete`, `focus`) are typed booleans and `Option<bool>`
/// respectively, so each verb reads its flag directly without reserving a
/// positional slot. Menu dispatch passes `&VerbArgs::default()`, which sets
/// `complete = false` and `focus = None` — semantically identical to the
/// absence of either flag.
///
/// These shared flag fields are proportionate at two; if a third flag field
/// arrives, or two verb families start reading the same flag, replace them
/// with a per-verb typed-args enum instead of widening further.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct VerbArgs {
    pub first: Option<String>,
    pub second: Option<String>,
    /// True when `list-workspaces --complete` was supplied.
    pub complete: bool,
    /// Optional `--focus <bool>` flag for `move-window-to-new-workspace-*`
    /// verbs. `None` means the flag was not supplied; the compositor default
    /// then applies. Never copies `default_value_t` from the compositor to
    /// keep this binary decoupled from future compositor retuning.
    pub focus: Option<bool>,
}

/// Menu grouping. Declaration order is the sort order used by [`enabled`].
/// Current order: `Workspace < Window < Monitor < Mode < Activity < System`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    Workspace,
    Window,
    Monitor,
    Mode,
    Activity,
    System,
}

/// A launcher verb: its CLI name, menu label, category, the capabilities it
/// requires, and the dispatch fn (which consumes the launch snapshot).
pub struct Verb {
    pub name: &'static str,
    pub label: &'static str,
    /// Menu group; drives stable category-ordered sort in [`enabled`].
    pub category: Category,
    /// False for verbs that are direct-CLI only and must not appear in the
    /// fuzzel menu (e.g. data verbs whose stdout has no destination in a
    /// launcher flow).
    pub menu_visible: bool,
    pub requires: Capabilities,
    /// Dispatch function for this verb. Receives the launch-time snapshot and
    /// the positional CLI args (e.g. the name for `create-activity <name>`
    /// in `first`); all-`None` for menu invocation or when the verb takes no
    /// positionals.
    pub dispatch: fn(&Snapshot, &VerbArgs) -> anyhow::Result<()>,
}

impl Verb {
    /// True iff every required capability is present.
    pub fn is_enabled(&self, caps: Capabilities) -> bool {
        caps.contains(self.requires)
    }
}

/// The complete registry. Registration order is stable-sort tiebreaker within
/// each category; [`enabled`] returns verbs sorted by [`Category`] declaration
/// order, preserving intra-category registration order.
pub static REGISTRY: &[Verb] = &[
    Verb {
        name: "switch-workspace",
        label: "Switch workspace",
        category: Category::Workspace,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET.union(Capabilities::FUZZEL),
        dispatch: verbs::switch_workspace::run,
    },
    Verb {
        name: "switch-workspace-all",
        label: "Switch workspace (all activities)",
        category: Category::Workspace,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET
            .union(Capabilities::FUZZEL)
            .union(Capabilities::FORK),
        dispatch: verbs::switch_workspace_all::run,
    },
    Verb {
        name: "focus-workspace-previous",
        label: "Focus previous workspace",
        category: Category::Workspace,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET,
        dispatch: verbs::focus_workspace_previous::run,
    },
    Verb {
        name: "toggle-debug-tint",
        label: "Toggle debug tint",
        category: Category::Mode,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET,
        dispatch: verbs::toggle_debug_tint::run,
    },
    Verb {
        name: "switch-activity",
        label: "Switch activity",
        category: Category::Activity,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET
            .union(Capabilities::FUZZEL)
            .union(Capabilities::FORK)
            .union(Capabilities::NIRI_ACTIVITIES),
        dispatch: verbs::switch_activity::run,
    },
    Verb {
        name: "switch-activity-previous",
        label: "Switch to previous activity",
        category: Category::Activity,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET
            .union(Capabilities::FORK)
            .union(Capabilities::NIRI_ACTIVITIES),
        dispatch: verbs::switch_activity_previous::run,
    },
    Verb {
        name: "move-window-to-activity",
        label: "Move window to activity",
        category: Category::Activity,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET
            .union(Capabilities::FUZZEL)
            .union(Capabilities::FORK)
            .union(Capabilities::NIRI_ACTIVITIES),
        dispatch: verbs::move_window_to_activity::run,
    },
    Verb {
        name: "move-window-here",
        label: "Move window to workspace here",
        category: Category::Activity,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET
            .union(Capabilities::FUZZEL)
            .union(Capabilities::FORK)
            .union(Capabilities::NIRI_ACTIVITIES),
        dispatch: verbs::move_window_here::run,
    },
    Verb {
        name: "move-workspace-to-activity",
        label: "Move workspace to activity",
        category: Category::Activity,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET
            .union(Capabilities::FUZZEL)
            .union(Capabilities::FORK)
            .union(Capabilities::NIRI_ACTIVITIES),
        dispatch: verbs::move_workspace_to_activity::run,
    },
    Verb {
        name: "assign-workspace",
        label: "Assign workspace to activities",
        category: Category::Activity,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET
            .union(Capabilities::FUZZEL)
            .union(Capabilities::FORK)
            .union(Capabilities::NIRI_ACTIVITIES),
        dispatch: verbs::assign_workspace::run,
    },
    Verb {
        name: "save-activity",
        label: "Save activity",
        category: Category::Activity,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET
            .union(Capabilities::FORK)
            .union(Capabilities::NIRI_ACTIVITIES),
        dispatch: verbs::save_activity::run,
    },
    Verb {
        name: "list-activities",
        label: "List activities",
        category: Category::Activity,
        menu_visible: false,
        requires: Capabilities::NIRI_SOCKET
            .union(Capabilities::FORK)
            .union(Capabilities::NIRI_ACTIVITIES),
        dispatch: verbs::list_activities::run,
    },
    Verb {
        name: "create-activity",
        label: "Create activity",
        category: Category::Activity,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET
            .union(Capabilities::FUZZEL)
            .union(Capabilities::FORK)
            .union(Capabilities::NIRI_ACTIVITIES),
        dispatch: verbs::create_activity::run,
    },
    Verb {
        name: "remove-activity",
        label: "Remove activity",
        category: Category::Activity,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET
            .union(Capabilities::FUZZEL)
            .union(Capabilities::FORK)
            .union(Capabilities::NIRI_ACTIVITIES),
        dispatch: verbs::remove_activity::run,
    },
    Verb {
        name: "rename-activity",
        label: "Rename activity",
        category: Category::Activity,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET
            .union(Capabilities::FUZZEL)
            .union(Capabilities::FORK)
            .union(Capabilities::NIRI_ACTIVITIES),
        dispatch: verbs::rename_activity::run,
    },
    Verb {
        name: "reload-config",
        label: "Reload config",
        category: Category::System,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET,
        dispatch: verbs::reload_config::run,
    },
    Verb {
        name: "power-on-monitors",
        label: "Power on monitors",
        category: Category::System,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET,
        dispatch: verbs::power_on_monitors::run,
    },
    Verb {
        name: "unset-workspace-name",
        label: "Unset workspace name",
        category: Category::Workspace,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET,
        dispatch: verbs::unset_workspace_name::run,
    },
    Verb {
        name: "rename-workspace",
        label: "Rename workspace",
        category: Category::Workspace,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET.union(Capabilities::FUZZEL),
        dispatch: verbs::rename_workspace::run,
    },
    Verb {
        name: "list-workspaces",
        label: "List workspaces",
        category: Category::Workspace,
        menu_visible: false,
        requires: Capabilities::NIRI_SOCKET,
        dispatch: verbs::list_workspaces::run,
    },
    Verb {
        name: "pick-window",
        label: "Pick window",
        category: Category::Window,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET,
        dispatch: verbs::pick_window::run,
    },
    Verb {
        name: "bookmark",
        label: "Jump to bookmark",
        category: Category::Window,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET
            .union(Capabilities::FUZZEL)
            .union(Capabilities::FORK),
        dispatch: verbs::bookmark::run,
    },
    Verb {
        name: "bookmark-remove",
        label: "Remove bookmark",
        category: Category::Window,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET
            .union(Capabilities::FUZZEL)
            .union(Capabilities::FORK),
        dispatch: verbs::bookmark_remove::run,
    },
    Verb {
        name: "bookmark-move",
        label: "Move bookmark",
        category: Category::Window,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET
            .union(Capabilities::FUZZEL)
            .union(Capabilities::FORK),
        dispatch: verbs::bookmark_move::run,
    },
    Verb {
        name: "bookmark-assign-key",
        label: "Assign bookmark key",
        category: Category::Window,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET
            .union(Capabilities::FUZZEL)
            .union(Capabilities::FORK),
        dispatch: verbs::bookmark_assign_key::run,
    },
    Verb {
        name: "bookmark-unassign-key",
        label: "Unassign bookmark key",
        category: Category::Window,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET
            .union(Capabilities::FUZZEL)
            .union(Capabilities::FORK),
        dispatch: verbs::bookmark_unassign_key::run,
    },
    // ---- Monitor verbs ----
    Verb {
        name: "focus-monitor",
        label: "Focus monitor",
        category: Category::Monitor,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET.union(Capabilities::FUZZEL),
        dispatch: verbs::focus_monitor::run,
    },
    Verb {
        name: "move-window-to-monitor",
        label: "Move window to monitor",
        category: Category::Monitor,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET.union(Capabilities::FUZZEL),
        dispatch: verbs::move_window_to_monitor::run,
    },
    Verb {
        name: "move-column-to-monitor",
        label: "Move column to monitor",
        category: Category::Monitor,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET.union(Capabilities::FUZZEL),
        dispatch: verbs::move_column_to_monitor::run,
    },
    Verb {
        name: "move-workspace-to-monitor",
        label: "Move workspace to monitor",
        category: Category::Monitor,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET.union(Capabilities::FUZZEL),
        dispatch: verbs::move_workspace_to_monitor::run,
    },
    Verb {
        name: "pick-color",
        label: "Pick color",
        category: Category::System,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET,
        dispatch: verbs::pick_color::run,
    },
    Verb {
        name: "quit",
        label: "Quit jiji",
        category: Category::System,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET.union(Capabilities::FUZZEL),
        dispatch: verbs::quit::run,
    },
    Verb {
        name: "power-off-monitors",
        label: "Power off monitors",
        category: Category::System,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET.union(Capabilities::FUZZEL),
        dispatch: verbs::power_off_monitors::run,
    },
    Verb {
        name: "stop-cast",
        label: "Stop screencast",
        category: Category::System,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET.union(Capabilities::FUZZEL),
        dispatch: verbs::stop_cast::run,
    },
    Verb {
        name: "add-workspace-up",
        label: "Add workspace up",
        category: Category::Workspace,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET.union(Capabilities::FORK),
        dispatch: verbs::add_workspace_up::run,
    },
    Verb {
        name: "add-workspace-down",
        label: "Add workspace down",
        category: Category::Workspace,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET.union(Capabilities::FORK),
        dispatch: verbs::add_workspace_down::run,
    },
    Verb {
        name: "move-window-to-new-workspace-up",
        label: "Move window to new workspace up",
        category: Category::Workspace,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET.union(Capabilities::FORK),
        dispatch: verbs::move_window_to_new_workspace_up::run,
    },
    Verb {
        name: "move-window-to-new-workspace-down",
        label: "Move window to new workspace down",
        category: Category::Workspace,
        menu_visible: true,
        requires: Capabilities::NIRI_SOCKET.union(Capabilities::FORK),
        dispatch: verbs::move_window_to_new_workspace_down::run,
    },
];

/// Verbs whose required capabilities are all present, sorted by [`Category`]
/// declaration order. Intra-category registration order is preserved (stable
/// sort).
pub fn enabled(caps: Capabilities) -> Vec<&'static Verb> {
    let mut out: Vec<&'static Verb> = REGISTRY.iter().filter(|v| v.is_enabled(caps)).collect();
    out.sort_by_key(|v| v.category);
    out
}

/// Look up a verb by its CLI name.
pub fn find(name: &str) -> Option<&'static Verb> {
    REGISTRY.iter().find(|v| v.name == name)
}

/// Used by the menu render path only; `enabled()` and `find()` continue to
/// surface menu-hidden verbs so `--debug` and direct CLI dispatch remain
/// unaffected.
pub fn enabled_for_menu(caps: Capabilities) -> Vec<&'static Verb> {
    enabled(caps)
        .into_iter()
        .filter(|v| v.menu_visible)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_filters_by_capability() {
        // NIRI_SOCKET + FUZZEL: the full set of verbs that require at most
        // NIRI_SOCKET (plus switch-workspace which additionally needs FUZZEL).
        // Verbs needing FORK remain excluded — including switch-workspace-all
        // and all five bookmark verbs (NIRI_SOCKET + FUZZEL + FORK), and all
        // NIRI_ACTIVITIES verbs.
        // Category order: Workspace, Window, Monitor, Mode, Activity, System.
        let caps = Capabilities::NIRI_SOCKET | Capabilities::FUZZEL;
        let names: Vec<_> = enabled(caps).iter().map(|v| v.name).collect();
        assert_eq!(
            names,
            vec![
                "switch-workspace",
                "focus-workspace-previous",
                "unset-workspace-name",
                "rename-workspace",
                "list-workspaces",
                "pick-window",
                "focus-monitor",
                "move-window-to-monitor",
                "move-column-to-monitor",
                "move-workspace-to-monitor",
                "toggle-debug-tint",
                "reload-config",
                "power-on-monitors",
                "pick-color",
                "quit",
                "power-off-monitors",
                "stop-cast",
            ]
        );
    }

    #[test]
    fn full_capabilities_enable_all() {
        let caps = Capabilities::all();
        assert_eq!(enabled(caps).len(), REGISTRY.len());
    }

    #[test]
    fn empty_capabilities_enable_none() {
        assert!(enabled(Capabilities::empty()).is_empty());
    }

    #[test]
    fn find_resolves_known_verb() {
        assert!(find("switch-activity").is_some());
        assert!(find("nope").is_none());
    }

    /// Pin the behavioral order-preservation contract that `enabled()` relies on:
    /// same-category verbs registered in reverse order must emerge in that same
    /// reverse order after sorting by category. The current implementation gets
    /// this from `sort_by_key`'s stability guarantee. (Note: this test alone
    /// does not discriminate `sort_unstable_by_key` from `sort_by_key`, because
    /// pdqsort's insertion-sort fallback handles slices ≤20 elements stably in
    /// practice — the discriminator would need >20 same-key inputs. The
    /// behavioral invariant this test pins is still the load-bearing one.)
    #[test]
    fn sort_by_key_preserves_intra_category_registration_order() {
        fn noop_dispatch(_: &crate::snapshot::Snapshot, _: &VerbArgs) -> anyhow::Result<()> {
            Ok(())
        }
        // Four Workspace-category verbs declared in a deliberate order (D, C, B, A).
        // After a category sort the order must be preserved: D, C, B, A.
        static V_D: Verb = Verb {
            name: "d",
            label: "D",
            category: Category::Workspace,
            menu_visible: true,
            requires: Capabilities::empty(),
            dispatch: noop_dispatch,
        };
        static V_C: Verb = Verb {
            name: "c",
            label: "C",
            category: Category::Workspace,
            menu_visible: true,
            requires: Capabilities::empty(),
            dispatch: noop_dispatch,
        };
        static V_B: Verb = Verb {
            name: "b",
            label: "B",
            category: Category::Workspace,
            menu_visible: true,
            requires: Capabilities::empty(),
            dispatch: noop_dispatch,
        };
        static V_A: Verb = Verb {
            name: "a",
            label: "A",
            category: Category::Workspace,
            menu_visible: true,
            requires: Capabilities::empty(),
            dispatch: noop_dispatch,
        };
        let mut verbs: Vec<&Verb> = vec![&V_D, &V_C, &V_B, &V_A];
        verbs.sort_by_key(|v| v.category);
        let names: Vec<&str> = verbs.iter().map(|v| v.name).collect();
        assert_eq!(
            names,
            vec!["d", "c", "b", "a"],
            "stable sort must preserve registration order within a category"
        );
    }

    /// Pin the `Category` variant declaration order directly so a future enum
    /// reorder fails at the enum definition (the cause), not just at the
    /// registry name-vector assertions (the consequence).
    #[test]
    fn category_enum_declaration_order() {
        assert!(Category::Workspace < Category::Window);
        assert!(Category::Window < Category::Monitor);
        assert!(Category::Monitor < Category::Mode);
        assert!(Category::Mode < Category::Activity);
        assert!(Category::Activity < Category::System);
    }

    #[test]
    fn category_declaration_order_governs_enabled_sort() {
        let all = enabled(Capabilities::all());
        let names: Vec<&str> = all.iter().map(|v| v.name).collect();

        // Workspace verbs come first, intra-category registration order preserved.
        let sw_pos = names.iter().position(|&n| n == "switch-workspace").unwrap();
        let fwp_pos = names
            .iter()
            .position(|&n| n == "focus-workspace-previous")
            .unwrap();
        let pw_pos = names.iter().position(|&n| n == "pick-window").unwrap();
        let fm_pos = names.iter().position(|&n| n == "focus-monitor").unwrap();
        let tdt_pos = names
            .iter()
            .position(|&n| n == "toggle-debug-tint")
            .unwrap();
        let sa_pos = names.iter().position(|&n| n == "switch-activity").unwrap();
        let rc_pos = names.iter().position(|&n| n == "reload-config").unwrap();

        // Workspace group precedes Window group.
        assert!(
            sw_pos < fwp_pos,
            "switch-workspace must precede focus-workspace-previous"
        );
        // Window group precedes Monitor group.
        assert!(
            pw_pos < fm_pos,
            "pick-window (Window) must precede focus-monitor (Monitor)"
        );
        // Monitor group precedes Mode group.
        assert!(
            fm_pos < tdt_pos,
            "focus-monitor (Monitor) must precede toggle-debug-tint (Mode)"
        );
        // Mode group precedes Activity group.
        assert!(
            tdt_pos < sa_pos,
            "toggle-debug-tint must precede switch-activity"
        );
        // Activity group precedes System group.
        assert!(
            sa_pos < rc_pos,
            "switch-activity must precede reload-config (System)"
        );
        // Confirm exact order.
        assert_eq!(
            names,
            vec![
                "switch-workspace",
                "switch-workspace-all",
                "focus-workspace-previous",
                "unset-workspace-name",
                "rename-workspace",
                "list-workspaces",
                "add-workspace-up",
                "add-workspace-down",
                "move-window-to-new-workspace-up",
                "move-window-to-new-workspace-down",
                "pick-window",
                "bookmark",
                "bookmark-remove",
                "bookmark-move",
                "bookmark-assign-key",
                "bookmark-unassign-key",
                "focus-monitor",
                "move-window-to-monitor",
                "move-column-to-monitor",
                "move-workspace-to-monitor",
                "toggle-debug-tint",
                "switch-activity",
                "switch-activity-previous",
                "move-window-to-activity",
                "move-window-here",
                "move-workspace-to-activity",
                "assign-workspace",
                "save-activity",
                "list-activities",
                "create-activity",
                "remove-activity",
                "rename-activity",
                "reload-config",
                "power-on-monitors",
                "pick-color",
                "quit",
                "power-off-monitors",
                "stop-cast",
            ]
        );
    }

    #[test]
    fn enabled_with_full_activities_capabilities_includes_all_passthrough_verbs() {
        let caps = Capabilities::all();
        let names: Vec<&str> = enabled(caps).iter().map(|v| v.name).collect();
        // All Activity-category verbs must appear in the full-capabilities enabled set.
        assert!(
            names.contains(&"switch-activity"),
            "switch-activity missing from full-caps enabled set"
        );
        assert!(
            names.contains(&"switch-activity-previous"),
            "switch-activity-previous missing from full-caps enabled set"
        );
        assert!(
            names.contains(&"move-window-to-activity"),
            "move-window-to-activity missing from full-caps enabled set"
        );
        assert!(
            names.contains(&"move-window-here"),
            "move-window-here missing from full-caps enabled set"
        );
        assert!(
            names.contains(&"move-workspace-to-activity"),
            "move-workspace-to-activity missing from full-caps enabled set"
        );
        assert!(
            names.contains(&"assign-workspace"),
            "assign-workspace missing from full-caps enabled set"
        );
        assert!(
            names.contains(&"save-activity"),
            "save-activity missing from full-caps enabled set"
        );
        assert!(
            names.contains(&"list-activities"),
            "list-activities missing from full-caps enabled set"
        );
        assert!(
            names.contains(&"create-activity"),
            "create-activity missing from full-caps enabled set"
        );
        assert!(
            names.contains(&"remove-activity"),
            "remove-activity missing from full-caps enabled set"
        );
        assert!(
            names.contains(&"rename-activity"),
            "rename-activity missing from full-caps enabled set"
        );
    }

    #[test]
    fn enabled_for_menu_hides_menu_hidden_verbs() {
        let caps = Capabilities::all();
        let menu_names: Vec<&str> = enabled_for_menu(caps).iter().map(|v| v.name).collect();
        let all_names: Vec<&str> = enabled(caps).iter().map(|v| v.name).collect();
        // menu-hidden verb must not appear in the menu set.
        assert!(
            !menu_names.contains(&"list-activities"),
            "list-activities must not appear in the menu (menu_visible=false)"
        );
        // but a normal verb must still be in the menu set.
        assert!(
            menu_names.contains(&"save-activity"),
            "save-activity must appear in the menu (menu_visible=true)"
        );
        // enabled() must still surface the menu-hidden verb for --debug and direct dispatch.
        assert!(
            all_names.contains(&"list-activities"),
            "list-activities must still be in enabled() for --debug and direct dispatch"
        );
    }

    /// Bidirectional set-equality between the clap-derived subcommand surface
    /// and `REGISTRY`: every clap subcommand name matches a registry entry
    /// and vice versa. This is the load-bearing guard against
    /// enum↔registry drift, now derived from the clap surface itself instead
    /// of a hand-maintained list — a new verb variant is auto-covered with
    /// zero additional bookkeeping.
    ///
    /// `completions` is filtered out: it's a meta subcommand whose
    /// `Cmd::verb_name()` returns `None` and which has no registry entry by
    /// design. The global `--debug` flag never appears in
    /// `get_subcommands()` since it isn't a subcommand.
    ///
    /// Empirically probed (clap 4.6.1): the *unbuilt* `Command` returned by
    /// `CommandFactory::command()` lists exactly the derive-declared
    /// subcommand variants. Clap's auto `help` subcommand is injected only
    /// during `Command::build()` (which parsing triggers internally, but
    /// `command()` alone does not build). Do not call `.build()` or trigger
    /// parsing on the introspected `Command` here — if a future clap upgrade
    /// changes this and `help` leaks into the unbuilt set, this test's
    /// sorted-vec diff fails loudly showing `help`, and the fix is to extend
    /// the filter above.
    #[test]
    fn clap_subcommands_match_registry_names() {
        use clap::CommandFactory;

        // Must bind to a local: `Cli::command().get_subcommands()` alone
        // fails to compile (E0716, temporary dropped while borrowed).
        let cmd = crate::cli::Cli::command();
        let mut clap_names: Vec<&str> = cmd
            .get_subcommands()
            .map(|c| c.get_name())
            .filter(|&n| n != "completions")
            .collect();
        clap_names.sort_unstable();

        let mut registry_names: Vec<&str> = REGISTRY.iter().map(|v| v.name).collect();
        registry_names.sort_unstable();

        assert_eq!(
            clap_names, registry_names,
            "clap subcommand surface and REGISTRY names must match exactly \
             (excluding the `completions` meta subcommand)"
        );
    }

    /// Pins `Cmd::verb_name()` arm-correctness for every registry verb by
    /// roundtripping through the real clap parser: parse the bare verb name,
    /// then check the resulting variant's `verb_name()` reports that same
    /// name back. A misrouted arm that returns a *different* valid registry
    /// name (e.g. a copy-paste error where parsing `"pick-window"` yields
    /// `verb_name() == Some("pick-color")`) is caught by this equality; the
    /// set-based test above cannot catch it, since both names are already
    /// registry members.
    ///
    /// Every registry verb must be bare-invocable because menu dispatch
    /// calls each verb with `VerbArgs::default()` — every registry verb
    /// parses with no arguments at all, since every field is omittable
    /// (optional, or defaulted, like a plain `bool` flag). If a future verb
    /// gains a required positional that is direct-CLI only, this test fails
    /// loudly at parse; the fix is a per-verb argv exception for that verb,
    /// not weakening the assertion.
    #[test]
    fn verb_name_roundtrips_for_every_registry_verb() {
        use clap::Parser;

        for verb in REGISTRY {
            let cli = crate::cli::Cli::try_parse_from(["jiji-do", verb.name])
                .unwrap_or_else(|e| panic!("registry verb '{}' must parse bare: {e}", verb.name));
            let cmd = cli
                .cmd
                .unwrap_or_else(|| panic!("parse of '{}' produced no subcommand", verb.name));
            assert_eq!(
                cmd.verb_name(),
                Some(verb.name),
                "verb_name() arm misroute: parsing '{}' must report itself",
                verb.name
            );
        }

        // Pin the one arm a registry-name parse cannot reach: the meta
        // `completions` subcommand must not map to any registry verb.
        assert_eq!(
            crate::cli::Cmd::Completions {
                shell: clap_complete::Shell::Fish
            }
            .verb_name(),
            None,
            "Completions is a meta subcommand and must not map to a registry verb"
        );
    }
}
