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

/// A `niri` shim that answers the four `--json` reads and records actions.
/// `$2 $3 = "--json windows"` etc. → echo JSON; action arms → append the
/// full action tail (everything after `msg action`) to $ACTIONS_FILE via
/// `shift 2; echo "$@"`, so multi-arg actions like
/// `focus-workspace --activity home id:23` are captured in full.
/// For zero- and one-arg actions the recorded output is unchanged from the
/// previous two-token form (no trailing space from an empty `$@`).
fn niri_body(actions_file: &str) -> String {
    format!(
        r#"
case "$2 $3" in
  "--json windows")    echo '[{{"id":11,"is_focused":true}}]' ;;
  "--json workspaces") echo '[{{"id":21,"idx":1,"name":"web","output":"DP-1","is_focused":true,"is_in_active_activity":true,"activities":[1]}},{{"id":22,"idx":2,"name":null,"output":"DP-1","is_focused":false,"is_in_active_activity":true,"activities":[1]}},{{"id":23,"idx":1,"name":"mail","output":"DP-1","is_focused":false,"is_in_active_activity":false,"activities":[2]}}]' ;;
  "--json activities") echo '[{{"id":1,"name":"acme","is_active":true,"last_active_seq":9}},{{"id":2,"name":"home","is_active":false,"last_active_seq":4}}]' ;;
  "--json outputs")    echo '{{"DP-1":{{"make":"Dell","model":"U2720Q","serial":"","physical_size":{{"w":600,"h":340}},"modes":[],"current_mode":null,"vrr_supported":false,"vrr_enabled":false,"logical":null}}}}' ;;
  *)
    shift 2
    echo "$@" >> "{actions_file}"
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
  "--json outputs")    echo '{"DP-1":{"make":"Dell","model":"U2720Q","serial":"","physical_size":{"w":600,"h":340},"modes":[],"current_mode":null,"vrr_supported":false,"vrr_enabled":false,"logical":null}}' ;;
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
        .stdout(predicates::str::contains("switch-workspace-all: filtered"))
        .stdout(predicates::str::contains("focus-workspace-previous: kept"))
        .stdout(predicates::str::contains("unset-workspace-name: kept"))
        .stdout(predicates::str::contains("pick-window: kept"))
        .stdout(predicates::str::contains("focus-monitor: kept"))
        .stdout(predicates::str::contains("move-window-to-monitor: kept"))
        .stdout(predicates::str::contains("move-column-to-monitor: kept"))
        .stdout(predicates::str::contains("move-workspace-to-monitor: kept"))
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
        .stdout(predicates::str::contains("save-activity: filtered"))
        .stdout(predicates::str::contains("list-activities: filtered"))
        .stdout(predicates::str::contains("create-activity: filtered"))
        .stdout(predicates::str::contains("remove-activity: filtered"))
        .stdout(predicates::str::contains("rename-activity: filtered"))
        .stdout(predicates::str::contains("reload-config: kept"))
        .stdout(predicates::str::contains("power-on-monitors: kept"))
        .stdout(predicates::str::contains("pick-color: kept"))
        .stdout(predicates::str::contains("quit: kept"))
        .stdout(predicates::str::contains("power-off-monitors: kept"))
        .stdout(predicates::str::contains("stop-cast: kept"))
        .stdout(predicates::str::contains("rename-workspace: kept"))
        .stdout(predicates::str::contains("list-workspaces: kept"));
}

#[test]
fn switch_workspace_dispatches_name_for_named_row() {
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

    // Named workspace dispatches by name, not by unique id.
    let recorded = std::fs::read_to_string(&actions).unwrap();
    assert!(
        recorded.contains("focus-workspace web"),
        "expected focus-workspace web, got: {recorded:?}"
    );
}

#[test]
fn switch_workspace_dispatches_idx_for_unnamed_row() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    // fuzzel shim: pick the unnamed row (DP-1 #22), which has idx=2.
    shim(dir.path(), "fuzzel", "cat >/dev/null; echo 'DP-1 #22'");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("switch-workspace")
        .assert()
        .success();

    // Unnamed workspace must dispatch by per-monitor idx, NOT by the unique id 22.
    let recorded = std::fs::read_to_string(&actions).unwrap();
    assert!(
        recorded.contains("focus-workspace 2"),
        "expected focus-workspace 2 (idx), got: {recorded:?}"
    );
    assert!(
        !recorded.contains("focus-workspace 22"),
        "must NOT dispatch the unique id 22 as the reference, got: {recorded:?}"
    );
}

#[test]
fn switch_workspace_hides_dormant_activity_rows() {
    let dir = TempDir::new().unwrap();
    let stdin_file = dir.path().join("fuzzel_stdin");
    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    // fuzzel shim: record its stdin to a file, then exit 1 (cancel).
    shim(
        dir.path(),
        "fuzzel",
        &format!(
            r#"cat > "{stdin_file}"
exit 1"#,
            stdin_file = stdin_file.display()
        ),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("switch-workspace")
        .assert()
        .success(); // cancel → exit 0

    // Picker must contain active-activity rows but NOT the dormant "mail" row.
    let picker_input = std::fs::read_to_string(&stdin_file).unwrap();
    assert!(
        picker_input.contains("web"),
        "expected 'web' in picker input, got: {picker_input:?}"
    );
    assert!(
        picker_input.contains("DP-1 #22"),
        "expected 'DP-1 #22' in picker input, got: {picker_input:?}"
    );
    assert!(
        !picker_input.contains("mail"),
        "dormant-activity workspace 'mail' must NOT appear in picker, got: {picker_input:?}"
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
    // fuzzel shim: drain stdin (avoids broken-pipe race on the workspace label
    // list write in `menu::pick_one`), then exit 2 to simulate a display error.
    shim(
        dir.path(),
        "fuzzel",
        "cat >/dev/null; echo 'display error' >&2; exit 2",
    );

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
    // fuzzel shim: drain stdin before exiting (avoids broken-pipe race on the
    // candidate list write in `menu::pick_one`) then exit 1 for user cancel.
    shim(dir.path(), "fuzzel", "cat >/dev/null; exit 1");

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

    // The shim records the full action tail (after `msg action`).
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

/// Passthrough stderr forwarding: informational breadcrumbs the passthrough
/// target prints to stderr (confirmations, warnings) must reach jiji-do's own
/// stderr on success. `Command::output` captures the child's stderr; without
/// an explicit forward in `run_capture` it would be silently swallowed.
#[test]
fn save_activity_forwards_passthrough_stderr_breadcrumb() {
    let dir = TempDir::new().unwrap();

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    shim(dir.path(), "fuzzel", "exit 0");
    // jiji-activities shim: emit a breadcrumb to stderr on the save dispatch
    // (not on the --version capability probe) and exit 0.
    shim(
        dir.path(),
        "jiji-activities",
        r#"case "$1" in
  save) echo "saved activity layout for acme" >&2 ;;
esac
exit 0"#,
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("save-activity")
        .assert()
        .success()
        .stderr(predicates::str::contains("saved activity layout for acme"));
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

#[test]
fn list_activities_forwards_stdout() {
    let dir = TempDir::new().unwrap();

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    shim(
        dir.path(),
        "jiji-activities",
        r#"case "$1" in
  --version) exit 0 ;;
  list) printf '[{"name":"acme"}]\n' ;;
  *) echo "$@" >> "$argv" ;;
esac"#,
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("list-activities")
        .assert()
        .success()
        // Exact-byte assertion: run_capture returns stdout verbatim (no trim),
        // and print! emits it as-is. A refactor from print! to println! would
        // emit a spurious trailing newline and break `jiji-do list-activities | jq`.
        .stdout(predicates::ord::eq("[{\"name\":\"acme\"}]\n"));
}

/// Upstream-shaped capabilities (no jiji-activities on PATH) → `list-activities`
/// invoked directly exits 69 (capability miss). Mirrors the pattern of
/// `gated_verb_direct_invocation_exits_69_upstream`.
#[test]
fn list_activities_capability_miss_exits_69() {
    let dir = TempDir::new().unwrap();
    // Upstream: niri present but activities read fails; no jiji-activities shim.
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
        .arg("list-activities")
        .assert()
        .code(69)
        .stderr(predicates::str::contains("list-activities"));
}

/// `jiji-activities list` exits non-zero (subprocess failure) → `jiji-do
/// list-activities` exits non-zero (not 0, not 69) and stderr contains the
/// subprocess error message. Mirrors the cancel-vs-failure discipline: subprocess
/// failures must propagate, not be silently swallowed.
#[test]
fn list_activities_subprocess_failure_propagates_nonzero() {
    let dir = TempDir::new().unwrap();

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    // jiji-activities shim: --version exits 0 (capability probe passes),
    // list arm exits non-zero with a message on stderr.
    shim(
        dir.path(),
        "jiji-activities",
        r#"case "$1" in
  --version) exit 0 ;;
  list) echo "compositor unavailable" >&2; exit 1 ;;
  *) exit 0 ;;
esac"#,
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("list-activities")
        .assert()
        .failure()
        // Must NOT be exit 69 (capability miss) — this is a runtime subprocess failure.
        .code(predicates::ord::ne(69))
        .stderr(predicates::str::contains("compositor unavailable"));
}

#[test]
fn menu_does_not_render_list_activities() {
    let dir = TempDir::new().unwrap();
    let stdin_file = dir.path().join("fuzzel_stdin");

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    shim(
        dir.path(),
        "jiji-activities",
        r#"case "$1" in
  --version) exit 0 ;;
  *) exit 0 ;;
esac"#,
    );
    shim(
        dir.path(),
        "fuzzel",
        &format!(
            r#"cat > "{stdin_file}"
exit 1"#,
            stdin_file = stdin_file.display()
        ),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .assert()
        .success(); // exit 1 from fuzzel = user cancel → jiji-do exits 0

    let stdin = std::fs::read_to_string(&stdin_file).unwrap();
    assert!(
        !stdin.contains("List activities"),
        "List activities must not appear in the menu (menu_visible=false), got: {stdin:?}"
    );
    assert!(
        stdin.contains("Switch workspace"),
        "Switch workspace must appear in the menu, got: {stdin:?}"
    );
}

#[test]
fn debug_reports_list_activities_kept_on_fork() {
    let dir = TempDir::new().unwrap();

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    shim(
        dir.path(),
        "jiji-activities",
        r#"case "$1" in
  --version) exit 0 ;;
  *) exit 0 ;;
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
        .stdout(predicates::str::contains("list-activities: kept"));
}

/// Direct-CLI path: `jiji-do create-activity foo` supplies the name as a
/// positional arg and must pass it straight to `jiji-activities create foo`
/// without opening fuzzel. A sabotaged fuzzel shim (exit 99) makes any
/// accidental prompt invocation visible as a test failure.
#[test]
fn create_activity_direct_cli_skips_prompt_and_passes_name() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    // Sabotaged fuzzel: if called, prints to stderr and exits 99 so
    // prompt_name's bail! propagates through main → non-zero exit → .success()
    // assertion fails, making the regression loud.
    shim(
        dir.path(),
        "fuzzel",
        "echo 'fuzzel should not be called' >&2; exit 99",
    );
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
        .args(["create-activity", "foo"])
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.contains(&"create foo"),
        "expected jiji-activities to receive 'create foo', got: {recorded:?}"
    );
}

