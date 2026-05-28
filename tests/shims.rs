//! End-to-end tests with $PATH-scoped shim executables.

use assert_cmd::Command;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

/// Create an executable shim named `name` in `dir` with the given sh body.
fn shim(dir: &std::path::Path, name: &str, body: &str) {
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).unwrap();
}

/// A `niri` shim that answers the three `--json` reads and records actions.
/// `$2 $3 = "--json windows"` etc. → echo JSON; `msg action focus-workspace <id>` →
/// append the id to $ACTIONS_FILE.
fn niri_body(actions_file: &str) -> String {
    format!(
        r#"
case "$2 $3" in
  "--json windows")    echo '[{{"id":11,"is_focused":true}}]' ;;
  "--json workspaces") echo '[{{"id":21,"name":"web","output":"DP-1","is_focused":true}}]' ;;
  "--json activities") echo '[{{"name":"acme","is_active":true}}]' ;;
  *)
    # `msg action focus-workspace <id>` → $3=focus-workspace, $4=<id>
    echo "$3 $4" >> "{actions_file}"
    ;;
esac
"#
    )
}

#[test]
fn debug_reports_filtering_upstream() {
    let dir = TempDir::new().unwrap();
    // Upstream: niri present (but activities read fails), fuzzel present, no jiji-activities.
    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json activities") exit 1 ;;
  "--json windows")    echo '[]' ;;
  "--json workspaces") echo '[]' ;;
esac"#,
    );
    shim(dir.path(), "fuzzel", "exit 0");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("--debug")
        .assert()
        .success()
        // switch-activity needs FORK + NIRI_ACTIVITIES → filtered upstream.
        .stdout(predicates::str::contains("switch-activity: filtered"))
        .stdout(predicates::str::contains("switch-workspace: kept"))
        .stdout(predicates::str::contains("focus-workspace-previous: kept"))
        .stdout(predicates::str::contains("toggle-debug-tint: kept"))
        .stdout(predicates::str::contains(
            "switch-activity-previous: filtered",
        ))
        .stdout(predicates::str::contains(
            "move-window-to-activity: filtered",
        ))
        .stdout(predicates::str::contains("move-window-here: filtered"))
        .stdout(predicates::str::contains(
            "move-workspace-to-activity: filtered",
        ))
        .stdout(predicates::str::contains("assign-workspace: filtered"))
        .stdout(predicates::str::contains("save-activity: filtered"));
}

#[test]
fn switch_workspace_focuses_picked_id() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    // fuzzel shim: echo the workspace label the user "picked".
    shim(dir.path(), "fuzzel", "cat >/dev/null; echo 'web'");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("switch-workspace")
        .assert()
        .success();

    // The shim recorded `focus-workspace 21`.
    let recorded = std::fs::read_to_string(&actions).unwrap();
    assert!(
        recorded.contains("focus-workspace 21"),
        "expected focus-workspace 21, got: {recorded:?}"
    );
}

#[test]
fn switch_activity_passes_switch_to_jiji_activities() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    // niri shim: answer all --json probes (FORK detected via activities success).
    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    shim(dir.path(), "fuzzel", "exit 0");
    // jiji-activities shim: record its argv and exit 0.
    shim(
        dir.path(),
        "jiji-activities",
        &format!(
            r#"echo "$@" >> "{argv}"
exit 0"#,
            argv = argv_file.display()
        ),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("switch-activity")
        .assert()
        .success();

    // jiji-activities is called twice: once with "--version" during capability
    // probing, then with "switch" for the actual dispatch. Assert that the
    // dispatch call arrived with exactly "switch" (not "switch-activity").
    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.contains(&"switch"),
        "expected jiji-activities to receive 'switch', got: {recorded:?}"
    );
    assert!(
        !lines.iter().any(|l| l.contains("switch-activity")),
        "jiji-activities must not receive 'switch-activity', got: {recorded:?}"
    );
}

#[test]
fn gated_verb_direct_invocation_exits_69_upstream() {
    let dir = TempDir::new().unwrap();
    // Upstream: no fork (activities read fails), no jiji-activities.
    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json activities") exit 1 ;;
  "--json windows")    echo '[{"id":1,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":2,"output":"DP-1","is_focused":true}]' ;;
esac"#,
    );
    shim(dir.path(), "fuzzel", "exit 0");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("switch-activity")
        .assert()
        .code(69)
        .stderr(predicates::str::contains("switch-activity"));
}

