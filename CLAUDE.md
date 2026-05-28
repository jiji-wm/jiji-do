# CLAUDE.md

Operational guidance for Claude Code in the `jiji-do` repo.

## What this is

`jiji-do` is a Helix-style command launcher for the jiji wayland compositor.
One fuzzel menu lists every capability-enabled verb; each verb is also a
top-level subcommand. See `docs/design.md` (the owning DD).

## Build & test

```sh
cargo +nightly fmt --all      # before every commit
cargo test                    # all tests
cargo clippy --all --all-targets   # zero-warning baseline
```

## Dependency contract (LOAD-BEARING — do not violate)

1. **No `niri-ipc` Cargo dep.** All compositor interaction is via `niri msg`.
2. **No `jiji-activities` Cargo dep.** Activities verbs are subprocess passthroughs.
3. **Snapshots are JSON over `niri msg --json …`**, parsed into minimal local
   serde structs (only the fields read).

A grep test (`tests/cli.rs::no_forbidden_dependencies`) enforces rules 1 and 2.

## Implementer discipline (read by jiji-rust-implementer)

- **`assert_cmd` rigor.** End-to-end CLI behavior is tested via `assert_cmd` with
  `$PATH`-scoped shim executables (a tempdir `#!/bin/sh` `fuzzel` / `jiji-activities`
  / `niri`) — never by mutating the real environment or talking to a live compositor.
- **Capability probing is faked in tests** by injecting a `Capabilities` value, not
  by touching the real environment — keeps tests hermetic and avoids the env-var-mutex
  isolation hazard seen in jiji-activities' `ipc::tests`.
- **Exit-code consistency.** Capability misses on direct verb invocation map to exit
  `69`; normal dispatch is `0`. Pin these in tests; a changed exit code without a test
  update is a stop-and-report condition.

## Naming

Use `niri msg` / `$NIRI_SOCKET` (the compositor exports both `$JIJI_SOCKET`
and `$NIRI_SOCKET`). The rename to `jiji msg` is deferred with the compositor
source rename.

## Git

Follow the global `~/CLAUDE.md` commit conventions: `Review-Needed:` +
`AI-Assisted:` trailers; never `Co-Authored-By`; never push without request.

## Loop integration

This repo is a target of the unified jiji loop (workspace `loops.conf` row
`jiji-do|rust|repos/jiji-do|repos/jiji-do/docs/design.md|repos/jiji-do`). Drive
sub-phases with `/jiji:land-subphase jiji-do` (architect → implementer → review →
fixer → scribe); the DD at `docs/design.md` is the owning ledger. The DD and code
commits both land in this repo (`dd_commit_repo` = this repo).
