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

### Curation discipline

Two principles govern verb-registry additions:

1. **Curate, don't enumerate**. The compositor has ~80+
   `Action` variants. Most are continuous gestures or animation primitives
   that don't read as menu entries. Adding a verb to `REGISTRY` is a
   deliberate UX call requiring DD justification, not a mechanical reflex
   to a new IPC variant.
2. **Exclude verbs already on muscle-memory keybinds** in the standard
   niri config (`~/.local/share/chezmoi/dot_config/niri/config.kdl.tmpl`,
   ratified 2026-05-28). A launcher menu entry that
   duplicates a one-key shortcut is dead weight — the user reaches for
   the key, not the menu. The launcher's value lives in (a) discovery
   verbs without obvious keybinds, (b) picker-based verbs that need a
   fuzzel choice, (c) infrequent/debug verbs, (d) Stage 3 activities
   passthrough (no keybinds exist).

A proposed verb must clear both gates. Restore candidates for previously
cut verbs live in the cut-verb candidates section in `docs/design.md`
with current-keybind annotations — restoring requires a new rationale
or evidence the keybind has been removed.

## Naming

The compositor msg binary is resolved once per process by `proc::msg_bin()`:
`$JIJI_MSG_BIN` override (set-but-empty = unset) → `jiji` on `$PATH` →
`niri` fallback. Never hardcode `"niri"` at a dispatch site — a post-rename
system may carry a stale `niri` binary whose CLI parser lags the live
compositor (capability probing succeeds against the socket, but newer flags
die locally at clap parse time). The compositor exports both `$JIJI_SOCKET`
and `$NIRI_SOCKET`, so either binary reaches the live instance.

## Git

Follow the global `~/CLAUDE.md` commit conventions: `Review-Needed:` +
`AI-Assisted:` trailers; never `Co-Authored-By`; never push without request.

## Workspace

This repo is developed as part of the jiji-wm workspace, which carries the
cross-repo process and design docs. The owning design doc is `docs/design.md`;
DD and code commits both land in this repo.