/// fuzzel exits with a non-1 code (genuine failure, e.g. display connection error)
/// during switch-workspace → jiji-do exits non-zero and reports "fuzzel failed".
/// This is the discriminating test for the cancel-vs-failure fix: under the old
/// `if success && !empty { Some } else { None }` shape, exit 2 silently becomes
/// None and jiji-do exits 0. Under the fix (only exit 1 → cancel), bail! fires.
#[test]
fn switch_workspace_fuzzel_failure_exits_nonzero() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    // fuzzel shim: print to stderr and exit 2 (e.g. display error).
    shim(dir.path(), "fuzzel", "echo 'display error' >&2; exit 2");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("switch-workspace")
        .assert()
        .failure()
        .stderr(predicates::str::contains("fuzzel failed"));

    // No focus-workspace action must have been dispatched.
    assert!(
        !actions.exists(),
        "expected no actions on fuzzel failure, but actions file exists"
    );
}

/// fuzzel exit-1 (user cancel) during switch-workspace → jiji-do exits 0 and
/// records no focus-workspace action.
#[test]
fn switch_workspace_fuzzel_cancel_exits_zero_no_action() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    // fuzzel shim: exit 1 (user pressed Escape) with empty stdout.
    shim(dir.path(), "fuzzel", "exit 1");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("switch-workspace")
        .assert()
        .success();

    // No focus-workspace action must have been dispatched.
    assert!(
        !actions.exists(),
        "expected no actions to be recorded on cancel, but actions file exists"
    );
}

#[test]
fn focus_workspace_previous_dispatches_action() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("focus-workspace-previous")
        .assert()
        .success();

    // The shim records `$3 $4`; for zero-arg actions $4 is empty.
    let recorded = std::fs::read_to_string(&actions).unwrap();
    assert!(
        recorded.starts_with("focus-workspace-previous"),
        "expected action starting with focus-workspace-previous, got: {recorded:?}"
    );
}

/// A failing `niri msg action` (non-zero exit) must surface as a non-zero
/// `jiji-do` exit — NOT exit 0 (silent failure), NOT exit 69 (capability miss).
/// This pins the contract analogous to the fuzzel cancel-vs-failure fix:
/// subprocess failures must propagate, not be silently swallowed.
#[test]
fn niri_action_failure_propagates_nonzero() {
    let dir = TempDir::new().unwrap();
    // niri shim: answers snapshot probes successfully, but exits 1 on action dispatch.
    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[{"name":"acme","is_active":true}]' ;;
  *)
    echo "niri msg action failed" >&2
    exit 1
    ;;
esac"#,
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("focus-workspace-previous")
        .assert()
        .failure()
        // Must NOT be exit 69 (capability miss) — this is a runtime action failure.
        .code(predicates::ord::ne(69));
}

#[test]
fn toggle_debug_tint_dispatches_action() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("toggle-debug-tint")
        .assert()
        .success();

    // The shim records `$3 $4`; for zero-arg actions $4 is empty.
    let recorded = std::fs::read_to_string(&actions).unwrap();
    assert!(
        recorded.starts_with("toggle-debug-tint"),
        "expected action starting with toggle-debug-tint, got: {recorded:?}"
    );
}

#[test]
fn switch_activity_previous_dispatches_switch_previous() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    shim(dir.path(), "fuzzel", "exit 0");
    shim(
        dir.path(),
        "jiji-activities",
        &format!(
            r#"echo "$@" >> "{argv}"
exit 0"#,
            argv = argv_file.display()
        ),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("switch-activity-previous")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.contains(&"switch-previous"),
        "expected jiji-activities to receive 'switch-previous', got: {recorded:?}"
    );
}

#[test]
fn move_window_to_activity_passes_window_id() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    shim(dir.path(), "fuzzel", "exit 0");
    shim(
        dir.path(),
        "jiji-activities",
        &format!(
            r#"echo "$@" >> "{argv}"
exit 0"#,
            argv = argv_file.display()
        ),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("move-window-to-activity")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.contains(&"move-window --window=11"),
        "expected jiji-activities to receive 'move-window --window=11', got: {recorded:?}"
    );
}

#[test]
fn move_window_here_passes_window_id() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    shim(dir.path(), "fuzzel", "exit 0");
    shim(
        dir.path(),
        "jiji-activities",
        &format!(
            r#"echo "$@" >> "{argv}"
exit 0"#,
            argv = argv_file.display()
        ),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("move-window-here")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.contains(&"move-window-here --window=11"),
        "expected jiji-activities to receive 'move-window-here --window=11', got: {recorded:?}"
    );
}

#[test]
fn move_workspace_to_activity_passes_workspace_id() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    shim(dir.path(), "fuzzel", "exit 0");
    shim(
        dir.path(),
        "jiji-activities",
        &format!(
            r#"echo "$@" >> "{argv}"
exit 0"#,
            argv = argv_file.display()
        ),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("move-workspace-to-activity")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.contains(&"move-workspace --workspace=21"),
        "expected jiji-activities to receive 'move-workspace --workspace=21', got: {recorded:?}"
    );
}

