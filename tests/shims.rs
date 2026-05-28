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
        .stdout(predicates::str::contains("switch-workspace: kept"));
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
