//! The static verb registry and capability-based filtering. The registry is
//! the single source of truth for which verbs exist.

use crate::capabilities::Capabilities;
use crate::snapshot::Snapshot;
use crate::verbs;

/// Menu grouping. Defined now; only USED for ordering in Stage 2 (Stage 1
/// renders in registration order).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Workspace,
    Activity,
}

/// A launcher verb: its CLI name, menu label, category, the capabilities it
/// requires, and the dispatch fn (which consumes the launch snapshot).
pub struct Verb {
    pub name: &'static str,
    pub label: &'static str,
    pub category: Category,
    pub requires: Capabilities,
    pub dispatch: fn(&Snapshot) -> anyhow::Result<()>,
}

impl Verb {
    /// True iff every required capability is present.
    pub fn is_enabled(&self, caps: Capabilities) -> bool {
        caps.contains(self.requires)
    }
}

/// The complete registry. Order here is the menu order (Stage 1).
pub static REGISTRY: &[Verb] = &[
    Verb {
        name: "switch-workspace",
        label: "Switch workspace",
        category: Category::Workspace,
        requires: Capabilities::NIRI_SOCKET.union(Capabilities::FUZZEL),
        dispatch: verbs::switch_workspace::run,
    },
    Verb {
        name: "switch-activity",
        label: "Switch activity",
        category: Category::Activity,
        requires: Capabilities::NIRI_SOCKET
            .union(Capabilities::FUZZEL)
            .union(Capabilities::FORK)
            .union(Capabilities::NIRI_ACTIVITIES),
        dispatch: verbs::switch_activity::run,
    },
];

/// Verbs whose required capabilities are all present, in registration order.
pub fn enabled(caps: Capabilities) -> Vec<&'static Verb> {
    REGISTRY.iter().filter(|v| v.is_enabled(caps)).collect()
}

/// Look up a verb by its CLI name.
pub fn find(name: &str) -> Option<&'static Verb> {
    REGISTRY.iter().find(|v| v.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_filters_by_capability() {
        // Only switch-workspace's two flags present → only it is enabled.
        let caps = Capabilities::NIRI_SOCKET | Capabilities::FUZZEL;
        let names: Vec<_> = enabled(caps).iter().map(|v| v.name).collect();
        assert_eq!(names, vec!["switch-workspace"]);
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
}
