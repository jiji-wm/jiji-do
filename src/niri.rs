//! Helpers for the `niri msg` subprocess surface used by native verbs.

use serde::Deserialize;

#[derive(Deserialize)]
struct WorkspaceRow {
    id: u64,
    name: Option<String>,
    output: Option<String>,
}

/// A workspace as offered in the switch picker: a stable id plus a human label.
#[derive(Debug, PartialEq, Eq)]
pub struct WorkspaceChoice {
    pub id: u64,
    pub label: String,
}

/// Parse `niri msg --json workspaces` into picker choices. Label is the
/// workspace name when set, else `"<output> #<id>"`. Pure (unit-tested).
pub fn parse_workspace_choices(json: &str) -> anyhow::Result<Vec<WorkspaceChoice>> {
    let rows: Vec<WorkspaceRow> = serde_json::from_str(json)?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let label = r
                .name
                .unwrap_or_else(|| format!("{} #{}", r.output.as_deref().unwrap_or("?"), r.id));
            WorkspaceChoice { id: r.id, label }
        })
        .collect())
}

/// Fetch the workspace choices live.
pub fn workspace_choices() -> anyhow::Result<Vec<WorkspaceChoice>> {
    let json = crate::proc::run_capture("niri", &["msg", "--json", "workspaces"])?;
    parse_workspace_choices(&json)
}

/// Dispatch a zero-argument compositor action by kebab-case name.
/// Wraps `niri msg action <name>`. Returns `Err` if `niri` exits non-zero
/// or cannot be found on `$PATH`.
pub fn run_action(name: &str) -> anyhow::Result<()> {
    crate::proc::run_capture("niri", &["msg", "action", name])?;
    Ok(())
}

/// Focus a workspace by id via `niri msg action focus-workspace <id>`.
pub fn focus_workspace(id: u64) -> anyhow::Result<()> {
    let id = id.to_string();
    crate::proc::run_capture("niri", &["msg", "action", "focus-workspace", &id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_uses_name_then_falls_back() {
        let json = r#"[
            {"id":21,"name":"web","output":"DP-2"},
            {"id":22,"name":null,"output":"DP-3"}
        ]"#;
        let c = parse_workspace_choices(json).unwrap();
        assert_eq!(
            c[0],
            WorkspaceChoice {
                id: 21,
                label: "web".into()
            }
        );
        assert_eq!(
            c[1],
            WorkspaceChoice {
                id: 22,
                label: "DP-3 #22".into()
            }
        );
    }
}
