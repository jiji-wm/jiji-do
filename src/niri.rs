//! Helpers for the `niri msg` subprocess surface used by native verbs.

use anyhow::Context;
use serde::Deserialize;

#[derive(Deserialize)]
struct WorkspaceRow {
    id: u64,
    /// Per-monitor index. Present on jiji; absent on vanilla niri (older
    /// compositors that predate the field default to 0 via `#[serde(default)]`).
    /// On such payloads all unnamed workspace rows collapse to reference `"0"`.
    /// niri indices are 1-based, so `"0"` would fail loudly at the compositor
    /// rather than silently mis-targeting a workspace.
    #[serde(default)]
    idx: u8,
    name: Option<String>,
    output: Option<String>,
    /// Present only on jiji. `None` (vanilla niri) means "no activity
    /// concept" — include everything.
    #[serde(default)]
    is_in_active_activity: Option<bool>,
}

/// A named wrapper for an activity name — compositor-payload-derived or
/// user-typed — used as a transposition guard.
///
/// This newtype makes the first parameter of [`focus_workspace_in_activity`]
/// type-distinct from the adjacent workspace-reference `&str`, so the compiler
/// rejects a transposition of the two arguments. The payload may be derived
/// from compositor JSON or from a user-supplied positional argument.
/// Construct at the CLI or verb boundary via [`ActivityName::new`].
pub struct ActivityName(String);

impl ActivityName {
    /// Wrap an activity name obtained at the CLI or verb dispatch boundary.
    pub fn new(name: impl Into<String>) -> Self {
        ActivityName(name.into())
    }

    /// The activity name as a `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A workspace reference that came directly from user-typed CLI input.
///
/// Constructible only via [`UserWorkspaceRef::from_cli`], which is intended
/// to be called solely at the CLI/verb boundary where argv values are
/// extracted. The constructor accepts any `Into<String>` — the guard is
/// intent-signalling rather than structural enforcement — but it discourages
/// programmatic callers from accidentally routing a computed reference
/// (e.g. `"id:N"`) through [`focus_workspace_typed`], bypassing the
/// [`FocusReference`] invariant that guards the standard programmatic lane.
/// Contrast with [`FocusReference`], which is only obtainable through
/// [`WorkspaceChoice::focus_reference`] and enforces the mapping
/// structurally.
///
/// For programmatically constructed references use [`focus_workspace`] with
/// a [`FocusReference`] obtained from [`WorkspaceChoice::focus_reference`].
pub struct UserWorkspaceRef(String);

impl UserWorkspaceRef {
    /// Wrap a workspace reference that was read from CLI argv (user input).
    ///
    /// Call this only at the CLI/verb dispatch layer where the value
    /// originates from user-provided positional arguments.
    pub fn from_cli(s: impl Into<String>) -> Self {
        UserWorkspaceRef(s.into())
    }