/// Menu path: `jiji-do create-activity` (no positional) must open fuzzel in
/// free-text mode and pass the typed name to `jiji-activities create <name>`.
#[test]
fn create_activity_menu_path_prompts_and_passes_typed_name() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    // fuzzel shim: drain stdin (simulating free-text prompt) and echo the
    // "typed" name.
    shim(dir.path(), "fuzzel", "cat >/dev/null; echo 'newact'");
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
        .arg("create-activity")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.contains(&"create newact"),
        "expected jiji-activities to receive 'create newact', got: {recorded:?}"
    );
}

/// fuzzel exit-1 (user cancel) during `create-activity` → jiji-do exits 0 and
/// does NOT dispatch to jiji-activities. Only the --version capability-probe
/// line appears in the argv file (no `create` dispatch argv).
#[test]
fn create_activity_cancel_exits_zero_no_dispatch() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    shim(dir.path(), "fuzzel", "exit 1");
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
        .arg("create-activity")
        .assert()
        .success();

    // Only the --version probe must appear; no create dispatch.
    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.iter().all(|l| *l == "--version"),
        "expected only --version probe in argv on cancel, got: {recorded:?}"
    );
}

/// fuzzel success exit with empty stdout (user pressed Enter without typing) →
/// jiji-do exits 0 and does NOT dispatch to jiji-activities. Only the --version
/// probe line appears in the argv file.
#[test]
fn create_activity_empty_prompt_exits_zero_no_dispatch() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    // fuzzel shim: drain stdin, emit empty stdout, exit 0.
    shim(dir.path(), "fuzzel", "cat >/dev/null; printf ''");
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
        .arg("create-activity")
        .assert()
        .success();

    // Only the --version probe must appear; no create dispatch.
    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.iter().all(|l| *l == "--version"),
        "expected only --version probe in argv on empty prompt, got: {recorded:?}"
    );
}

/// Empty-string positional (`jiji-do create-activity ""`) must route to the
/// fuzzel prompt, not dispatch `jiji-activities create ""`. When fuzzel cancels
/// (exit 1), jiji-do exits 0 and only the --version probe appears in the argv
/// file — confirming no `create` dispatch was sent.
///
/// This pins the `.filter(|s| !s.is_empty())` normalization in
/// `verbs/create_activity.rs`: removing that filter would silently dispatch an
/// empty name and break this test.
#[test]
fn create_activity_empty_positional_routes_to_prompt() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    // fuzzel exit 1 = user cancel — clean no-op, exit 0.
    shim(dir.path(), "fuzzel", "exit 1");
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
        .args(["create-activity", ""])
        .assert()
        .success();

    // Only the --version probe must appear; no create dispatch.
    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.iter().all(|l| *l == "--version"),
        "expected only --version probe in argv on empty positional, got: {recorded:?}"
    );
}

/// fuzzel exits non-1 (genuine failure, e.g. display connection error) during
/// `create-activity` → jiji-do exits non-zero (not 0, not 69) and stderr
/// contains "fuzzel failed". This discriminates cancel (exit 1 → clean no-op)
/// from real failure (exit ≥2 → propagated error).
#[test]
fn create_activity_fuzzel_failure_propagates_nonzero() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    shim(dir.path(), "fuzzel", "echo 'display error' >&2; exit 2");
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
        .arg("create-activity")
        .assert()
        .failure()
        .code(predicates::ord::ne(69))
        .stderr(predicates::str::contains("fuzzel failed"));
}

/// Direct-CLI path: `jiji-do remove-activity work` supplies the name as a
/// positional arg and must pass it straight to `jiji-activities remove work`
/// without opening fuzzel. A sabotaged fuzzel shim (exit 99) makes any
/// accidental picker invocation visible as a test failure.
#[test]
fn remove_activity_direct_cli_skips_picker_and_passes_name() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    // Sabotaged fuzzel: if called, prints to stderr and exits 99 so the
    // spawning error propagates through main → non-zero exit → .success()
    // assertion fails, making the regression loud.
    shim(
        dir.path(),
        "fuzzel",
        "echo 'fuzzel should not be called' >&2; exit 99",
    );
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
        .args(["remove-activity", "work"])
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.contains(&"remove work"),
        "expected jiji-activities to receive 'remove work', got: {recorded:?}"
    );
}

/// Menu path: `jiji-do remove-activity` (no positional) reads the activity
/// inventory from `niri msg --json activities`, pipes names into fuzzel, and
/// passes the picked name to `jiji-activities remove <name>`.
#[test]
fn remove_activity_menu_path_picks_and_passes_picked_name() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    // Custom niri shim: returns a two-activity inventory including both active
    // and inactive entries (picker must show all, not filter on is_active).
    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[{"name":"work","is_active":true},{"name":"play","is_active":false}]' ;;
esac"#,
    );
    // fuzzel shim: drain stdin (simulating the candidate list) and echo the
    // "picked" activity name.
    shim(dir.path(), "fuzzel", "cat >/dev/null; echo 'play'");
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
        .arg("remove-activity")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.contains(&"remove play"),
        "expected jiji-activities to receive 'remove play', got: {recorded:?}"
    );
}

/// fuzzel exit-1 (user cancel) during `remove-activity` → jiji-do exits 0 and
/// does NOT dispatch to jiji-activities. Only the --version capability-probe
/// line appears in the argv file.
#[test]
fn remove_activity_cancel_exits_zero_no_dispatch() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[{"name":"work","is_active":true},{"name":"play","is_active":false}]' ;;
esac"#,
    );
    shim(dir.path(), "fuzzel", "cat >/dev/null; exit 1");
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
        .arg("remove-activity")
        .assert()
        .success();

    // Only the --version probe must appear; no remove dispatch.
    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.iter().all(|l| *l == "--version"),
        "expected only --version probe in argv on cancel, got: {recorded:?}"
    );
}

/// Empty-string positional (`jiji-do remove-activity ""`) must route to the
/// fuzzel picker, not dispatch `jiji-activities remove ""`. When fuzzel cancels
/// (exit 1), jiji-do exits 0 and only the --version probe appears in the argv
/// file — confirming no `remove` dispatch was sent.
///
/// This pins the `.filter(|s| !s.is_empty())` normalization in
/// `verbs/remove_activity.rs`: removing that filter would silently dispatch an
/// empty name and break this test.
#[test]
fn remove_activity_empty_positional_routes_to_picker() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[{"name":"work","is_active":true},{"name":"play","is_active":false}]' ;;
esac"#,
    );
    // fuzzel exit 1 = user cancel — clean no-op, exit 0. Drain stdin first to
    // avoid a broken-pipe race on the activity-name list write in `menu::pick_one`.
    shim(dir.path(), "fuzzel", "cat >/dev/null; exit 1");
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
        .args(["remove-activity", ""])
        .assert()
        .success();

    // Only the --version probe must appear; no remove dispatch.
    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.iter().all(|l| *l == "--version"),
        "expected only --version probe in argv on empty positional, got: {recorded:?}"
    );
}

/// `jiji-do switch-activity work` — with a name supplied — must forward exactly
/// `switch work` to `jiji-activities` (no fuzzel invocation). A sabotaged fuzzel
/// shim (exit 99) makes any accidental picker spawn loud.
#[test]
fn switch_activity_with_name_passes_name() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    shim(
        dir.path(),
        "fuzzel",
        "echo 'fuzzel should not be called' >&2; exit 99",
    );
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
        .args(["switch-activity", "work"])
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.contains(&"switch work"),
        "expected jiji-activities to receive 'switch work', got: {recorded:?}"
    );
}

/// `jiji-do switch-activity` without a name — omit-path — must forward exactly
/// `switch` to `jiji-activities`, byte-identical to the pre-Stage-5 behavior.
#[test]
fn switch_activity_without_name_passes_switch_only() {
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
        .arg("switch-activity")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.contains(&"switch"),
        "expected jiji-activities to receive 'switch', got: {recorded:?}"
    );
    // Omit-path must not forward a name argument.
    assert!(
        !lines
            .iter()
            .any(|l| l.starts_with("switch ") && l.len() > "switch".len()),
        "omit-path must not forward a name, got: {recorded:?}"
    );
}

/// `jiji-do move-window-to-activity work` — name supplied — must forward exactly
/// `move-window work --window=<id>` to `jiji-activities` (name before flag).
/// A sabotaged fuzzel shim makes any accidental picker spawn loud.
#[test]
fn move_window_to_activity_with_name_passes_name_then_window() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    shim(
        dir.path(),
        "fuzzel",
        "echo 'fuzzel should not be called' >&2; exit 99",
    );
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
        .args(["move-window-to-activity", "work"])
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    // Assert the exact argv line: name precedes --window flag.
    assert!(
        lines.contains(&"move-window work --window=11"),
        "expected jiji-activities to receive 'move-window work --window=11', got: {recorded:?}"
    );
}

/// `jiji-do move-window-to-activity` without a name — omit-path — must forward
/// exactly `move-window --window=<id>`, byte-identical to the pre-Stage-5 behavior.
#[test]
fn move_window_to_activity_without_name_passes_window_only() {
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

/// `jiji-do move-workspace-to-activity work` — name supplied — must forward
/// exactly `move-workspace work --workspace=<id>` to `jiji-activities` (name
/// before flag). A sabotaged fuzzel shim makes any accidental picker spawn loud.
#[test]
fn move_workspace_to_activity_with_name_passes_name_then_workspace() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    shim(
        dir.path(),
        "fuzzel",
        "echo 'fuzzel should not be called' >&2; exit 99",
    );
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
        .args(["move-workspace-to-activity", "work"])
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    // Assert the exact argv line: name precedes --workspace flag.
    assert!(
        lines.contains(&"move-workspace work --workspace=21"),
        "expected jiji-activities to receive 'move-workspace work --workspace=21', got: {recorded:?}"
    );
}

/// `jiji-do move-workspace-to-activity` without a name — omit-path — must
/// forward exactly `move-workspace --workspace=<id>`, byte-identical to the
/// pre-Stage-5 behavior.
#[test]
fn move_workspace_to_activity_without_name_passes_workspace_only() {
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

/// `jiji-do save-activity backup` — name supplied — must forward exactly
/// `save backup` to `jiji-activities`, even when no activity is focused in the
/// snapshot. This pins the save-asymmetry: the `no focused activity` bail applies
/// ONLY to the omit-path; a supplied name bypasses it entirely.
#[test]
fn save_activity_with_name_passes_save_as() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    // Snapshot has NO focused activity (is_active=false everywhere) — this
    // would cause the omit-path to bail, but the name-supplied path must not.
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
        .args(["save-activity", "backup"])
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    // The supplied name (not the snapshot's acme) must be used.
    assert!(
        lines.contains(&"save backup"),
        "expected jiji-activities to receive 'save backup', got: {recorded:?}"
    );
    // The snapshot-derived name must NOT appear in the dispatch call.
    assert!(
        !lines.contains(&"save acme"),
        "omit-path name 'acme' must not appear when a name was supplied, got: {recorded:?}"
    );
}

/// `jiji-do save-activity` without a name — omit-path — must derive the name
/// from the snapshot's focused activity and forward `save <focused>`, exactly
/// as before Stage 5.
#[test]
fn save_activity_without_name_derives_focused() {
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
    // The niri_body shim sets is_active=true for "acme".
    assert!(
        lines.contains(&"save acme"),
        "expected jiji-activities to receive 'save acme', got: {recorded:?}"
    );
}