#[test]
fn assign_workspace_passes_workspace_id() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    shim(dir.path(), "fuzzel", "exit 0");
    shim(
        dir.path(),
        "jiji-activities",
        &format!(
            r#"echo "$@" >> "{argv}"
exit 0"#,
            argv = argv_file.display()
        ),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("assign-workspace")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.contains(&"assign-workspace --workspace=21"),
        "expected jiji-activities to receive 'assign-workspace --workspace=21', got: {recorded:?}"
    );
}

/// When no window is focused at launch, `move-window-to-activity` must bail
/// before calling `jiji-activities` (exit non-zero, NOT 69). The `jiji-activities`
/// argv file must contain only the capability-probe `--version` line, confirming
/// no dispatch argv was sent.
#[test]
fn move_window_to_activity_bails_when_no_focused_window() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    // niri shim: windows list has no focused window; workspaces and activities normal.
    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":false}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[{"name":"acme","is_active":true}]' ;;
esac"#,
    );
    shim(dir.path(), "fuzzel", "exit 0");
    shim(
        dir.path(),
        "jiji-activities",
        &format!(
            r#"echo "$@" >> "{argv}"
exit 0"#,
            argv = argv_file.display()
        ),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("move-window-to-activity")
        .assert()
        .failure()
        .code(predicates::ord::ne(69))
        .stderr(predicates::str::contains("no focused window at launch"));

    // Only the --version probe must appear in the argv file; no dispatch argv.
    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.iter().all(|l| *l == "--version"),
        "expected only --version probe in argv, got: {recorded:?}"
    );
}

/// When no workspace is focused at launch, `move-workspace-to-activity` must
/// bail before calling `jiji-activities` (exit non-zero, NOT 69). The
/// `jiji-activities` argv file must contain only the capability-probe
/// `--version` line, confirming no dispatch argv was sent.
#[test]
fn move_workspace_to_activity_bails_when_no_focused_workspace() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    // niri shim: workspaces list has no focused workspace; windows and activities normal.
    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":false}]' ;;
  "--json activities") echo '[{"name":"acme","is_active":true}]' ;;
esac"#,
    );
    shim(dir.path(), "fuzzel", "exit 0");
    shim(
        dir.path(),
        "jiji-activities",
        &format!(
            r#"echo "$@" >> "{argv}"
exit 0"#,
            argv = argv_file.display()
        ),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("move-workspace-to-activity")
        .assert()
        .failure()
        .code(predicates::ord::ne(69))
        .stderr(predicates::str::contains("no focused workspace at launch"));

    // Only the --version probe must appear in the argv file; no dispatch argv.
    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.iter().all(|l| *l == "--version"),
        "expected only --version probe in argv, got: {recorded:?}"
    );
}

#[test]
fn save_activity_passes_focused_activity_name() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    shim(dir.path(), "fuzzel", "exit 0");
    shim(
        dir.path(),
        "jiji-activities",
        &format!(
            r#"echo "$@" >> "{argv}"
exit 0"#,
            argv = argv_file.display()
        ),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("save-activity")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.contains(&"save acme"),
        "expected jiji-activities to receive 'save acme', got: {recorded:?}"
    );
    assert!(
        !lines
            .iter()
            .any(|l| l.contains("save-activity") || l.contains("--name=")),
        "jiji-activities must not receive flag-style name arg, got: {recorded:?}"
    );
}

/// When no activity is focused at launch, `save-activity` must bail before
/// calling `jiji-activities` (exit non-zero, NOT 69). The `jiji-activities`
/// argv file must contain only the capability-probe `--version` line,
/// confirming no dispatch argv was sent.
#[test]
fn save_activity_bails_when_no_focused_activity() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    // niri shim: activities list has no active activity; windows and workspaces normal.
    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[{"name":"acme","is_active":false}]' ;;
esac"#,
    );
    shim(dir.path(), "fuzzel", "exit 0");
    shim(
        dir.path(),
        "jiji-activities",
        &format!(
            r#"echo "$@" >> "{argv}"
exit 0"#,
            argv = argv_file.display()
        ),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("save-activity")
        .assert()
        .failure()
        .code(predicates::ord::ne(69))
        .stderr(predicates::str::contains("no focused activity at launch"));

    // Only the --version probe must appear in the argv file; no dispatch argv.
    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.iter().all(|l| *l == "--version"),
        "expected only --version probe in argv, got: {recorded:?}"
    );
}