    /// The reference string as a `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A dispatchable `focus-workspace` positional argument — either a workspace
/// name (globally addressable across monitors) or a per-monitor index as a
/// decimal string.
///
/// The wrapped string is deliberately **not** the workspace's stable unique
/// `id` (`u64`), because the compositor parses a bare positional as a
/// per-monitor `u8` index. Passing the `id` value would silently mis-target
/// a workspace at that index position rather than the intended workspace.
/// The only way to obtain a `FocusReference` is through
/// [`WorkspaceChoice::focus_reference`], which enforces this mapping.
///
/// For user-typed references, use [`focus_workspace_typed`] instead — that
/// lane forwards input verbatim and deliberately bypasses this type.
pub struct FocusReference(String);

impl FocusReference {
    /// The string to pass as the `focus-workspace` positional argument.
    pub fn as_arg(&self) -> &str {
        &self.0
    }
}

/// A workspace as offered in the switch picker.
#[derive(Debug, PartialEq, Eq)]
pub struct WorkspaceChoice {
    pub id: u64,
    pub idx: u8,
    pub name: Option<String>,
    pub label: String,
}

impl WorkspaceChoice {
    /// The `niri msg action focus-workspace` positional: the name when set
    /// (globally addressable), else the per-monitor index as a string.
    /// Known edge: an unnamed workspace on a non-focused monitor dispatches
    /// by index against the active monitor. Known edge on legacy payloads:
    /// when the compositor omits `idx` (serde default 0), all unnamed rows
    /// collapse to reference `"0"`. niri indices are 1-based, so `"0"` fails
    /// loudly rather than silently mis-targeting another workspace.
    pub fn focus_reference(&self) -> FocusReference {
        let s = match &self.name {
            Some(name) => name.clone(),
            None => self.idx.to_string(),
        };
        FocusReference(s)
    }
}

/// Parse `niri msg --json workspaces` into picker choices for the
/// current-activity picker. Rows from dormant activities are dropped;
/// vanilla niri (no activity fields) lists everything. Label is the
/// workspace name when set, else `"<output> #<id>"`. Pure (unit-tested).
pub fn parse_workspace_choices(json: &str) -> anyhow::Result<Vec<WorkspaceChoice>> {
    let rows: Vec<WorkspaceRow> = serde_json::from_str(json)?;
    Ok(rows
        .into_iter()
        .filter(|r| r.is_in_active_activity != Some(false))
        .map(|r| {
            let label = r
                .name
                .clone()
                .unwrap_or_else(|| format!("{} #{}", r.output.as_deref().unwrap_or("?"), r.id));
            WorkspaceChoice {
                id: r.id,
                idx: r.idx,
                name: r.name,
                label,
            }
        })
        .collect())
}

/// Fetch the workspace choices live.
pub fn workspace_choices() -> anyhow::Result<Vec<WorkspaceChoice>> {
    let json = crate::proc::run_capture(crate::proc::msg_bin(), &["msg", "--json", "workspaces"])?;
    parse_workspace_choices(&json)
}

/// Names of the current-activity workspaces (named workspaces only —
/// unnamed ones have no typeable reference to offer), in inventory order.
/// Candidates source for shell completion; scope matches the
/// `switch-workspace` picker. Pure (unit-tested).
pub fn parse_workspace_names(json: &str) -> anyhow::Result<Vec<String>> {
    Ok(parse_workspace_choices(json)?
        .into_iter()
        .filter_map(|c| c.name)
        .collect())
}

/// Fetch current-activity workspace names live.
pub fn workspace_names() -> anyhow::Result<Vec<String>> {
    let json = crate::proc::run_capture(crate::proc::msg_bin(), &["msg", "--json", "workspaces"])?;
    parse_workspace_names(&json)
}

/// Names of `activity`'s workspaces (named only), in inventory order.
/// Errors when `activity` is not in the activities payload — callers
/// surface that as exit 1. Pure (unit-tested).
pub fn parse_workspace_names_in_activity(
    workspaces_json: &str,
    activities_json: &str,
    activity: &str,
) -> anyhow::Result<Vec<String>> {
    #[derive(Deserialize)]
    struct Row {
        name: Option<String>,
        #[serde(default)]
        activities: Vec<u64>,
    }
    let act_rows: Vec<ActivityRow> =
        serde_json::from_str(activities_json).context("parsing activities JSON")?;
    let act = act_rows
        .iter()
        .find(|a| a.name == activity)
        .ok_or_else(|| anyhow::anyhow!("unknown activity: {activity}"))?;
    let ws_rows: Vec<Row> =
        serde_json::from_str(workspaces_json).context("parsing workspaces JSON")?;
    Ok(ws_rows
        .into_iter()
        .filter(|w| w.activities.contains(&act.id))
        .filter_map(|w| w.name)
        .collect())
}

/// Fetch `activity`'s workspace names live. Reads the activities payload
/// (jiji-only request) — on vanilla niri the subprocess fails and the error
/// propagates with the compositor's own message.
pub fn workspace_names_in_activity(activity: &str) -> anyhow::Result<Vec<String>> {
    let workspaces =
        crate::proc::run_capture(crate::proc::msg_bin(), &["msg", "--json", "workspaces"])?;
    let activities =
        crate::proc::run_capture(crate::proc::msg_bin(), &["msg", "--json", "activities"])?;
    parse_workspace_names_in_activity(&workspaces, &activities, activity)
}

#[derive(Deserialize)]
struct ActivityRow {
    /// Stable activity id. Present on jiji; absent on older compositors
    /// that predate the field — defaults to 0 via `#[serde(default)]` so
    /// existing `parse_activity_names_mru` fixtures without the field remain
    /// parseable.
    #[serde(default)]
    id: u64,
    name: String,
    #[serde(default)]
    is_active: bool,
    #[serde(default)]
    last_active_seq: Option<u64>,
}

/// Parse `niri msg --json activities` into picker rows in most-recently-used
/// order, so the first fuzzel row (preselected) is the activity the user is
/// most likely to target. Pure (unit-tested).
///
/// The compositor exposes `last_active_seq` (monotonic activation counter;
/// the active activity holds the unique maximum) but serves the array itself
/// in inventory order — MRU is the client's job. Sort key: `last_active_seq`
/// descending when present; on older compositors that predate the field,
/// `is_active` (1/0) substitutes, putting the current activity first and
/// keeping the rest in inventory order (the sort is stable).
pub fn parse_activity_names_mru(json: &str) -> anyhow::Result<Vec<String>> {
    let mut rows: Vec<ActivityRow> =
        serde_json::from_str(json).context("parsing activities JSON")?;
    rows.sort_by_key(|r| std::cmp::Reverse(r.last_active_seq.unwrap_or(r.is_active as u64)));
    Ok(rows.into_iter().map(|r| r.name).collect())
}

/// Fetch the activity names live, MRU-ordered. Read at dispatch time (not
/// from `Snapshot` — activities state can change between launch and menu
/// selection).
pub fn activity_names_mru() -> anyhow::Result<Vec<String>> {
    let json = crate::proc::run_capture(crate::proc::msg_bin(), &["msg", "--json", "activities"])?;
    parse_activity_names_mru(&json)
}

/// One `(activity, workspace)` membership row for the all-activities picker.
#[derive(Debug, PartialEq, Eq)]
pub struct AllWorkspaceRow {
    pub activity_name: String,
    pub ws_id: u64,
    /// Human-readable label, formatted as `"<activity>: <workspace>"`. Used
    /// both for display in the fuzzel picker and as the lookup key in
    /// `menu::resolve_by_label`. Resolution is first-match within the row vec;
    /// labels are unique in practice because activity names are unique and the
    /// workspace label (`name` or `<output> #<id>`) is unique per activity.
    pub label: String,
}

/// Build the all-activities picker rows from the two `--json` payloads.
///
/// One row per (activity, workspace) membership — workspaces belonging to
/// multiple activities (sticky) repeat under every activity, and the row's
/// activity decides the landing activity. Groups are MRU-ordered with the
/// active activity's group moved last (its workspaces are mostly reachable
/// via the plain picker); the focused workspace's row in the active
/// activity's group carries a `" (current)"` suffix. Memberships referencing
/// an unknown activity id (event-stream race) are dropped row-wise, never
/// fatally. Pure (unit-tested).
pub fn build_all_workspace_rows(
    workspaces_json: &str,
    activities_json: &str,
) -> anyhow::Result<Vec<AllWorkspaceRow>> {
    #[derive(Deserialize)]
    struct Row {
        id: u64,
        name: Option<String>,
        output: Option<String>,
        /// Absent on legacy payloads that predate the field — treat as
        /// unfocused via `#[serde(default)]`. On such payloads all rows in
        /// the active-activity group will silently lack the `" (current)"`
        /// suffix, which is cosmetically fine.
        #[serde(default)]
        is_focused: bool,
        #[serde(default)]
        activities: Vec<u64>,
    }
    let ws_rows: Vec<Row> =
        serde_json::from_str(workspaces_json).context("parsing workspaces JSON")?;
    let mut act_rows: Vec<ActivityRow> =
        serde_json::from_str(activities_json).context("parsing activities JSON")?;
    // MRU sort, then rotate the active activity's group to the end.
    act_rows.sort_by_key(|r| std::cmp::Reverse(r.last_active_seq.unwrap_or(r.is_active as u64)));
    act_rows.sort_by_key(|r| r.is_active); // stable: false (non-active) first, active last

    let mut rows = Vec::new();
    for act in &act_rows {
        for ws in ws_rows.iter().filter(|w| w.activities.contains(&act.id)) {
            let ws_label = ws
                .name
                .clone()
                .unwrap_or_else(|| format!("{} #{}", ws.output.as_deref().unwrap_or("?"), ws.id));
            let marker = if act.is_active && ws.is_focused {
                " (current)"
            } else {
                ""
            };
            rows.push(AllWorkspaceRow {
                activity_name: act.name.clone(),
                ws_id: ws.id,
                label: format!("{}: {ws_label}{marker}", act.name),
            });
        }
    }
    Ok(rows)
}

/// Fetch both payloads live and build the rows. Read at dispatch time, not
/// from `Snapshot` — activities state can change between launch and pick.
pub fn all_workspace_rows() -> anyhow::Result<Vec<AllWorkspaceRow>> {
    let workspaces =
        crate::proc::run_capture(crate::proc::msg_bin(), &["msg", "--json", "workspaces"])?;
    let activities =
        crate::proc::run_capture(crate::proc::msg_bin(), &["msg", "--json", "activities"])?;
    build_all_workspace_rows(&workspaces, &activities)
}

/// Dispatch a zero-argument compositor action by kebab-case name.
/// Wraps `niri msg action <name>`. Returns `Err` if `niri` exits non-zero
/// or cannot be found on `$PATH`.
pub fn run_action(name: &str) -> anyhow::Result<()> {
    crate::proc::run_capture(crate::proc::msg_bin(), &["msg", "action", name])?;
    Ok(())
}

/// Atomically land in `activity` with the referenced workspace focused, via
/// `niri msg action focus-workspace --activity <activity> <reference>`.
///
/// `activity` must be an [`ActivityName`] — a named type that prevents
/// transposing the activity and workspace-reference arguments.
///
/// `reference` is either programmatic or user-typed:
/// - Picker path: the caller formats `"id:{ws_id}"` at the call site and
///   passes that string — the compositor resolves by stable id.
/// - CLI passthrough: the raw string the user typed (name, per-monitor
///   index, or `id:N` on jiji), forwarded verbatim. The compositor rejects
///   an unrecognised reference loudly.
///
/// Requires the jiji compositor; on an older binary the subprocess fails
/// and the error (with its stderr) propagates — no fallback by design.
pub fn focus_workspace_in_activity(activity: &ActivityName, reference: &str) -> anyhow::Result<()> {
    crate::proc::run_capture(
        crate::proc::msg_bin(),
        &[
            "msg",
            "action",
            "focus-workspace",
            "--activity",
            activity.as_str(),
            reference,
        ],
    )?;
    Ok(())
}

/// Focus a workspace via `niri msg action focus-workspace <reference>`,
/// where `reference` must come from [`WorkspaceChoice::focus_reference`]:
/// the workspace name when set, else the per-monitor index as a string.
/// The [`FocusReference`] type enforces that the stable unique workspace `id`
/// is never passed here — the compositor would interpret it as an index.
pub fn focus_workspace(reference: &FocusReference) -> anyhow::Result<()> {
    crate::proc::run_capture(
        crate::proc::msg_bin(),
        &["msg", "action", "focus-workspace", reference.as_arg()],
    )?;
    Ok(())
}

/// Focus a workspace from a **user-typed** reference, passed verbatim as the
/// `focus-workspace` positional. This is the user-input trust boundary:
/// unlike [`focus_workspace`], which only accepts the programmatically
/// constructed [`FocusReference`], this lane forwards whatever the user
/// typed (a name, a per-monitor index, or `id:N` on jiji) and relies on the
/// compositor to reject a bad reference loudly.
///
/// The parameter type [`UserWorkspaceRef`] is constructible only at the
/// CLI/verb boundary (`UserWorkspaceRef::from_cli`). This prevents
/// programmatic callers from accidentally routing a computed reference
/// through this lane instead of using [`focus_workspace`] with a
/// [`FocusReference`].
pub fn focus_workspace_typed(reference: &UserWorkspaceRef) -> anyhow::Result<()> {
    crate::proc::run_capture(
        crate::proc::msg_bin(),
        &["msg", "action", "focus-workspace", reference.as_str()],
    )?;
    Ok(())
}

/// Reload the default compositor config via `niri msg action load-config-file`
/// (no path → reloads the current config file).
pub fn reload_config() -> anyhow::Result<()> {
    run_action("load-config-file")
}

/// Quit the compositor, bypassing its built-in confirmation dialog.
///
/// Uses the two-token form `niri msg action quit --skip-confirmation` rather
/// than `run_action`, which only supports zero-argument actions (no extra flags).
pub fn quit_skip_confirmation() -> anyhow::Result<()> {
    crate::proc::run_capture(
        crate::proc::msg_bin(),
        &["msg", "action", "quit", "--skip-confirmation"],
    )?;
    Ok(())
}

/// Set the focused workspace name via `niri msg action set-workspace-name <name>`.
/// No workspace reference is passed — the action defaults to the focused workspace,
/// mirroring the convention of `unset-workspace-name`.
pub fn set_workspace_name(name: &str) -> anyhow::Result<()> {
    crate::proc::run_capture(
        crate::proc::msg_bin(),
        &["msg", "action", "set-workspace-name", name],
    )?;
    Ok(())
}

/// Run `niri msg pick-window` and return its human-readable stdout.
///
/// `pick-window` is a top-level `Request` variant, not an `Action`, so it
/// is reached via `niri msg pick-window` rather than `niri msg action …`.
/// Returns `Err` if niri exits non-zero (e.g. user cancels the picker or
/// niri is unavailable).
pub fn pick_window() -> anyhow::Result<String> {
    crate::proc::run_capture(crate::proc::msg_bin(), &["msg", "pick-window"])
}

/// Run `niri msg pick-color` and return its human-readable stdout.
///
/// Like `pick-window`, this is a top-level `Request` variant reached via
/// `niri msg pick-color`. Returns `Err` if niri exits non-zero.
pub fn pick_color() -> anyhow::Result<String> {
    crate::proc::run_capture(crate::proc::msg_bin(), &["msg", "pick-color"])
}

#[derive(Deserialize)]
struct OutputBrief {
    make: String,
    model: String,
}

/// An output as offered in the monitor picker: a connector name plus a human
/// label combining the connector, make, and model.
#[derive(Debug, PartialEq, Eq)]
pub struct OutputChoice {
    /// Connector name (e.g. `"DP-1"`), used as the value passed to actions.
    pub connector: String,
    /// Human-readable label shown in the picker (e.g. `"DP-1 (Dell U2720Q)"`).
    pub label: String,
}

/// Parse `niri msg --json outputs` into picker choices.
///
/// The JSON is a `HashMap<String, OutputBrief>` keyed by connector name —
/// unlike the workspace/activities responses which are arrays. Results are
/// sorted by connector name for deterministic picker ordering.
///
/// # Errors
///
/// Returns `Err` if `json` is not valid JSON or cannot be deserialized into
/// the expected object shape.
pub fn parse_output_choices(json: &str) -> anyhow::Result<Vec<OutputChoice>> {
    use std::collections::HashMap;
    let map: HashMap<String, OutputBrief> = serde_json::from_str(json)?;
    let mut choices: Vec<OutputChoice> = map
        .into_iter()
        .map(|(connector, brief)| {
            let label = format!("{} ({} {})", connector, brief.make, brief.model);
            OutputChoice { connector, label }
        })
        .collect();
    choices.sort_by(|a, b| a.connector.cmp(&b.connector));
    Ok(choices)
}

/// Fetch the output choices live via `niri msg --json outputs`.
pub fn output_choices() -> anyhow::Result<Vec<OutputChoice>> {
    let json = crate::proc::run_capture(crate::proc::msg_bin(), &["msg", "--json", "outputs"])?;
    parse_output_choices(&json)
}

/// Focus a monitor by connector name via `niri msg action focus-monitor <connector>`.
pub fn focus_monitor(connector: &str) -> anyhow::Result<()> {
    crate::proc::run_capture(
        crate::proc::msg_bin(),
        &["msg", "action", "focus-monitor", connector],
    )?;
    Ok(())
}

/// Move the focused window to a monitor via `niri msg action move-window-to-monitor <connector>`.
///
/// No `--id` flag is passed; the compositor defaults to the focused window.
pub fn move_window_to_monitor(connector: &str) -> anyhow::Result<()> {
    crate::proc::run_capture(
        crate::proc::msg_bin(),
        &["msg", "action", "move-window-to-monitor", connector],
    )?;
    Ok(())
}

/// Move the focused column to a monitor via `niri msg action move-column-to-monitor <connector>`.
pub fn move_column_to_monitor(connector: &str) -> anyhow::Result<()> {
    crate::proc::run_capture(
        crate::proc::msg_bin(),
        &["msg", "action", "move-column-to-monitor", connector],
    )?;
    Ok(())
}

/// Move the focused workspace to a monitor via `niri msg action move-workspace-to-monitor <connector>`.
///
/// No `--reference` flag is passed; the compositor defaults to the focused workspace.
pub fn move_workspace_to_monitor(connector: &str) -> anyhow::Result<()> {
    crate::proc::run_capture(
        crate::proc::msg_bin(),
        &["msg", "action", "move-workspace-to-monitor", connector],
    )?;
    Ok(())
}

#[derive(Deserialize)]
struct CastRow {
    session_id: u64,
    pid: Option<i32>,
}

/// A screencast session as offered in the stop-cast picker: a session id plus
/// a human label.
#[derive(Debug, PartialEq, Eq)]
pub struct CastChoice {
    pub session_id: u64,
    pub label: String,
}

/// Parse `niri msg --json casts` into picker choices.
///
/// Multiple `Cast` rows can share one `session_id` (one session, multiple
/// streams). This function deduplicates by `session_id`, keeping the first
/// occurrence. Results are sorted by `session_id` for deterministic picker
/// ordering.
///
/// # Errors
///
/// Returns `Err` if `json` is not valid JSON or cannot be deserialized into the
/// expected array shape.
pub fn parse_cast_choices(json: &str) -> anyhow::Result<Vec<CastChoice>> {
    let rows: Vec<CastRow> =
        serde_json::from_str(json).context("parsing niri casts JSON (schema may have changed)")?;
    let mut seen = std::collections::HashSet::new();
    let mut choices: Vec<CastChoice> = rows
        .into_iter()
        .filter(|r| seen.insert(r.session_id))
        .map(|r| {
            let label = match r.pid {
                Some(pid) => format!("session {} (pid {})", r.session_id, pid),
                None => format!("session {}", r.session_id),
            };
            CastChoice {
                session_id: r.session_id,
                label,
            }
        })
        .collect();
    choices.sort_by_key(|c| c.session_id);
    Ok(choices)
}

/// Fetch the cast choices live via `niri msg --json casts`.
pub fn cast_choices() -> anyhow::Result<Vec<CastChoice>> {
    let json = crate::proc::run_capture(crate::proc::msg_bin(), &["msg", "--json", "casts"])?;
    parse_cast_choices(&json)
}

/// Stop a screencast session via `niri msg action stop-cast --session-id <id>`.
///
/// The session id is passed as a separate argv element after `--session-id`,
/// not joined with `=`. Cannot go through `run_action` (zero-arg only).
pub fn stop_cast(session_id: u64) -> anyhow::Result<()> {
    let id = session_id.to_string();
    crate::proc::run_capture(
        crate::proc::msg_bin(),
        &["msg", "action", "stop-cast", "--session-id", &id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_uses_name_then_falls_back() {
        let json = r#"[
            {"id":21,"idx":1,"name":"web","output":"DP-2"},
            {"id":22,"idx":2,"name":null,"output":"DP-3"}
        ]"#;
        let c = parse_workspace_choices(json).unwrap();
        assert_eq!(
            c[0],
            WorkspaceChoice {
                id: 21,
                idx: 1,
                name: Some("web".into()),
                label: "web".into(),
            }
        );
        assert_eq!(
            c[1],
            WorkspaceChoice {
                id: 22,
                idx: 2,
                name: None,
                label: "DP-3 #22".into(),
            }
        );
    }

    #[test]
    fn parse_both_null_uses_question_mark_fallback() {
        // When both name and output are null, the output placeholder is "?" and
        // the label format is "? #<id>".
        let json = r#"[{"id":5,"idx":1,"name":null,"output":null}]"#;
        let c = parse_workspace_choices(json).unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(
            c[0],
            WorkspaceChoice {
                id: 5,
                idx: 1,
                name: None,
                label: "? #5".into(),
            }
        );
    }

    #[test]
    fn workspace_choices_filter_to_active_activity_on_jiji() {
        // jiji-shaped JSON: is_in_active_activity present.
        let json = r#"[
            {"id":1,"idx":1,"name":"web","output":"DP-1","is_focused":true,"is_in_active_activity":true},
            {"id":2,"idx":2,"name":null,"output":"DP-1","is_focused":false,"is_in_active_activity":true},
            {"id":3,"idx":1,"name":"mail","output":"DP-1","is_focused":false,"is_in_active_activity":false}
        ]"#;
        let c = parse_workspace_choices(json).unwrap();
        assert_eq!(
            c.len(),
            2,
            "dormant-activity workspace must be filtered out"
        );
        assert_eq!(c[0].label, "web");
        assert_eq!(c[1].label, "DP-1 #2");
    }

    #[test]
    fn workspace_choices_include_everything_on_vanilla_niri() {
        // Vanilla niri: no is_in_active_activity field at all.
        let json = r#"[
            {"id":1,"idx":1,"name":"web","output":"DP-1","is_focused":true},
            {"id":2,"idx":2,"name":null,"output":"DP-1","is_focused":false}
        ]"#;
        let c = parse_workspace_choices(json).unwrap();
        assert_eq!(c.len(), 2, "absent field must not filter anything");
    }

