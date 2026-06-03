//! The launch-time snapshot of focused state. Captured ONCE at process start,
//! before any picker opens — a picker steals focus, so a later read would see
//! the picker, not the user's context.

use serde::Deserialize;

/// Minimal projection of `niri msg --json windows` — only the fields we read.
#[derive(Deserialize)]
struct WindowBrief {
    id: u64,
    is_focused: bool,
}

/// Minimal projection of `niri msg --json workspaces`.
#[derive(Deserialize)]
struct WorkspaceBrief {
    id: u64,
    output: Option<String>,
    is_focused: bool,
}

/// Minimal projection of `niri msg --json activities` (fork-only).
#[derive(Deserialize)]
struct ActivityBrief {
    name: String,
    is_active: bool,
}

/// Focused context captured at launch. Every field is `None` when nothing
/// matches (no focus, or — for `focused_activity` — upstream niri).
#[derive(Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub focused_window: Option<u64>,
    pub focused_workspace: Option<u64>,
    pub focused_output: Option<String>,
    pub focused_activity: Option<String>,
}

impl Snapshot {
    /// Build from raw JSON strings. `activities_json` is `None` on upstream
    /// niri (no fork). Pure — no subprocess; this is the unit-tested seam.
    pub fn from_json(
        windows_json: &str,
        workspaces_json: &str,
        activities_json: Option<&str>,
    ) -> anyhow::Result<Self> {
        let windows: Vec<WindowBrief> = serde_json::from_str(windows_json)?;
        let workspaces: Vec<WorkspaceBrief> = serde_json::from_str(workspaces_json)?;
        let focused_window = windows.into_iter().find(|w| w.is_focused).map(|w| w.id);
        let focused_ws = workspaces.into_iter().find(|w| w.is_focused);
        let focused_workspace = focused_ws.as_ref().map(|w| w.id);
        let focused_output = focused_ws.and_then(|w| w.output);
        let focused_activity = match activities_json {
            Some(j) => {
                let acts: Vec<ActivityBrief> = serde_json::from_str(j)?;
                acts.into_iter().find(|a| a.is_active).map(|a| a.name)
            }
            None => None,
        };
        Ok(Snapshot {
            focused_window,
            focused_workspace,
            focused_output,
            focused_activity,
        })
    }

    /// Capture from the live compositor. Calls `niri msg --json` for windows,
    /// workspaces, and (only when `FORK` is set) activities, then delegates to
    /// [`Snapshot::from_json`]. Must be called before any picker opens.
    pub fn capture(caps: crate::capabilities::Capabilities) -> anyhow::Result<Self> {
        use crate::capabilities::Capabilities;
        let windows = crate::proc::run_capture("niri", &["msg", "--json", "windows"])?;
        let workspaces = crate::proc::run_capture("niri", &["msg", "--json", "workspaces"])?;
        let activities = if caps.contains(Capabilities::FORK) {
            Some(crate::proc::run_capture(
                "niri",
                &["msg", "--json", "activities"],
            )?)
        } else {
            None
        };
        Snapshot::from_json(&windows, &workspaces, activities.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOWS: &str = r#"[
        {"id":10,"is_focused":false},
        {"id":11,"is_focused":true}
    ]"#;
    const WORKSPACES: &str = r#"[
        {"id":20,"output":"DP-1","is_focused":false},
        {"id":21,"output":"DP-2","is_focused":true}
    ]"#;
    const ACTIVITIES: &str = r#"[
        {"name":"default","is_active":false},
        {"name":"acme","is_active":true}
    ]"#;

    #[test]
    fn from_json_extracts_focused_fields() {
        let s = Snapshot::from_json(WINDOWS, WORKSPACES, Some(ACTIVITIES)).unwrap();
        assert_eq!(s.focused_window, Some(11));
        assert_eq!(s.focused_workspace, Some(21));
        assert_eq!(s.focused_output.as_deref(), Some("DP-2"));
        assert_eq!(s.focused_activity.as_deref(), Some("acme"));
    }

    #[test]
    fn from_json_upstream_has_no_activity() {
        let s = Snapshot::from_json(WINDOWS, WORKSPACES, None).unwrap();
        assert_eq!(s.focused_activity, None);
        assert_eq!(s.focused_window, Some(11));
    }

    #[test]
    fn from_json_nothing_focused_is_all_none() {
        let none_focused = r#"[{"id":1,"is_focused":false}]"#;
        let s = Snapshot::from_json(none_focused, none_focused_ws(), None).unwrap();
        assert_eq!(s.focused_window, None);
        assert_eq!(s.focused_workspace, None);
        assert_eq!(s.focused_output, None);
    }

    fn none_focused_ws() -> &'static str {
        r#"[{"id":1,"output":"DP-1","is_focused":false}]"#
    }
}