/// Empty-string positional (`jiji-do switch-activity ""`) must route to the
/// jiji-activities picker (omit-path: `switch` only), not dispatch
/// `jiji-activities switch ""`. This pins the `.filter(|s| !s.is_empty())`
/// normalization in `verbs/switch_activity.rs`.
#[test]
fn switch_activity_empty_positional_routes_to_omit_path() {
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
        .args(["switch-activity", ""])
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    // Must dispatch bare `switch`, not `switch ` or `switch ""`.
    assert!(
        lines.contains(&"switch"),
        "expected jiji-activities to receive 'switch', got: {recorded:?}"
    );
    assert!(
        !lines
            .iter()
            .any(|l| l.starts_with("switch ") && l.len() > "switch".len()),
        "empty positional must not be forwarded as a name, got: {recorded:?}"
    );
}

/// Empty-string positional (`jiji-do move-window-to-activity ""`) must route to
/// the omit-path (`move-window --window=<id>`), not dispatch
/// `jiji-activities move-window "" --window=<id>`. Focused-window bail still
/// fires first. This pins the `.filter(|s| !s.is_empty())` normalization in
/// `verbs/move_window_to_activity.rs`.
#[test]
fn move_window_to_activity_empty_positional_routes_to_omit_path() {
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
        .args(["move-window-to-activity", ""])
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    // Must dispatch flag-only form, byte-identical to the omit-path.
    assert!(
        lines.contains(&"move-window --window=11"),
        "expected jiji-activities to receive 'move-window --window=11', got: {recorded:?}"
    );
    assert!(
        !lines
            .iter()
            .any(|l| l.contains("move-window  ")
                || (l.contains("move-window") && l.contains("\"\""))),
        "empty positional must not be forwarded as a name, got: {recorded:?}"
    );
}

/// Empty-string positional (`jiji-do move-workspace-to-activity ""`) must route
/// to the omit-path (`move-workspace --workspace=<id>`), not dispatch
/// `jiji-activities move-workspace "" --workspace=<id>`. Focused-workspace bail
/// still fires first. This pins the `.filter(|s| !s.is_empty())` normalization
/// in `verbs/move_workspace_to_activity.rs`.
#[test]
fn move_workspace_to_activity_empty_positional_routes_to_omit_path() {
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
        .args(["move-workspace-to-activity", ""])
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    // Must dispatch flag-only form, byte-identical to the omit-path.
    assert!(
        lines.contains(&"move-workspace --workspace=21"),
        "expected jiji-activities to receive 'move-workspace --workspace=21', got: {recorded:?}"
    );
    assert!(
        !lines.iter().any(|l| l.contains("move-workspace  ")
            || (l.contains("move-workspace") && l.contains("\"\""))),
        "empty positional must not be forwarded as a name, got: {recorded:?}"
    );
}

/// Empty-string positional (`jiji-do save-activity ""`) with a focused activity
/// must route to the derive-from-focused path (`save <focused>`), not dispatch
/// `jiji-activities save ""`. This is the most surprising normalization case:
/// an empty name bypasses the save-as path and falls into the focused-activity
/// derive, which bails if none is focused. Pins the `.filter(|s| !s.is_empty())`
/// normalization in `verbs/save_activity.rs`.
#[test]
fn save_activity_empty_positional_routes_to_focused_derive() {
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
        .args(["save-activity", ""])
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    // Must derive the focused activity ("acme" per niri_body shim), not forward "".
    assert!(
        lines.contains(&"save acme"),
        "expected jiji-activities to receive 'save acme' (derived from focused), got: {recorded:?}"
    );
    assert!(
        !lines.iter().any(|l| *l == "save" || *l == "save "),
        "save must not be dispatched with an empty name, got: {recorded:?}"
    );
}

// ---- reload-config, power-on-monitors, unset-workspace-name shim tests ----

#[test]
fn reload_config_dispatches_action() {
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
        .arg("reload-config")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&actions).unwrap();
    assert!(
        recorded.starts_with("load-config-file"),
        "expected action load-config-file, got: {recorded:?}"
    );
}

#[test]
fn power_on_monitors_dispatches_action() {
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
        .arg("power-on-monitors")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&actions).unwrap();
    assert!(
        recorded.starts_with("power-on-monitors"),
        "expected action power-on-monitors, got: {recorded:?}"
    );
}

/// `unset-workspace-name` must dispatch `niri msg action unset-workspace-name`
/// with no trailing reference arg — the action defaults to the focused workspace.
#[test]
fn unset_workspace_name_dispatches_action_no_reference() {
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
        .arg("unset-workspace-name")
        .assert()
        .success();

    // The shim records the action tail (after `msg action`).
    let recorded = std::fs::read_to_string(&actions).unwrap();
    assert!(
        recorded.starts_with("unset-workspace-name"),
        "expected action unset-workspace-name, got: {recorded:?}"
    );
    let words: Vec<&str> = recorded.split_whitespace().collect();
    assert_eq!(
        words,
        vec!["unset-workspace-name"],
        "unset-workspace-name must carry no trailing reference arg, got: {recorded:?}"
    );
}

// ---- pick-window shim tests ----

/// Happy path: `pick-window` captures the niri stdout and routes it to all
/// three sinks: stdout (unconditional), `wl-copy` (stdin), and `notify-send`
/// (argv). The shims record what they receive; exit 0.
#[test]
fn pick_window_happy_path_routes_to_all_sinks() {
    let dir = TempDir::new().unwrap();
    let notify_argv = dir.path().join("notify_argv");
    let wl_copy_stdin = dir.path().join("wl_copy_stdin");

    // niri shim: answers snapshot probes AND `msg pick-window` → echoes a summary.
    // pick-window is a top-level Request, not an Action: argv is "niri msg pick-window"
    // so $1=msg $2=pick-window $3="". Match on $2 for the pick-* variants.
    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[{"name":"acme","is_active":true}]' ;;
  *) case "$2" in
       "pick-window") echo "Firefox - Main window" ;;
     esac ;;
esac"#,
    );
    // wl-copy shim: record stdin content and exit 0.
    shim(
        dir.path(),
        "wl-copy",
        &format!(
            r#"cat > "{stdin_file}"
exit 0"#,
            stdin_file = wl_copy_stdin.display()
        ),
    );
    // notify-send shim: record argv and exit 0.
    shim(
        dir.path(),
        "notify-send",
        &format!(
            r#"echo "$@" >> "{argv}"
exit 0"#,
            argv = notify_argv.display()
        ),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("pick-window")
        .assert()
        .success()
        // stdout is unconditional — it must carry the summary even though
        // both soft-dep sinks succeeded.
        .stdout(predicates::str::contains("Firefox - Main window"));

    let copied = std::fs::read_to_string(&wl_copy_stdin).unwrap();
    assert!(
        copied.contains("Firefox - Main window"),
        "expected wl-copy to receive the picked window summary via stdin, got: {copied:?}"
    );
    let recorded = std::fs::read_to_string(&notify_argv).unwrap();
    assert!(
        recorded.contains("Firefox - Main window"),
        "expected notify-send to receive the picked window summary, got: {recorded:?}"
    );
    assert!(
        recorded.contains("Picked window"),
        "expected notify-send to receive 'Picked window' as title, got: {recorded:?}"
    );
}

/// Soft-deps-failing path: when both `wl-copy` and `notify-send` exit non-zero
/// (absent or failing), the summary must still reach stdout and the verb must
/// still exit 0. Both are shadowed with shims that exit 1; this exercises
/// `run_best_effort` returning `false` regardless of system PATH contents.
#[test]
fn pick_window_soft_deps_failing_still_prints_to_stdout() {
    let dir = TempDir::new().unwrap();

    // niri shim: answers snapshot probes AND `msg pick-window`.
    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[{"name":"acme","is_active":true}]' ;;
  *) case "$2" in
       "pick-window") echo "Firefox - Main window" ;;
     esac ;;
esac"#,
    );
    // Both soft-dep shims exit 1 — simulates failing clipboard and notifier
    // (daemon not running, or binary absent; PATH shadow makes it deterministic
    // and keeps the test away from the real clipboard).
    shim(dir.path(), "wl-copy", "cat >/dev/null; exit 1");
    shim(dir.path(), "notify-send", "exit 1");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("pick-window")
        .assert()
        .success()
        .stdout(predicates::str::contains("Firefox - Main window"));
}

/// Picker failure: a niri shim that exits non-zero for `pick-window` must make
/// jiji-do exit non-zero. This pins the fail-loud half of the asymmetry: the
/// pick itself must fail loud even though the routing is best-effort.
#[test]
fn pick_window_picker_failure_exits_nonzero() {
    let dir = TempDir::new().unwrap();

    // niri shim: answers snapshot probes normally but fails on pick-window.
    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[{"name":"acme","is_active":true}]' ;;
  *) case "$2" in
       "pick-window") echo "picker cancelled" >&2; exit 1 ;;
     esac ;;
esac"#,
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("pick-window")
        .assert()
        .failure()
        .code(predicates::ord::ne(69));
}

// ---- pick-color shim tests ----