    #[test]
    fn focus_args_prefer_name_then_idx() {
        let named = WorkspaceChoice {
            id: 7,
            idx: 3,
            name: Some("web".into()),
            label: "web".into(),
        };
        let unnamed = WorkspaceChoice {
            id: 8,
            idx: 4,
            name: None,
            label: "DP-1 #8".into(),
        };
        assert_eq!(named.focus_reference().as_arg(), "web");
        assert_eq!(unnamed.focus_reference().as_arg(), "4");
    }

    #[test]
    fn parse_activity_names_mru_sorts_by_last_active_seq_desc() {
        // Inventory order is id order; MRU order must come from the seq.
        let json = r#"[
            {"name":"default","is_active":false,"last_active_seq":2},
            {"name":"work","is_active":true,"last_active_seq":7},
            {"name":"play","is_active":false,"last_active_seq":5}
        ]"#;
        let names = parse_activity_names_mru(json).unwrap();
        assert_eq!(names, vec!["work", "play", "default"]);
    }

    #[test]
    fn parse_activity_names_mru_without_seq_puts_active_first() {
        // Older compositors omit last_active_seq: the active activity leads,
        // the rest keep inventory order (stable sort).
        let json = r#"[
            {"name":"default","is_active":false},
            {"name":"work","is_active":false},
            {"name":"play","is_active":true},
            {"name":"games","is_active":false}
        ]"#;
        let names = parse_activity_names_mru(json).unwrap();
        assert_eq!(names, vec!["play", "default", "work", "games"]);
    }

    #[test]
    fn parse_activity_names_mru_empty_array_returns_empty_vec() {
        assert!(parse_activity_names_mru("[]").unwrap().is_empty());
    }

    #[test]
    fn parse_empty_array_returns_empty_vec() {
        let c = parse_workspace_choices("[]").unwrap();
        assert!(c.is_empty());
    }

    #[test]
    fn parse_malformed_json_returns_err() {
        assert!(parse_workspace_choices("{not json").is_err());
    }

    // ---- output choices ----

    #[test]
    fn parse_output_choices_multi_output_fixture() {
        let json = r#"{"DP-1":{"make":"Dell","model":"U2720Q","serial":"abc","physical_size":{"w":600,"h":340},"modes":[],"current_mode":null,"vrr_supported":false,"vrr_enabled":false,"logical":null},"eDP-1":{"make":"Apple","model":"Built-in","serial":"","physical_size":{"w":300,"h":190},"modes":[],"current_mode":null,"vrr_supported":false,"vrr_enabled":false,"logical":null}}"#;
        let choices = parse_output_choices(json).unwrap();
        assert_eq!(choices.len(), 2);
        // Sorted by connector: DP-1 before eDP-1 (uppercase D < lowercase e in ASCII
        // but not lexicographic Unicode; however both start with D/e — in byte order
        // 'D' (68) < 'e' (101) so DP-1 sorts first).
        assert_eq!(choices[0].connector, "DP-1");
        assert_eq!(choices[0].label, "DP-1 (Dell U2720Q)");
        assert_eq!(choices[1].connector, "eDP-1");
        assert_eq!(choices[1].label, "eDP-1 (Apple Built-in)");
    }

    #[test]
    fn parse_output_choices_empty_object_returns_empty_vec() {
        let choices = parse_output_choices("{}").unwrap();
        assert!(choices.is_empty());
    }

    #[test]
    fn parse_output_choices_malformed_json_returns_err() {
        assert!(parse_output_choices("[not an object]").is_err());
        assert!(parse_output_choices("{not json").is_err());
    }

    #[test]
    fn parse_output_choices_sorted_by_connector() {
        // Three outputs in non-sorted declaration order; result must be sorted.
        let json = r#"{"HDMI-A-1":{"make":"LG","model":"27UK850","serial":"","physical_size":{"w":600,"h":340},"modes":[],"current_mode":null,"vrr_supported":false,"vrr_enabled":false,"logical":null},"DP-2":{"make":"Samsung","model":"S27A800U","serial":"","physical_size":{"w":600,"h":340},"modes":[],"current_mode":null,"vrr_supported":false,"vrr_enabled":false,"logical":null},"DP-1":{"make":"Dell","model":"U2720Q","serial":"","physical_size":{"w":600,"h":340},"modes":[],"current_mode":null,"vrr_supported":false,"vrr_enabled":false,"logical":null}}"#;
        let choices = parse_output_choices(json).unwrap();
        let connectors: Vec<&str> = choices.iter().map(|c| c.connector.as_str()).collect();
        let mut sorted = connectors.clone();
        sorted.sort();
        assert_eq!(
            connectors, sorted,
            "output choices must be sorted by connector name"
        );
    }

    // ---- cast choices ----

    #[test]
    fn parse_cast_choices_deduplicates_by_session_id() {
        // Two rows share session_id 7; one row has session_id 3.
        // After dedup, two choices: session 3 and session 7 (first occurrence wins).
        // Sorted by session_id: [3, 7].
        let json = r#"[
            {"session_id":7,"stream_id":1,"pid":1234},
            {"session_id":7,"stream_id":2,"pid":1234},
            {"session_id":3,"stream_id":3,"pid":5678}
        ]"#;
        let choices = parse_cast_choices(json).unwrap();
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0].session_id, 3);
        assert_eq!(choices[1].session_id, 7);
        // First occurrence of session 7 had pid 1234.
        assert!(choices[1].label.contains("1234"));
    }

    #[test]
    fn parse_cast_choices_empty_array_returns_empty_vec() {
        let choices = parse_cast_choices("[]").unwrap();
        assert!(choices.is_empty());
    }

    #[test]
    fn parse_cast_choices_pid_null_omits_pid_suffix() {
        // A cast row with pid: null must produce a label without the "(pid …)" suffix.
        let json = r#"[{"session_id":5,"stream_id":1,"pid":null}]"#;
        let choices = parse_cast_choices(json).unwrap();
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].label, "session 5");
        assert!(!choices[0].label.contains("pid"));
    }

    #[test]
    fn parse_cast_choices_malformed_json_returns_err() {
        assert!(parse_cast_choices("{not json").is_err());
    }

    // ---- workspace name listing (completion candidates source) ----

    #[test]
    fn parse_workspace_names_lists_current_activity_named_only() {
        // web is named+active-activity; #22 unnamed (omitted); mail is
        // dormant-activity (filtered).
        let json = r#"[
            {"id":21,"idx":1,"name":"web","output":"DP-1","is_in_active_activity":true},
            {"id":22,"idx":2,"name":null,"output":"DP-1","is_in_active_activity":true},
            {"id":23,"idx":1,"name":"mail","output":"DP-1","is_in_active_activity":false}
        ]"#;
        assert_eq!(parse_workspace_names(json).unwrap(), vec!["web"]);
    }

    #[test]
    fn parse_workspace_names_vanilla_niri_lists_everything_named() {
        let json = r#"[
            {"id":1,"idx":1,"name":"web","output":"DP-1"},
            {"id":2,"idx":2,"name":null,"output":"DP-1"}
        ]"#;
        assert_eq!(parse_workspace_names(json).unwrap(), vec!["web"]);
    }

    #[test]
    fn parse_workspace_names_in_activity_filters_by_membership() {
        let workspaces = r#"[
            {"id":21,"idx":1,"name":"web","output":"DP-1","activities":[1]},
            {"id":22,"idx":2,"name":null,"output":"DP-1","activities":[2]},
            {"id":23,"idx":1,"name":"mail","output":"DP-1","activities":[2]}
        ]"#;
        let activities = r#"[
            {"id":1,"name":"acme","is_active":true,"last_active_seq":9},
            {"id":2,"name":"home","is_active":false,"last_active_seq":4}
        ]"#;
        assert_eq!(
            parse_workspace_names_in_activity(workspaces, activities, "home").unwrap(),
            vec!["mail"] // #22 is unnamed — omitted
        );
    }

    #[test]
    fn parse_workspace_names_in_activity_unknown_activity_errs() {
        let activities = r#"[{"id":1,"name":"acme","is_active":true,"last_active_seq":9}]"#;
        let err = parse_workspace_names_in_activity("[]", activities, "nope").unwrap_err();
        assert!(err.to_string().contains("unknown activity"), "{err}");
    }

    // ---- all-workspace rows (all-activities picker) ----

    #[test]
    fn all_rows_group_mru_current_last_with_membership_expansion() {
        let workspaces = r#"[
            {"id":1,"idx":1,"name":"editor","output":"DP-1","is_focused":false,"is_in_active_activity":true,"activities":[10]},
            {"id":2,"idx":2,"name":"browser","output":"DP-1","is_focused":true,"is_in_active_activity":true,"activities":[10]},
            {"id":3,"idx":1,"name":"media","output":"DP-1","is_focused":false,"is_in_active_activity":false,"activities":[20]},
            {"id":4,"idx":3,"name":"scratch","output":"DP-1","is_focused":false,"is_in_active_activity":true,"activities":[10,20,30]}
        ]"#;
        let activities = r#"[
            {"id":10,"name":"work","is_active":true,"last_active_seq":9},
            {"id":20,"name":"home","is_active":false,"last_active_seq":7},
            {"id":30,"name":"mail","is_active":false,"last_active_seq":3}
        ]"#;
        let rows = build_all_workspace_rows(workspaces, activities).unwrap();
        let labels: Vec<&str> = rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(
            labels,
            vec![
                // MRU first among non-current: home (seq 7), then mail (seq 3);
                // current activity (work) group last; focused row marked.
                "home: media",
                "home: scratch",
                "mail: scratch",
                "work: editor",
                "work: browser (current)",
                "work: scratch",
            ]
        );
        // Dispatch payload carried per row, not re-parsed from the label.
        assert_eq!(rows[0].activity_name, "home");
        assert_eq!(rows[0].ws_id, 3);
        assert_eq!(rows[2].activity_name, "mail");
        assert_eq!(rows[2].ws_id, 4);
    }

    #[test]
    fn all_rows_is_focused_absent_does_not_mark_current() {
        // Legacy payload: `is_focused` absent on workspace rows. No row should
        // carry the " (current)" suffix — the field defaults to false.
        let workspaces = r#"[
            {"id":1,"idx":1,"name":"editor","output":"DP-1","activities":[10]}
        ]"#;
        let activities = r#"[{"id":10,"name":"work","is_active":true,"last_active_seq":1}]"#;
        let rows = build_all_workspace_rows(workspaces, activities).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].label, "work: editor",
            "absent is_focused must not produce the (current) suffix"
        );
    }

    #[test]
    fn all_rows_unknown_activity_id_is_skipped_not_fatal() {
        // A workspace pointing at an activity id missing from the activities
        // array (event-stream race) drops that membership row only.
        let workspaces = r#"[
            {"id":1,"idx":1,"name":"a","output":"DP-1","is_focused":true,"is_in_active_activity":true,"activities":[10,99]}
        ]"#;
        let activities = r#"[{"id":10,"name":"work","is_active":true,"last_active_seq":1}]"#;
        let rows = build_all_workspace_rows(workspaces, activities).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label, "work: a (current)");
    }
}
