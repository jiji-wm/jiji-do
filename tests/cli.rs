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

#[test]
fn missing_socket_exits_69() {
    // With no NIRI_SOCKET the socket gate fires before verb resolution → 69.
    // (This also covers the "no env" path generically; a dedicated
    // unknown-verb-with-socket test lives in tests/shims.rs.)
    Command::cargo_bin("jiji-do")
        .unwrap()
        .env_remove("NIRI_SOCKET")
        .arg("definitely-not-a-verb")
        .assert()
        .code(69);
}