/// Happy path: `pick-color` captures the niri stdout and routes it to all
/// three sinks: stdout (unconditional), `wl-copy` (stdin), and `notify-send`
/// (argv). Exit 0.
#[test]
fn pick_color_happy_path_routes_to_all_sinks() {
    let dir = TempDir::new().unwrap();
    let notify_argv = dir.path().join("notify_argv");
    let wl_copy_stdin = dir.path().join("wl_copy_stdin");

    // niri shim: answers snapshot probes AND `msg pick-color` → echoes a color.
    // pick-color is a top-level Request: argv is "niri msg pick-color" so $2=pick-color.
    // Build the shim body via format! so the '#' in the color value is a plain Rust char
    // rather than something that would terminate a raw-string literal.
    let niri_shim_body = format!(
        concat!(
            "case \"$2 $3\" in\n",
            "  \"--json windows\")    echo '[{{\"id\":11,\"is_focused\":true}}]' ;;\n",
            "  \"--json workspaces\") echo '[{{\"id\":21,\"name\":\"web\",\"output\":\"DP-1\",\"is_focused\":true}}]' ;;\n",
            "  \"--json activities\") echo '[{{\"name\":\"acme\",\"is_active\":true}}]' ;;\n",
            "  *) case \"$2\" in\n",
            "       \"pick-color\") echo \"{color}\" ;;\n",
            "     esac ;;\n",
            "esac"
        ),
        color = "#aabbcc"
    );
    shim(dir.path(), "niri", &niri_shim_body);
    // wl-copy shim: record stdin content and exit 0.
    shim(
        dir.path(),
        "wl-copy",
        &format!(
            r#"cat > "{stdin_file}"
exit 0"#,
            stdin_file = wl_copy_stdin.display()
        ),
    );
    // notify-send shim: record argv and exit 0.
    shim(
        dir.path(),
        "notify-send",
        &format!(
            r#"echo "$@" >> "{argv}"
exit 0"#,
            argv = notify_argv.display()
        ),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("pick-color")
        .assert()
        .success()
        // stdout is unconditional — it must carry the color even though both
        // soft-dep sinks succeeded.
        .stdout(predicates::str::contains("#aabbcc"));

    let copied = std::fs::read_to_string(&wl_copy_stdin).unwrap();
    assert!(
        copied.contains("#aabbcc"),
        "expected wl-copy to receive the color via stdin, got: {copied:?}"
    );
    let notified = std::fs::read_to_string(&notify_argv).unwrap();
    assert!(
        notified.contains("#aabbcc"),
        "expected notify-send to receive the color in argv, got: {notified:?}"
    );
    assert!(
        notified.contains("Picked color"),
        "expected notify-send to receive 'Picked color' as title, got: {notified:?}"
    );
}

/// Both-soft-deps-failing path: when both `wl-copy` and `notify-send` exit
/// non-zero (absent or failing), the color must still reach stdout and the
/// verb must still exit 0. Both are shadowed with shims that exit 1 to make
/// this deterministic regardless of system PATH contents.
#[test]
fn pick_color_both_soft_deps_failing_still_prints_to_stdout() {
    let dir = TempDir::new().unwrap();

    let niri_shim_body = format!(
        concat!(
            "case \"$2 $3\" in\n",
            "  \"--json windows\")    echo '[{{\"id\":11,\"is_focused\":true}}]' ;;\n",
            "  \"--json workspaces\") echo '[{{\"id\":21,\"name\":\"web\",\"output\":\"DP-1\",\"is_focused\":true}}]' ;;\n",
            "  \"--json activities\") echo '[{{\"name\":\"acme\",\"is_active\":true}}]' ;;\n",
            "  *) case \"$2\" in\n",
            "       \"pick-color\") echo \"{color}\" ;;\n",
            "     esac ;;\n",
            "esac"
        ),
        color = "#aabbcc"
    );
    shim(dir.path(), "niri", &niri_shim_body);
    // Both soft-dep shims exit 1 — simulates failing notifier and clipboard.
    shim(dir.path(), "wl-copy", "exit 1");
    shim(dir.path(), "notify-send", "exit 1");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("pick-color")
        .assert()
        .success()
        .stdout(predicates::str::contains("#aabbcc"));
}

/// Picker failure: a niri shim that exits non-zero for `pick-color` must make
/// jiji-do exit non-zero (not 0, not 69). This pins the fail-loud half of the
/// asymmetric contract: the pick itself must fail loud even though routing is
/// best-effort.
#[test]
fn pick_color_picker_failure_exits_nonzero() {
    let dir = TempDir::new().unwrap();

    // niri shim: answers snapshot probes normally but fails on pick-color.
    let niri_shim_body = concat!(
        "case \"$2 $3\" in\n",
        "  \"--json windows\")    echo '[{\"id\":11,\"is_focused\":true}]' ;;\n",
        "  \"--json workspaces\") echo '[{\"id\":21,\"name\":\"web\",\"output\":\"DP-1\",\"is_focused\":true}]' ;;\n",
        "  \"--json activities\") echo '[{\"name\":\"acme\",\"is_active\":true}]' ;;\n",
        "  *) case \"$2\" in\n",
        "       \"pick-color\") echo 'picker cancelled' >&2; exit 1 ;;\n",
        "     esac ;;\n",
        "esac"
    );
    shim(dir.path(), "niri", niri_shim_body);

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("pick-color")
        .assert()
        .failure()
        // Must NOT be exit 69 (capability miss) — this is a picker failure.
        .code(predicates::ord::ne(69));
}

/// Partial-failure diagonal (a): `wl-copy` succeeds but `notify-send` fails.
/// stdout is unconditional — the color must be printed even though the
/// clipboard write succeeded. Exit 0.
///
/// Pins the no-gating rule: stdout is the retrievable home and must never be
/// suppressed by the outcome of any soft-dep sink.
#[test]
fn pick_color_wl_copy_succeeds_notify_fails_still_prints_to_stdout() {
    let dir = TempDir::new().unwrap();

    let niri_shim_body = format!(
        concat!(
            "case \"$2 $3\" in\n",
            "  \"--json windows\")    echo '[{{\"id\":11,\"is_focused\":true}}]' ;;\n",
            "  \"--json workspaces\") echo '[{{\"id\":21,\"name\":\"web\",\"output\":\"DP-1\",\"is_focused\":true}}]' ;;\n",
            "  \"--json activities\") echo '[{{\"name\":\"acme\",\"is_active\":true}}]' ;;\n",
            "  *) case \"$2\" in\n",
            "       \"pick-color\") echo \"{color}\" ;;\n",
            "     esac ;;\n",
            "esac"
        ),
        color = "#aabbcc"
    );
    shim(dir.path(), "niri", &niri_shim_body);
    // wl-copy succeeds; notify-send fails.
    shim(dir.path(), "wl-copy", "cat >/dev/null; exit 0");
    shim(dir.path(), "notify-send", "exit 1");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("pick-color")
        .assert()
        .success()
        // stdout is unconditional — a successful clipboard write must not
        // suppress it.
        .stdout(predicates::str::contains("#aabbcc"));
}

/// Partial-failure diagonal (b): `notify-send` succeeds but `wl-copy` fails.
/// stdout is unconditional, so the color must reach it and the user can still
/// retrieve the value despite the clipboard miss. Exit 0.
#[test]
fn pick_color_notify_succeeds_wl_copy_fails_still_prints_to_stdout() {
    let dir = TempDir::new().unwrap();

    let niri_shim_body = format!(
        concat!(
            "case \"$2 $3\" in\n",
            "  \"--json windows\")    echo '[{{\"id\":11,\"is_focused\":true}}]' ;;\n",
            "  \"--json workspaces\") echo '[{{\"id\":21,\"name\":\"web\",\"output\":\"DP-1\",\"is_focused\":true}}]' ;;\n",
            "  \"--json activities\") echo '[{{\"name\":\"acme\",\"is_active\":true}}]' ;;\n",
            "  *) case \"$2\" in\n",
            "       \"pick-color\") echo \"{color}\" ;;\n",
            "     esac ;;\n",
            "esac"
        ),
        color = "#aabbcc"
    );
    shim(dir.path(), "niri", &niri_shim_body);
    // wl-copy fails; notify-send succeeds.
    shim(dir.path(), "wl-copy", "exit 1");
    shim(dir.path(), "notify-send", "exit 0");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("pick-color")
        .assert()
        .success()
        // wl-copy failed → clipboard route missed → stdout (unconditional)
        // still carries the value.
        .stdout(predicates::str::contains("#aabbcc"));
}

/// Empty activity inventory: `niri msg --json activities` returns `[]` → jiji-do
/// bails before spawning fuzzel (exit 1, NOT 69) with stderr containing
/// "no activities to remove". A sabotaged fuzzel shim makes any accidental
/// spawn visible. Only the --version capability-probe appears in the argv file.
///
/// Pinned by `.code(predicates::ord::ne(69))`: empty-inventory is a runtime data
/// miss (exit 1), not a capability miss (exit 69).
#[test]
fn remove_activity_empty_inventory_bails_before_fuzzel() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[]' ;;
esac"#,
    );
    // Sabotaged fuzzel: if spawned, exits 99 which propagates as a real error —
    // making any accidental fuzzel spawn loud rather than silent.
    shim(dir.path(), "fuzzel", "exit 99");
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
        .arg("remove-activity")
        .assert()
        .failure()
        .code(predicates::ord::ne(69))
        .stderr(predicates::str::contains("no activities to remove"));

    // Only the --version capability probe must appear — no remove dispatch.
    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.iter().all(|l| *l == "--version"),
        "expected only --version probe in argv on empty inventory, got: {recorded:?}"
    );
}

// ---- quit / power-off-monitors / rename-workspace shim tests ----

/// `quit` with a Yes fuzzel confirm dispatches `niri msg action quit
/// --skip-confirmation`. The shim records `$3 $4`; both tokens must appear.
#[test]
fn quit_confirm_yes_dispatches_skip_confirmation() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    // Drain stdin (the No/Yes list), then echo "Yes" — simulates user selecting Yes.
    shim(dir.path(), "fuzzel", "cat >/dev/null; echo 'Yes'");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("quit")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&actions).unwrap();
    let words: Vec<&str> = recorded.split_whitespace().collect();
    assert_eq!(
        words,
        vec!["quit", "--skip-confirmation"],
        "expected tokens ['quit', '--skip-confirmation'], got: {recorded:?}"
    );
}

/// `quit` with a No confirm or cancel (exit 1) must exit 0 without dispatching
/// any action.
#[test]
fn quit_confirm_no_or_cancel_no_dispatch_exit_zero() {
    // Sub-test A: fuzzel echoes "No".
    {
        let dir = TempDir::new().unwrap();
        let actions = dir.path().join("actions");
        shim(
            dir.path(),
            "niri",
            &niri_body(&actions.display().to_string()),
        );
        shim(dir.path(), "fuzzel", "cat >/dev/null; echo 'No'");

        Command::cargo_bin("jiji-do")
            .unwrap()
            .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
            .env("NIRI_SOCKET", "/dummy")
            .arg("quit")
            .assert()
            .success();

        // actions file must not exist (no action dispatched).
        assert!(
            !actions.exists(),
            "no action must be dispatched on No confirm, but actions file appeared"
        );
    }
    // Sub-test B: fuzzel exits 1 (cancel / Escape).
    {
        let dir = TempDir::new().unwrap();
        let actions = dir.path().join("actions");
        shim(
            dir.path(),
            "niri",
            &niri_body(&actions.display().to_string()),
        );
        shim(dir.path(), "fuzzel", "exit 1");

        Command::cargo_bin("jiji-do")
            .unwrap()
            .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
            .env("NIRI_SOCKET", "/dummy")
            .arg("quit")
            .assert()
            .success();

        assert!(
            !actions.exists(),
            "no action must be dispatched on cancel (exit 1), but actions file appeared"
        );
    }
}

/// `power-off-monitors` with a Yes confirm dispatches `niri msg action
/// power-off-monitors`.
#[test]
fn power_off_monitors_confirm_yes_dispatches_action() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    shim(dir.path(), "fuzzel", "cat >/dev/null; echo 'Yes'");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("power-off-monitors")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&actions).unwrap();
    assert!(
        recorded.starts_with("power-off-monitors"),
        "expected action power-off-monitors, got: {recorded:?}"
    );
}

/// `power-off-monitors` with a No confirm or cancel must exit 0 without
/// dispatching any action.
#[test]
fn power_off_monitors_confirm_no_no_dispatch_exit_zero() {
    // Sub-test A: No selected.
    {
        let dir = TempDir::new().unwrap();
        let actions = dir.path().join("actions");
        shim(
            dir.path(),
            "niri",
            &niri_body(&actions.display().to_string()),
        );
        shim(dir.path(), "fuzzel", "cat >/dev/null; echo 'No'");

        Command::cargo_bin("jiji-do")
            .unwrap()
            .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
            .env("NIRI_SOCKET", "/dummy")
            .arg("power-off-monitors")
            .assert()
            .success();

        assert!(
            !actions.exists(),
            "no action must be dispatched on No confirm, but actions file appeared"
        );
    }
    // Sub-test B: cancel (exit 1).
    {
        let dir = TempDir::new().unwrap();
        let actions = dir.path().join("actions");
        shim(
            dir.path(),
            "niri",
            &niri_body(&actions.display().to_string()),
        );
        shim(dir.path(), "fuzzel", "exit 1");

        Command::cargo_bin("jiji-do")
            .unwrap()
            .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
            .env("NIRI_SOCKET", "/dummy")
            .arg("power-off-monitors")
            .assert()
            .success();

        assert!(
            !actions.exists(),
            "no action must be dispatched on cancel (exit 1), but actions file appeared"
        );
    }
}

