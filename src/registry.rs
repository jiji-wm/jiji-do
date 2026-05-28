//! The static verb registry and capability-based filtering. The registry is
//! the single source of truth for which verbs exist.

use crate::capabilities::Capabilities;
use crate::snapshot::Snapshot;
use crate::verbs;

/// Menu grouping. Declaration order is the sort order used by [`enabled`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    Workspace,
    // `#[allow(dead_code)]` suppresses the unused-variant lint until the first
    // Window-category verb is registered.
    #[allow(dead_code)]
    Window,
    Mode,
    Activity,
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
    /// the optional positional CLI arg (e.g. the name for `create-activity
    /// <name>`); `None` for menu invocation or when the verb takes no
    /// positional.
    pub dispatch: fn(&Snapshot, Option<&str>) -> anyhow::Result<()>,
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
        // NIRI_SOCKET + FUZZEL: switch-workspace (needs both), focus-workspace-previous
        // (needs NIRI_SOCKET only), toggle-debug-tint (needs NIRI_SOCKET only).
        // switch-activity is still excluded (needs FORK + NIRI_ACTIVITIES too).
        let caps = Capabilities::NIRI_SOCKET | Capabilities::FUZZEL;
        let names: Vec<_> = enabled(caps).iter().map(|v| v.name).collect();
        assert_eq!(
            names,
            vec![
                "switch-workspace",
                "focus-workspace-previous",
                "toggle-debug-tint"
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
        fn noop_dispatch(_: &crate::snapshot::Snapshot, _: Option<&str>) -> anyhow::Result<()> {
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

    #[test]
    fn category_grouped_ordering_pins_workspace_before_mode_regardless_of_registration_order() {
        let all = enabled(Capabilities::all());
        let names: Vec<&str> = all.iter().map(|v| v.name).collect();

        // Both Workspace verbs come first, intra-category registration order preserved.
        let sw_pos = names.iter().position(|&n| n == "switch-workspace").unwrap();
        let fwp_pos = names
            .iter()
            .position(|&n| n == "focus-workspace-previous")
            .unwrap();
        let tdt_pos = names
            .iter()
            .position(|&n| n == "toggle-debug-tint")
            .unwrap();
        let sa_pos = names.iter().position(|&n| n == "switch-activity").unwrap();

        // Workspace group: switch-workspace before focus-workspace-previous.
        assert!(
            sw_pos < fwp_pos,
            "switch-workspace must precede focus-workspace-previous"
        );
        // toggle-debug-tint (Mode) comes after both Workspace verbs.
        assert!(
            fwp_pos < tdt_pos,
            "focus-workspace-previous must precede toggle-debug-tint"
        );
        // switch-activity (Activity) comes last.
        assert!(
            tdt_pos < sa_pos,
            "toggle-debug-tint must precede switch-activity"
        );
        // Confirm exact order.
        assert_eq!(
            names,
            vec![
                "switch-workspace",
                "focus-workspace-previous",
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
            ]
        );
    }

    #[test]
    fn enabled_with_full_activities_capabilities_includes_all_passthrough_verbs() {
        let caps = Capabilities::all();
        let names: Vec<&str> = enabled(caps).iter().map(|v| v.name).collect();
        // All six Activity-category verbs must be present.
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
}
