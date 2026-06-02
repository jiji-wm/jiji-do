//! Surface + contract integration tests.

use assert_cmd::Command;
use predicates::prelude::*;

/// Dependency contract rules 1 + 2: jiji-do must never gain niri-ipc or
/// jiji-activities Cargo deps.
#[test]
fn no_forbidden_dependencies() {
    let manifest = include_str!("../Cargo.toml");
    assert!(
        !manifest.contains("niri-ipc"),
        "Cargo.toml must not depend on niri-ipc (dependency contract rule 1)"
    );
    assert!(
        !manifest.contains("jiji-activities"),
        "Cargo.toml must not depend on jiji-activities (dependency contract rule 2)"
    );
}

#[test]
fn version_flag_exits_zero() {
    Command::cargo_bin("jiji-do")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("jiji-do"));
}

/// A valid registry verb with no socket available must exit 69 (capability
/// miss), not a clap parse error. Pins that the socket gate fires after
/// successful parse.
#[test]
fn missing_socket_exits_69() {
    Command::cargo_bin("jiji-do")
        .unwrap()
        .env_remove("NIRI_SOCKET")
        .arg("switch-activity")
        .assert()
        .code(69);
}

/// An unrecognised subcommand must be rejected by clap at parse time → exit 2.
#[test]
fn invalid_verb_exits_2() {
    Command::cargo_bin("jiji-do")
        .unwrap()
        .arg("definitely-not-a-verb")
        .assert()
        .code(2);
}

/// `completions fish` must exit 0 and emit non-empty output. This path returns
/// before the capability probe — no shims for fuzzel / niri / jiji-activities
/// are needed.
#[test]
fn completions_fish_exits_zero_with_content() {
    Command::cargo_bin("jiji-do")
        .unwrap()
        .args(["completions", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

/// `completions bash` must enumerate registered verb names.
#[test]
fn completions_bash_contains_registered_verbs() {
    Command::cargo_bin("jiji-do")
        .unwrap()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("switch-activity"));
}

/// `completions fish` must enumerate registered verb names. This is the
/// integration proof that modelling verbs as subcommands fixes the fish
/// completion gap: the fish generator emits subcommand-tree completions
/// and now includes verb names.
#[test]
fn completions_fish_contains_registered_verbs() {
    Command::cargo_bin("jiji-do")
        .unwrap()
        .args(["completions", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("switch-activity"));
}