/// `rename-workspace` with a non-empty prompt response dispatches
/// `niri msg action set-workspace-name <name>`.
#[test]
fn rename_workspace_prompt_dispatches_set_workspace_name() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    // Free-text fuzzel shim: drain stdin (EOF from drop), echo the typed name.
    shim(dir.path(), "fuzzel", "cat >/dev/null; echo 'foo'");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("rename-workspace")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&actions).unwrap();
    let words: Vec<&str> = recorded.split_whitespace().collect();
    assert_eq!(
        words,
        vec!["set-workspace-name", "foo"],
        "expected 'set-workspace-name foo', got: {recorded:?}"
    );
}

/// The rename-workspace prompt identifies the focused workspace from the
/// launch snapshot: per-output idx, name (when set), and stable id. The fuzzel
/// shim records its argv (which carries `--prompt <text>`); the workspaces
/// fixture includes `idx` so all three context pieces are exercised.
#[test]
fn rename_workspace_prompt_names_focused_workspace() {
    let dir = TempDir::new().unwrap();
    let fuzzel_argv = dir.path().join("fuzzel_argv");

    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"idx":2,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[{"name":"acme","is_active":true}]' ;;
esac"#,
    );
    // Record argv, drain stdin, then cancel (exit 1) — the prompt content is
    // what this test pins; no dispatch should follow.
    shim(
        dir.path(),
        "fuzzel",
        &format!(
            r#"echo "$@" >> "{argv}"
cat >/dev/null
exit 1"#,
            argv = fuzzel_argv.display()
        ),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("rename-workspace")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&fuzzel_argv).unwrap();
    assert!(
        recorded.contains(r#"Rename workspace 2 "web" (id 21) to: "#),
        "expected the prompt to name the focused workspace, got: {recorded:?}"
    );
}

/// `rename-workspace` with empty or cancelled prompt must exit 0 without
/// dispatching any action.
#[test]
fn rename_workspace_empty_prompt_no_dispatch_exit_zero() {
    // Sub-test A: empty Enter (success exit, blank stdout).
    {
        let dir = TempDir::new().unwrap();
        let actions = dir.path().join("actions");
        shim(
            dir.path(),
            "niri",
            &niri_body(&actions.display().to_string()),
        );
        shim(dir.path(), "fuzzel", "cat >/dev/null; echo ''");

        Command::cargo_bin("jiji-do")
            .unwrap()
            .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
            .env("NIRI_SOCKET", "/dummy")
            .arg("rename-workspace")
            .assert()
            .success();

        assert!(
            !actions.exists(),
            "no action must be dispatched on empty prompt, but actions file appeared"
        );
    }
    // Sub-test B: cancel (exit 1).
    {
        let dir = TempDir::new().unwrap();
        let actions = dir.path().join("actions");
        shim(
            dir.path(),
            "niri",
            &niri_body(&actions.display().to_string()),
        );
        shim(dir.path(), "fuzzel", "exit 1");

        Command::cargo_bin("jiji-do")
            .unwrap()
            .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
            .env("NIRI_SOCKET", "/dummy")
            .arg("rename-workspace")
            .assert()
            .success();

        assert!(
            !actions.exists(),
            "no action must be dispatched on cancel (exit 1), but actions file appeared"
        );
    }
}

/// `confirm` allowlist is strict: only the exact trimmed text `"Yes"` (capital Y)
/// triggers the affirmative. `"yes"` (lowercase) must be treated as a non-Yes
/// selection → `Ok(false)` → no dispatch, exit 0.
///
/// This pins the `sel == "Yes"` strict-equality guard documented in
/// `menu::confirm`'s rustdoc: any variant that deviates from the exact form must
/// be treated as a no-op.
#[test]
fn quit_confirm_lowercase_yes_no_dispatch_exit_zero() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    // fuzzel echoes "yes" (lowercase) — must NOT be treated as confirmation.
    shim(dir.path(), "fuzzel", "cat >/dev/null; echo 'yes'");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("quit")
        .assert()
        .success();

    // No action must be dispatched — lowercase "yes" is not in the allowlist.
    assert!(
        !actions.exists(),
        "lowercase 'yes' must not trigger dispatch, but actions file appeared"
    );
}

/// fuzzel exits ≥2 (genuine failure, e.g. display connection error) during the
/// `confirm` seam of `quit` → jiji-do exits non-zero and stderr contains
/// "fuzzel failed". The actions file must not exist (no dispatch).
///
/// This is the discriminating test for `confirm`'s cancel-vs-failure shape:
/// under the old all-non-success-→-false pattern, exit 2 would silently become
/// Ok(false) and jiji-do would exit 0 without dispatching. Under the correct
/// shape (only exit 1 → cancel), bail! fires and the error propagates. One test
/// covers the shared `confirm` seam; power-off-monitors shares the code path.
#[test]
fn quit_confirm_fuzzel_failure_propagates_nonzero() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    shim(dir.path(), "fuzzel", "echo 'display error' >&2; exit 2");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("quit")
        .assert()
        .failure()
        .stderr(predicates::str::contains("fuzzel failed"));

    // No action must have been dispatched.
    assert!(
        !actions.exists(),
        "expected no actions on fuzzel failure, but actions file exists"
    );
}

/// fuzzel exits ≥2 (genuine failure) during the `prompt_name` seam of
/// `rename-workspace` → jiji-do exits non-zero and stderr contains "fuzzel
/// failed". The actions file must not exist (no dispatch).
///
/// Mirrors the per-verb fuzzel-failure convention established by
/// `create_activity_fuzzel_failure_propagates_nonzero` and
/// `switch_workspace_fuzzel_failure_exits_nonzero`.
#[test]
fn rename_workspace_fuzzel_failure_propagates_nonzero() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    shim(dir.path(), "fuzzel", "echo 'display error' >&2; exit 2");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("rename-workspace")
        .assert()
        .failure()
        .stderr(predicates::str::contains("fuzzel failed"));

    // No action must have been dispatched.
    assert!(
        !actions.exists(),
        "expected no actions on fuzzel failure, but actions file exists"
    );
}

// ---- Monitor verb shim tests ----

/// `focus-monitor` with a picked output dispatches `niri msg action
/// focus-monitor DP-1`.
#[test]
fn focus_monitor_picks_and_dispatches_action() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    // fuzzel shim: drain stdin (the output label list) and echo the label for DP-1.
    shim(
        dir.path(),
        "fuzzel",
        "cat >/dev/null; echo 'DP-1 (Dell U2720Q)'",
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("focus-monitor")
        .assert()
        .success();

    // The shim records the full action tail (after `msg action`).
    let recorded = std::fs::read_to_string(&actions).unwrap();
    let words: Vec<&str> = recorded.split_whitespace().collect();
    assert_eq!(
        words,
        vec!["focus-monitor", "DP-1"],
        "expected 'focus-monitor DP-1' action, got: {recorded:?}"
    );
}

/// `move-window-to-monitor` with a picked output dispatches
/// `niri msg action move-window-to-monitor DP-1`.
#[test]
fn move_window_to_monitor_picks_and_dispatches_action() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    shim(
        dir.path(),
        "fuzzel",
        "cat >/dev/null; echo 'DP-1 (Dell U2720Q)'",
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("move-window-to-monitor")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&actions).unwrap();
    let words: Vec<&str> = recorded.split_whitespace().collect();
    assert_eq!(
        words,
        vec!["move-window-to-monitor", "DP-1"],
        "expected 'move-window-to-monitor DP-1' action, got: {recorded:?}"
    );
}

/// `move-column-to-monitor` with a picked output dispatches
/// `niri msg action move-column-to-monitor DP-1`.
#[test]
fn move_column_to_monitor_picks_and_dispatches_action() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    shim(
        dir.path(),
        "fuzzel",
        "cat >/dev/null; echo 'DP-1 (Dell U2720Q)'",
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("move-column-to-monitor")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&actions).unwrap();
    let words: Vec<&str> = recorded.split_whitespace().collect();
    assert_eq!(
        words,
        vec!["move-column-to-monitor", "DP-1"],
        "expected 'move-column-to-monitor DP-1' action, got: {recorded:?}"
    );
}

/// `move-workspace-to-monitor` with a picked output dispatches
/// `niri msg action move-workspace-to-monitor DP-1`.
#[test]
fn move_workspace_to_monitor_picks_and_dispatches_action() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    shim(
        dir.path(),
        "fuzzel",
        "cat >/dev/null; echo 'DP-1 (Dell U2720Q)'",
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("move-workspace-to-monitor")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&actions).unwrap();
    let words: Vec<&str> = recorded.split_whitespace().collect();
    assert_eq!(
        words,
        vec!["move-workspace-to-monitor", "DP-1"],
        "expected 'move-workspace-to-monitor DP-1' action, got: {recorded:?}"
    );
}

/// Empty outputs JSON `{}` → any monitor verb bails before fuzzel (exit 1, NOT
/// 69) with stderr containing "no outputs available". A sabotaged fuzzel shim
/// (exit 99) makes any accidental spawn visible as a test failure.
///
/// One test covers all four monitor verbs via `focus-monitor`; the shared
/// `output_choices()` → early bail path is exercised once thoroughly.
#[test]
fn monitor_verb_empty_outputs_bails_before_fuzzel() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");

    // Custom niri shim: returns empty outputs object; other reads are normal.
    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[{"name":"acme","is_active":true}]' ;;
  "--json outputs")    echo '{}' ;;
  *) echo "$3 $4" >> "/dev/null" ;;
esac"#,
    );
    // Sabotaged fuzzel: if spawned, exits 99 which propagates as a real error.
    shim(
        dir.path(),
        "fuzzel",
        "echo 'fuzzel should not be called' >&2; exit 99",
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("focus-monitor")
        .assert()
        .failure()
        .code(predicates::ord::ne(69))
        .stderr(predicates::str::contains("no outputs available"));

    // No action must have been dispatched.
    assert!(
        !actions.exists(),
        "expected no action on empty outputs, but actions file appeared"
    );
}

/// fuzzel exit-1 (user cancel) during `focus-monitor` → jiji-do exits 0 and
/// records no action. Mirrors the cancel-vs-failure contract of
/// `switch_workspace_fuzzel_cancel_exits_zero_no_action`.
#[test]
fn focus_monitor_fuzzel_cancel_exits_zero_no_action() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    shim(dir.path(), "fuzzel", "cat >/dev/null; exit 1");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("focus-monitor")
        .assert()
        .success();

    assert!(
        !actions.exists(),
        "expected no action on fuzzel cancel, but actions file appeared"
    );
}

/// fuzzel exits ≥2 (genuine failure, e.g. display connection error) during
/// `focus-monitor` → jiji-do exits non-zero and stderr contains "fuzzel
/// failed". The actions file must not exist (no dispatch).
///
/// This is the discriminating test for the cancel-vs-failure fix on the
/// monitor verb family: under the old all-non-success-→-None shape, exit 2
/// would silently become Ok(None) and jiji-do would exit 0. Under the correct
/// shape (only exit 1 → cancel), bail! fires. One test covers the shared
/// `menu::pick_one` path; all four monitor verbs exercise it.
#[test]
fn focus_monitor_fuzzel_failure_exits_nonzero() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    // fuzzel shim: drain stdin (avoids broken-pipe race on the output label
    // list write in `menu::pick_one`), then exit 2 to simulate a display error.
    shim(
        dir.path(),
        "fuzzel",
        "cat >/dev/null; echo 'display error' >&2; exit 2",
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("focus-monitor")
        .assert()
        .failure()
        .stderr(predicates::str::contains("fuzzel failed"));

    // No action must have been dispatched.
    assert!(
        !actions.exists(),
        "expected no action on fuzzel failure, but actions file appeared"
    );
}

/// fuzzel echoes a label that matches no output choice (e.g. a stale or
/// corrupted response) → jiji-do exits non-zero and stderr mentions "unknown
/// label". No action must be dispatched.
///
/// Pins the `.ok_or_else(|| anyhow!("picker returned unknown label: …"))` arm
/// in `menu::resolve_by_label`: a future regression to `unwrap_or_default` or
/// silent `Ok(())` would pass all happy-path tests but exit 0 here, making
/// the regression loud.
#[test]
fn focus_monitor_unknown_label_exits_nonzero() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    // fuzzel shim: drain stdin (the label list) and echo a label that doesn't
    // correspond to any output in the snapshot (the shim only knows DP-1).
    shim(dir.path(), "fuzzel", "cat >/dev/null; echo 'bogus'");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("focus-monitor")
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown label"));

    // No action must have been dispatched.
    assert!(
        !actions.exists(),
        "expected no action on unknown label, but actions file appeared"
    );
}

// ---- stop-cast shim tests ----

/// `stop-cast` with a picked session dispatches `niri msg action stop-cast
/// --session-id <id>`. The cast fixture contains two rows sharing one
/// `session_id` (exercises dedup) plus a distinct session.
#[test]
fn stop_cast_picks_and_dispatches_action() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    let actions_str = actions.display().to_string();

    // Custom niri shim: --json casts returns two rows sharing session 7 and
    // one row with session 3 (exercises dedup).
    shim(
        dir.path(),
        "niri",
        &format!(
            r#"case "$2 $3" in
  "--json windows")    echo '[{{"id":11,"is_focused":true}}]' ;;
  "--json workspaces") echo '[{{"id":21,"name":"web","output":"DP-1","is_focused":true}}]' ;;
  "--json activities") echo '[{{"name":"acme","is_active":true}}]' ;;
  "--json outputs")    echo '{{"DP-1":{{"make":"Dell","model":"U2720Q","serial":"","physical_size":{{"w":600,"h":340}},"modes":[],"current_mode":null,"vrr_supported":false,"vrr_enabled":false,"logical":null}}}}' ;;
  "--json casts")      echo '[{{"session_id":7,"stream_id":1,"pid":1234}},{{"session_id":7,"stream_id":2,"pid":1234}},{{"session_id":3,"stream_id":3,"pid":5678}}]' ;;
  *)
    shift 2
    echo "$@" >> "{actions_str}"
    ;;
esac"#
        ),
    );
    // fuzzel shim: drain stdin and echo the label for session 3.
    shim(
        dir.path(),
        "fuzzel",
        "cat >/dev/null; echo 'session 3 (pid 5678)'",
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("stop-cast")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&actions).unwrap();
    let words: Vec<&str> = recorded.split_whitespace().collect();
    assert_eq!(
        words,
        vec!["stop-cast", "--session-id", "3"],
        "expected 'stop-cast --session-id 3', got: {recorded:?}"
    );
}

/// `stop-cast` with an empty casts array bails before spawning fuzzel (exit 1,
/// NOT 69) with stderr containing "no active casts". A sabotaged fuzzel shim
/// makes any accidental spawn visible as a test failure.
#[test]
fn stop_cast_empty_casts_bails_before_fuzzel() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");

    // Custom niri shim: returns empty casts array.
    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[{"name":"acme","is_active":true}]' ;;
  "--json outputs")    echo '{"DP-1":{"make":"Dell","model":"U2720Q","serial":"","physical_size":{"w":600,"h":340},"modes":[],"current_mode":null,"vrr_supported":false,"vrr_enabled":false,"logical":null}}' ;;
  "--json casts")      echo '[]' ;;
  *) echo "$3 $4" >> "/dev/null" ;;
esac"#,
    );
    // Sabotaged fuzzel: if spawned, exits 99 which propagates as a real error.
    shim(
        dir.path(),
        "fuzzel",
        "echo 'fuzzel should not be called' >&2; exit 99",
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("stop-cast")
        .assert()
        .failure()
        .code(predicates::ord::ne(69))
        .stderr(predicates::str::contains("no active casts"));

    // No action must have been dispatched.
    assert!(
        !actions.exists(),
        "expected no action on empty casts, but actions file appeared"
    );
}

/// fuzzel exit-1 (user cancel) during `stop-cast` → jiji-do exits 0 and
/// records no action.
///
/// The fuzzel shim drains stdin before exiting to avoid a broken-pipe race on
/// the candidate list write in `menu::pick_one`.
#[test]
fn stop_cast_fuzzel_cancel_exits_zero_no_action() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    let actions_str = actions.display().to_string();

    // Custom niri shim with --json casts support.
    shim(
        dir.path(),
        "niri",
        &format!(
            r#"case "$2 $3" in
  "--json windows")    echo '[{{"id":11,"is_focused":true}}]' ;;
  "--json workspaces") echo '[{{"id":21,"name":"web","output":"DP-1","is_focused":true}}]' ;;
  "--json activities") echo '[{{"name":"acme","is_active":true}}]' ;;
  "--json outputs")    echo '{{"DP-1":{{"make":"Dell","model":"U2720Q","serial":"","physical_size":{{"w":600,"h":340}},"modes":[],"current_mode":null,"vrr_supported":false,"vrr_enabled":false,"logical":null}}}}' ;;
  "--json casts")      echo '[{{"session_id":7,"stream_id":1,"pid":1234}}]' ;;
  *)
    shift 2
    echo "$@" >> "{actions_str}"
    ;;
esac"#
        ),
    );
    shim(dir.path(), "fuzzel", "cat >/dev/null; exit 1");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("stop-cast")
        .assert()
        .success();

    assert!(
        !actions.exists(),
        "expected no action on fuzzel cancel, but actions file appeared"
    );
}

/// fuzzel exits ≥2 (genuine failure, e.g. display connection error) during
/// `stop-cast` → jiji-do exits non-zero and stderr contains "fuzzel failed".
/// The actions file must not exist (no dispatch).
///
/// This discriminates cancel (exit 1 → clean no-op) from failure (exit ≥2 →
/// propagate error). Mirrors `focus_monitor_fuzzel_failure_exits_nonzero`.
#[test]
fn stop_cast_fuzzel_failure_propagates_nonzero() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    let actions_str = actions.display().to_string();

    // Custom niri shim with --json casts support.
    shim(
        dir.path(),
        "niri",
        &format!(
            r#"case "$2 $3" in
  "--json windows")    echo '[{{"id":11,"is_focused":true}}]' ;;
  "--json workspaces") echo '[{{"id":21,"name":"web","output":"DP-1","is_focused":true}}]' ;;
  "--json activities") echo '[{{"name":"acme","is_active":true}}]' ;;
  "--json outputs")    echo '{{"DP-1":{{"make":"Dell","model":"U2720Q","serial":"","physical_size":{{"w":600,"h":340}},"modes":[],"current_mode":null,"vrr_supported":false,"vrr_enabled":false,"logical":null}}}}' ;;
  "--json casts")      echo '[{{"session_id":7,"stream_id":1,"pid":1234}}]' ;;
  *)
    shift 2
    echo "$@" >> "{actions_str}"
    ;;
esac"#
        ),
    );
    // fuzzel shim: drain stdin (avoids broken-pipe race on the candidate list
    // write), then exit 2 to simulate a display connection error.
    shim(
        dir.path(),
        "fuzzel",
        "cat >/dev/null; echo 'display error' >&2; exit 2",
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("stop-cast")
        .assert()
        .failure()
        .stderr(predicates::str::contains("fuzzel failed"));

    // No action must have been dispatched.
    assert!(
        !actions.exists(),
        "expected no action on fuzzel failure, but actions file appeared"
    );
}

// ---- rename-activity shim tests ----

/// Happy path: picker shim returns a target activity, prompt shim returns a new
/// name → `jiji-activities rename <new-name> --activity <target>` is dispatched
/// with all four tokens in that exact order.
///
/// Uses two fuzzel invocations. The first fuzzel call is `pick_one` (candidate
/// list piped to stdin); drain + echo the target. The second is `prompt_name`
/// (free-text mode); drain + echo the new name. A single shim cannot discriminate
/// the two calls reliably, so a counter file tracks which invocation is current.
#[test]
fn rename_activity_happy_path_dispatches_rename_with_correct_argv() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");
    let call_count = dir.path().join("fuzzel_calls");

    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[{"name":"work","is_active":true},{"name":"play","is_active":false}]' ;;
  "--json outputs")    echo '{"DP-1":{"make":"Dell","model":"U2720Q","serial":"","physical_size":{"w":600,"h":340},"modes":[],"current_mode":null,"vrr_supported":false,"vrr_enabled":false,"logical":null}}' ;;
esac"#,
    );
    // First fuzzel call → pick_one (drain the candidate list, return target).
    // Second fuzzel call → prompt_name (drain, return new name).
    shim(
        dir.path(),
        "fuzzel",
        &format!(
            r#"count=$(cat "{count}" 2>/dev/null || echo 0)
echo $((count + 1)) > "{count}"
cat >/dev/null
if [ "$count" = "0" ]; then echo 'work'; else echo 'renamed-work'; fi"#,
            count = call_count.display()
        ),
    );
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
        .arg("rename-activity")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    // The dispatch line must contain exactly: rename renamed-work --activity work
    // in that token order. echo "$@" in sh emits all args space-separated.
    assert!(
        lines.contains(&"rename renamed-work --activity work"),
        "expected jiji-activities to receive 'rename renamed-work --activity work', got: {recorded:?}"
    );
}

/// The rename-activity target picker lists activities in MRU order: sorted by
/// `last_active_seq` descending, so the current activity (unique maximum) is
/// the preselected first fuzzel row. The fuzzel shim records the candidate
/// list it receives on stdin; the fixture's inventory order differs from MRU
/// order to prove the sort happened.
#[test]
fn rename_activity_picker_rows_are_mru_ordered() {
    let dir = TempDir::new().unwrap();
    let stdin_file = dir.path().join("fuzzel_stdin");

    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[{"name":"default","is_active":false,"last_active_seq":2},{"name":"work","is_active":true,"last_active_seq":7},{"name":"play","is_active":false,"last_active_seq":5}]' ;;
esac"#,
    );
    // Record the candidate list, then cancel — the row order is what this
    // test pins; no dispatch should follow.
    shim(
        dir.path(),
        "fuzzel",
        &format!(
            r#"cat > "{stdin_file}"
exit 1"#,
            stdin_file = stdin_file.display()
        ),
    );
    // Present only for the --version capability probe (NIRI_ACTIVITIES);
    // the cancel path must never dispatch to it.
    shim(dir.path(), "jiji-activities", "exit 0");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("rename-activity")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&stdin_file).unwrap();
    let rows: Vec<&str> = recorded.lines().collect();
    assert_eq!(
        rows,
        vec!["work", "play", "default"],
        "expected MRU order (active first, then by recency), got: {recorded:?}"
    );
}

/// Target picker cancel (fuzzel exit 1 on first call) → no rename dispatched,
/// jiji-do exits 0. The fuzzel shim drains stdin before exiting to avoid a
/// broken-pipe race on the activity-name list write in `menu::pick_one`.
#[test]
fn rename_activity_target_picker_cancel_exits_zero_no_dispatch() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[{"name":"work","is_active":true},{"name":"play","is_active":false}]' ;;
  "--json outputs")    echo '{"DP-1":{"make":"Dell","model":"U2720Q","serial":"","physical_size":{"w":600,"h":340},"modes":[],"current_mode":null,"vrr_supported":false,"vrr_enabled":false,"logical":null}}' ;;
esac"#,
    );
    // Drain stdin then cancel (exit 1) — must not reach prompt_name or dispatch.
    shim(dir.path(), "fuzzel", "cat >/dev/null; exit 1");
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
        .arg("rename-activity")
        .assert()
        .success();

    // Only the --version capability probe must appear; no rename dispatch.
    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.iter().all(|l| *l == "--version"),
        "expected only --version probe in argv on picker cancel, got: {recorded:?}"
    );
}

/// Name prompt cancel/empty: picker returns a target but `prompt_name` cancels
/// (second fuzzel call exits 1) → no rename dispatched, jiji-do exits 0.
/// The prompt fuzzel shim drains stdin before exiting.
#[test]
fn rename_activity_name_prompt_cancel_exits_zero_no_dispatch() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");
    let call_count = dir.path().join("fuzzel_calls");

    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[{"name":"work","is_active":true},{"name":"play","is_active":false}]' ;;
  "--json outputs")    echo '{"DP-1":{"make":"Dell","model":"U2720Q","serial":"","physical_size":{"w":600,"h":340},"modes":[],"current_mode":null,"vrr_supported":false,"vrr_enabled":false,"logical":null}}' ;;
esac"#,
    );
    // First call: drain + return target "work"; second call: drain + cancel (exit 1).
    shim(
        dir.path(),
        "fuzzel",
        &format!(
            r#"count=$(cat "{count}" 2>/dev/null || echo 0)
echo $((count + 1)) > "{count}"
cat >/dev/null
if [ "$count" = "0" ]; then echo 'work'; else exit 1; fi"#,
            count = call_count.display()
        ),
    );
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
        .arg("rename-activity")
        .assert()
        .success();

    // Only the --version capability probe must appear; no rename dispatch.
    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.iter().all(|l| *l == "--version"),
        "expected only --version probe in argv on prompt cancel, got: {recorded:?}"
    );
}

/// Empty activity inventory: `niri msg --json activities` returns `[]` → jiji-do
/// bails before spawning fuzzel (exit 1, NOT 69) with stderr containing
/// "no activities to rename". A sabotaged fuzzel shim (exit 99) makes any
/// accidental spawn visible as a test failure. Only the --version capability-probe
/// appears in the argv file.
///
/// Pinned by `.code(predicates::ord::ne(69))`: empty-inventory is a runtime data
/// miss (exit 1), not a capability miss (exit 69).
#[test]
fn rename_activity_empty_inventory_bails_before_fuzzel() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[]' ;;
esac"#,
    );
    // Sabotaged fuzzel: if spawned, exits 99 which propagates as a real error —
    // making any accidental fuzzel spawn loud rather than silent.
    shim(dir.path(), "fuzzel", "exit 99");
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
        .arg("rename-activity")
        .assert()
        .failure()
        .code(predicates::ord::ne(69))
        .stderr(predicates::str::contains("no activities to rename"));

    // Only the --version capability probe must appear — no rename dispatch.
    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.iter().all(|l| *l == "--version"),
        "expected only --version probe in argv on empty inventory, got: {recorded:?}"
    );
}

/// Picker hard-failure (fuzzel exit ≥2) during `rename-activity` → jiji-do exits
/// non-zero (not 0, not 69) and stderr contains "fuzzel failed". This discriminates
/// cancel (exit 1 → clean no-op) from genuine failure (exit ≥2 → propagate).
/// The fuzzel shim drains stdin before exiting to avoid a broken-pipe race.
#[test]
fn rename_activity_picker_failure_propagates_nonzero() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");

    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[{"name":"work","is_active":true},{"name":"play","is_active":false}]' ;;
  "--json outputs")    echo '{"DP-1":{"make":"Dell","model":"U2720Q","serial":"","physical_size":{"w":600,"h":340},"modes":[],"current_mode":null,"vrr_supported":false,"vrr_enabled":false,"logical":null}}' ;;
esac"#,
    );
    // Drain stdin (avoids broken-pipe race), then exit 2 to simulate display error.
    shim(
        dir.path(),
        "fuzzel",
        "cat >/dev/null; echo 'display error' >&2; exit 2",
    );
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
        .arg("rename-activity")
        .assert()
        .failure()
        .code(predicates::ord::ne(69))
        .stderr(predicates::str::contains("fuzzel failed"));
}

/// Name-prompt hard-failure (second fuzzel call exits ≥2) during `rename-activity`
/// → jiji-do exits non-zero (not 0, not 69) and stderr contains "fuzzel failed".
/// This discriminates cancel (exit 1 → clean no-op) from genuine failure (exit ≥2
/// → propagate). The first fuzzel call (picker) succeeds; the second (prompt_name)
/// fails. Both shim calls drain stdin to avoid a broken-pipe race.
#[test]
fn rename_activity_name_prompt_failure_propagates_nonzero() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");
    let call_count = dir.path().join("fuzzel_calls");

    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[{"name":"work","is_active":true},{"name":"play","is_active":false}]' ;;
  "--json outputs")    echo '{"DP-1":{"make":"Dell","model":"U2720Q","serial":"","physical_size":{"w":600,"h":340},"modes":[],"current_mode":null,"vrr_supported":false,"vrr_enabled":false,"logical":null}}' ;;
esac"#,
    );
    // First call: drain + return target; second call: drain + exit 2 (hard failure).
    shim(
        dir.path(),
        "fuzzel",
        &format!(
            r#"count=$(cat "{count}" 2>/dev/null || echo 0)
echo $((count + 1)) > "{count}"
cat >/dev/null
if [ "$count" = "0" ]; then echo 'work'; else echo 'display error' >&2; exit 2; fi"#,
            count = call_count.display()
        ),
    );
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
        .arg("rename-activity")
        .assert()
        .failure()
        .code(predicates::ord::ne(69))
        .stderr(predicates::str::contains("fuzzel failed"));

    // Only the --version capability probe must appear; no rename dispatch.
    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.iter().all(|l| *l == "--version"),
        "expected only --version probe in argv on prompt hard-failure, got: {recorded:?}"
    );
}

/// Blank Enter on name prompt (second fuzzel call exits 0 with empty stdout) →
/// no rename dispatched, jiji-do exits 0. Mirrors the blank-Enter sub-case of
/// `rename_workspace_empty_prompt_no_dispatch_exit_zero`: `prompt_name` treats
/// empty stdout (after trimming) as Ok(None) → clean no-op, exit 0.
/// Both fuzzel calls drain stdin to avoid a broken-pipe race.
#[test]
fn rename_activity_blank_name_prompt_no_dispatch_exit_zero() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("argv");
    let call_count = dir.path().join("fuzzel_calls");

    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[{"name":"work","is_active":true},{"name":"play","is_active":false}]' ;;
  "--json outputs")    echo '{"DP-1":{"make":"Dell","model":"U2720Q","serial":"","physical_size":{"w":600,"h":340},"modes":[],"current_mode":null,"vrr_supported":false,"vrr_enabled":false,"logical":null}}' ;;
esac"#,
    );
    // First call: drain + return target; second call: drain + echo blank line (exit 0).
    shim(
        dir.path(),
        "fuzzel",
        &format!(
            r#"count=$(cat "{count}" 2>/dev/null || echo 0)
echo $((count + 1)) > "{count}"
cat >/dev/null
if [ "$count" = "0" ]; then echo 'work'; else echo ''; fi"#,
            count = call_count.display()
        ),
    );
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
        .arg("rename-activity")
        .assert()
        .success();

    // Only the --version capability probe must appear; no rename dispatch.
    let recorded = std::fs::read_to_string(&argv_file).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(
        lines.iter().all(|l| *l == "--version"),
        "expected only --version probe in argv on blank name prompt, got: {recorded:?}"
    );
}

/// `jiji-activities rename` exits non-zero → `jiji-do rename-activity` exits
/// non-zero (not 0, not 69) and stderr contains "jiji-activities exited".
/// The picker and name-prompt shims succeed; only the delegate subprocess fails.
/// Pins the subprocess-failure propagation guarantee for the rename dispatch leg.
#[test]
fn rename_activity_delegate_failure_propagates_nonzero() {
    let dir = TempDir::new().unwrap();
    let call_count = dir.path().join("fuzzel_calls");

    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json activities") echo '[{"name":"work","is_active":true},{"name":"play","is_active":false}]' ;;
  "--json outputs")    echo '{"DP-1":{"make":"Dell","model":"U2720Q","serial":"","physical_size":{"w":600,"h":340},"modes":[],"current_mode":null,"vrr_supported":false,"vrr_enabled":false,"logical":null}}' ;;
esac"#,
    );
    // Picker returns "work"; name prompt returns "renamed-work".
    shim(
        dir.path(),
        "fuzzel",
        &format!(
            r#"count=$(cat "{count}" 2>/dev/null || echo 0)
echo $((count + 1)) > "{count}"
cat >/dev/null
if [ "$count" = "0" ]; then echo 'work'; else echo 'renamed-work'; fi"#,
            count = call_count.display()
        ),
    );
    // Capability probe exits 0; rename dispatch fails with an error message.
    shim(
        dir.path(),
        "jiji-activities",
        r#"case "$1" in
  --version) exit 0 ;;
  rename) echo 'rename failed' >&2; exit 1 ;;
  *) exit 0 ;;
esac"#,
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("rename-activity")
        .assert()
        .failure()
        .code(predicates::ord::ne(69))
        .stderr(predicates::str::contains("jiji-activities exited"));
}

/// `--debug` with full capabilities shows `rename-activity: kept`. Mirrors the
/// pattern used for other Stage 7 activity-passthrough verbs.
#[test]
fn debug_reports_rename_activity_kept_on_fork() {
    let dir = TempDir::new().unwrap();

    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    shim(
        dir.path(),
        "jiji-activities",
        r#"case "$1" in
  --version) exit 0 ;;
  *) exit 0 ;;
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
        .stdout(predicates::str::contains("rename-activity: kept"));
}

// ---- switch-workspace-all shim tests ----

/// `switch-workspace-all` dispatches `niri msg action focus-workspace
/// --activity <name> id:<ws-id>` atomically for a dormant-activity row.
#[test]
fn switch_workspace_all_dispatches_atomic_focus() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    // Canned data: ws id 23 named "mail" belongs to activity id 2 ("home").
    shim(dir.path(), "fuzzel", "cat >/dev/null; echo 'home: mail'");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("switch-workspace-all")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&actions).unwrap();
    assert!(
        recorded.contains("focus-workspace --activity home id:23"),
        "expected atomic focus-workspace --activity home id:23, got: {recorded:?}"
    );
}

/// Same-activity row also dispatches the atomic form (--activity + id:N),
/// not a plain focus-workspace.
#[test]
fn switch_workspace_all_same_activity_row_also_atomic() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    // Canned data: ws id 21 named "web" belongs to activity id 1 ("acme").
    // It is the focused workspace so it carries " (current)" in the all-rows label.
    shim(
        dir.path(),
        "fuzzel",
        "cat >/dev/null; echo 'acme: web (current)'",
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("switch-workspace-all")
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&actions).unwrap();
    assert!(
        recorded.contains("focus-workspace --activity acme id:21"),
        "expected atomic focus-workspace --activity acme id:21, got: {recorded:?}"
    );
}

/// fuzzel exit-1 (user cancel) during `switch-workspace-all` → jiji-do exits
/// 0 and records no action.
#[test]
fn switch_workspace_all_cancel_exits_zero_no_action() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    shim(dir.path(), "fuzzel", "cat >/dev/null; exit 1");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("switch-workspace-all")
        .assert()
        .success();

    assert!(
        !actions.exists(),
        "expected no action on fuzzel cancel, but actions file appeared"
    );
}

/// Without FORK (activities read fails on upstream compositor),
/// `switch-workspace-all` must be absent from the menu and must exit 69
/// when invoked directly.
#[test]
fn switch_workspace_all_gated_on_fork() {
    let dir = TempDir::new().unwrap();
    let stdin_file = dir.path().join("fuzzel_stdin");

    // Upstream: activities read fails → FORK capability absent.
    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json activities") exit 1 ;;
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[{"id":21,"idx":1,"name":"web","output":"DP-1","is_focused":true}]' ;;
  "--json outputs")    echo '{"DP-1":{"make":"Dell","model":"U2720Q","serial":"","physical_size":{"w":600,"h":340},"modes":[],"current_mode":null,"vrr_supported":false,"vrr_enabled":false,"logical":null}}' ;;
esac"#,
    );
    // Record stdin then cancel — checks which verbs appear in the menu.
    shim(
        dir.path(),
        "fuzzel",
        &format!(
            r#"cat > "{stdin_file}"
exit 1"#,
            stdin_file = stdin_file.display()
        ),
    );

    // Menu must NOT contain "Switch workspace (all activities)".
    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .assert()
        .success();

    let menu_stdin = std::fs::read_to_string(&stdin_file).unwrap();
    assert!(
        !menu_stdin.contains("Switch workspace (all activities)"),
        "switch-workspace-all must not appear in the menu when FORK is absent, got: {menu_stdin:?}"
    );

    // Direct invocation must exit 69.
    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("switch-workspace-all")
        .assert()
        .code(69)
        .stderr(predicates::str::contains("switch-workspace-all"));
}

/// Empty workspace inventory → bail before spawning fuzzel (exit 1, NOT 69).
/// The FORK capability probe (activities read) must still succeed.
/// A canary fuzzel shim makes any accidental spawn visible.
#[test]
fn switch_workspace_all_empty_inventory_bails_before_fuzzel() {
    let dir = TempDir::new().unwrap();
    let canary = dir.path().join("fuzzel_canary");

    // FORK probe must succeed (activities returns []) but workspaces are empty.
    shim(
        dir.path(),
        "niri",
        r#"case "$2 $3" in
  "--json windows")    echo '[{"id":11,"is_focused":true}]' ;;
  "--json workspaces") echo '[]' ;;
  "--json activities") echo '[]' ;;
  "--json outputs")    echo '{}' ;;
esac"#,
    );
    // Canary fuzzel: if spawned, write a file and exit 0 so the test can detect it.
    shim(
        dir.path(),
        "fuzzel",
        &format!(
            r#"touch "{canary}"
exit 0"#,
            canary = canary.display()
        ),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("switch-workspace-all")
        .assert()
        .failure()
        .code(predicates::ord::ne(69))
        .stderr(predicates::str::contains("no workspaces found"));

    assert!(
        !canary.exists(),
        "fuzzel must not be spawned on empty inventory, but canary file appeared"
    );
}

/// fuzzel exits ≥2 (genuine failure, e.g. display connection error) during
/// `switch-workspace-all` → jiji-do exits non-zero and stderr contains
/// "fuzzel failed". No action must be dispatched. The fuzzel shim drains stdin
/// before exiting to avoid a broken-pipe race on the row-list write in
/// `menu::pick_one` (same EPIPE discipline as
/// `switch_workspace_fuzzel_failure_exits_nonzero`).
///
/// This discriminates cancel (exit 1 → exit 0) from genuine failure (exit ≥2
/// → propagate error): under the old all-non-success-→-None shape, exit 2
/// would silently become Ok(None) and jiji-do would exit 0.
#[test]
fn switch_workspace_all_fuzzel_failure_exits_nonzero() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    // Drain stdin (avoids broken-pipe race on the row-list write in
    // `menu::pick_one`), then exit 2 to simulate a display connection error.
    shim(
        dir.path(),
        "fuzzel",
        "cat >/dev/null; echo 'display error' >&2; exit 2",
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("switch-workspace-all")
        .assert()
        .failure()
        .stderr(predicates::str::contains("fuzzel failed"));

    // No focus-workspace action must have been dispatched.
    assert!(
        !actions.exists(),
        "expected no actions on fuzzel failure, but actions file exists"
    );
}

/// Pin the exact row order: MRU groups, current activity last, focused row
/// marked. The canned niri_body data: acme (active, seq=9) holds web (focused,
/// id=21) and DP-1 #22 (unnamed, id=22); home (inactive, seq=4) holds mail
/// (id=23). Expected: home rows first (lower MRU, non-active), then acme rows
/// last (active group), web marked with "(current)".
#[test]
fn switch_workspace_all_renders_rows_grouped_current_last() {
    let dir = TempDir::new().unwrap();
    let stdin_file = dir.path().join("fuzzel_stdin");
    shim(
        dir.path(),
        "niri",
        &niri_body(&dir.path().join("actions").display().to_string()),
    );
    // Record stdin then cancel — row order is what this test pins.
    shim(
        dir.path(),
        "fuzzel",
        &format!(
            r#"cat > "{stdin_file}"
exit 1"#,
            stdin_file = stdin_file.display()
        ),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .arg("switch-workspace-all")
        .assert()
        .success(); // cancel → exit 0

    let recorded = std::fs::read_to_string(&stdin_file).unwrap();
    let rows: Vec<&str> = recorded.lines().collect();
    assert_eq!(
        rows,
        vec!["home: mail", "acme: web (current)", "acme: DP-1 #22",],
        "expected home rows first (non-active MRU group), then acme (active group last), got: {recorded:?}"
    );
}

#[test]
fn list_workspaces_prints_current_activity_names() {
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
        .arg("list-workspaces")
        .assert()
        .success()
        // Named workspaces of the active activity only: web. The unnamed
        // #22 row is omitted (no typeable reference); dormant mail is out.
        .stdout("web\n");
}

#[test]
fn list_workspaces_activity_flag_lists_that_activity() {
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
        .args(["list-workspaces", "--activity", "home"])
        .assert()
        .success()
        .stdout("mail\n");
}

#[test]
fn list_workspaces_unknown_activity_exits_1() {
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
        .args(["list-workspaces", "--activity", "nope"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("unknown activity"));
}

#[test]
fn switch_workspace_with_arg_dispatches_verbatim_without_fuzzel() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    let fuzzel_marker = dir.path().join("fuzzel-ran");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    // Sentinel shim: records that fuzzel spawned, then drains stdin (EPIPE
    // discipline) and picks a row — if the passthrough path wrongly opens
    // the picker, the marker file appears AND the dispatched argv changes.
    shim(
        dir.path(),
        "fuzzel",
        &format!(
            "touch {}; cat >/dev/null; echo 'web'",
            fuzzel_marker.display()
        ),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .args(["switch-workspace", "mail"])
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&actions).unwrap();
    assert!(
        recorded.contains("focus-workspace mail"),
        "expected verbatim passthrough, got: {recorded:?}"
    );
    assert!(
        !fuzzel_marker.exists(),
        "fuzzel must not spawn when a workspace reference is supplied"
    );
}

#[test]
fn switch_workspace_whitespace_arg_falls_back_to_picker() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    shim(dir.path(), "fuzzel", "cat >/dev/null; echo 'web'");

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .args(["switch-workspace", "   "])
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&actions).unwrap();
    assert!(
        recorded.contains("focus-workspace web"),
        "whitespace-only arg must route to the picker, got: {recorded:?}"
    );
}

#[test]
fn switch_workspace_all_with_both_args_dispatches_directly() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    let fuzzel_marker = dir.path().join("fuzzel-ran");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    // Note: the `cat >/dev/null` drain and echo are unreachable — fuzzel must
    // NOT spawn on the both-args path. The marker file is the live assertion.
    shim(
        dir.path(),
        "fuzzel",
        &format!("touch {}", fuzzel_marker.display()),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .args(["switch-workspace-all", "home", "mail"])
        .assert()
        .success();

    let recorded = std::fs::read_to_string(&actions).unwrap();
    assert!(
        recorded.contains("focus-workspace --activity home mail"),
        "expected direct activity-scoped dispatch, got: {recorded:?}"
    );
    assert!(
        !fuzzel_marker.exists(),
        "fuzzel must not spawn when both references are supplied"
    );
}

#[test]
fn switch_workspace_all_activity_only_filters_picker() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    let fuzzel_input = dir.path().join("fuzzel-input");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    // Record what the picker was offered, then pick the home row.
    shim(
        dir.path(),
        "fuzzel",
        &format!("cat > {}; echo 'home: mail'", fuzzel_input.display()),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .args(["switch-workspace-all", "home"])
        .assert()
        .success();

    let offered = std::fs::read_to_string(&fuzzel_input).unwrap();
    assert!(
        offered.contains("home: mail"),
        "filtered picker must offer home's rows, got: {offered:?}"
    );
    assert!(
        !offered.contains("acme:"),
        "filtered picker must not offer other activities' rows, got: {offered:?}"
    );
    let recorded = std::fs::read_to_string(&actions).unwrap();
    assert!(
        recorded.contains("focus-workspace --activity home id:23"),
        "picked row must dispatch via its id reference, got: {recorded:?}"
    );
}

#[test]
fn switch_workspace_all_unknown_activity_bails_before_fuzzel() {
    let dir = TempDir::new().unwrap();
    let actions = dir.path().join("actions");
    let fuzzel_marker = dir.path().join("fuzzel-ran");
    shim(
        dir.path(),
        "niri",
        &niri_body(&actions.display().to_string()),
    );
    shim(
        dir.path(),
        "fuzzel",
        &format!(
            "touch {}; cat >/dev/null; echo 'home: mail'",
            fuzzel_marker.display()
        ),
    );

    Command::cargo_bin("jiji-do")
        .unwrap()
        .env("PATH", format!("{}:/bin:/usr/bin", dir.path().display()))
        .env("NIRI_SOCKET", "/dummy")
        .args(["switch-workspace-all", "nope"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("no workspaces found in activity"));

    assert!(
        !fuzzel_marker.exists(),
        "empty filtered inventory must bail before fuzzel spawns"
    );
}
