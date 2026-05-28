# jiji-do — Stage 1 design (skeleton + capability detection)

**Date:** 2026-05-27
**Status:** approved 2026-05-27 (design + written spec). This file is the **owning DD**, derived from the approved spec at `../../docs/superpowers/specs/2026-05-27-jiji-do-stage1-design.md` (which stays as the historical artifact). §1–11 below are the approved design verbatim; §12 has been rewritten for the unified jiji loop, and an `## Implementation phases` section has been appended as the loop-drivable ledger.
**Initiative:** `../../docs/launcher/initiative.md` (this is Stage 1; the launcher's end-state, capability model, dependency contract, and CLI surface live there)
**Name:** `jiji-do` — ratified 2026-05-27 (the §10 working-name decision; `jiji-cmd` / `jiji-palette` rejected)

## 1. Purpose

`jiji-do` is a Helix-style command launcher for the jiji desktop. One keybind opens a fuzzel menu of every actionable verb; each verb is also a top-level subcommand so direct keybindings skip the menu. It captures focused window/workspace/activity *once* at process start — before any picker grabs keyboard focus — and dispatches the snapshotted ids. It runs on upstream niri with a reduced verb set and fully on the jiji fork + `jiji-activities`.

**Stage 1 goal:** prove the wiring end-to-end with two pilot verbs and the capability-gating machinery. No verb-coverage breadth (that is Stage 2). This spec is the design for that first slice.

## 2. Scope

### In scope (Stage 1)
- Repo skeleton: `Cargo.toml`, `LICENSE`, `CLAUDE.md`, `docs/design.md` (the eventual home of this design), `src/main.rs`, `src/cli.rs`, `tests/`.
- Capability prober → a `Capabilities` bitflags value.
- `Snapshot` type built once at startup.
- Verb registry: a small struct per verb (label, category, required capabilities, dispatch fn).
- Two pilot verbs: `switch-workspace` (native) and `switch-activity` (passthrough, gated).
- Menu entry point (`jiji-do` no-arg) listing **enabled** verbs only; per-verb subcommand dispatch.
- `--debug` listing which verbs were filtered and why.
- Integration tests under `assert_cmd` + a sentinel-picker shim.

### Out of scope (Stage 1 — deferred)
- Verb-category menu ordering (**deferred to Stage 2** — only two verbs exist now; ordering is meaningless at n=2). Stage 1 lists verbs in registration order.
- The broad curated verb set (~20–30 verbs) — Stage 2.
- `jiji-activities` passthrough breadth — Stage 3.
- README / fish completions / packaging polish — Stage 4.
- Any `niri-ipc` Cargo dependency, daemon mode, D-Bus, plugin manifests, chord input, picker theming — Stage 5+ parking lot per the initiative.

## 3. §10 ratification decisions (resolved here)

1. **Final name:** `jiji-do`.
2. **Verb-category menu ordering:** deferred to Stage 2 (registration order for Stage 1).
3. **Snapshot caching across menu→verb dispatch:** the snapshot is read **once** at process start and passed by reference into the verb dispatch fn. Dispatch never re-reads compositor state. This is the core value of snapshot-at-launch: a picker stealing focus mid-flow cannot invalidate the captured ids.
4. **Per-verb capability degradation messaging:** invoking a capability-gated verb directly in an unsupported environment (e.g. `jiji-do switch-activity` on upstream niri, or with `jiji-activities` absent) prints a one-line stderr diagnostic naming the missing capability and exits **69**. The menu path simply hides disabled verbs (they never appear, so never error). `--debug` is the diagnostic surface for *why* a verb was hidden.

## 4. Architecture

Four units, each with one purpose, a defined interface, and independent testability.

### 4.1 Capability prober (`src/capabilities.rs`)
- Produces a `Capabilities` bitflags value. Flags: `NIRI_SOCKET`, `FUZZEL`, `FORK`, `NIRI_ACTIVITIES`. (`ROFI` reserved for a future multi-select verb; not probed in Stage 1 since no verb needs it.)
- Probes (each independent, no dispatch side effects):
  - `NIRI_SOCKET` — `$NIRI_SOCKET` set **and** a connect succeeds.
  - `FUZZEL` — `fuzzel` resolves on `$PATH` (same `$PATH`-walk discipline as jiji-activities' picker availability check).
  - `FORK` — `niri msg --json activities` returns the activities-shaped response (upstream niri lacks the subcommand / returns an error or an unrecognized shape).
  - `NIRI_ACTIVITIES` — `jiji-activities` resolves on `$PATH` **and** `jiji-activities --version` exits 0.
- Interface: `fn probe() -> Capabilities`. Depends on: env, `$PATH`, one `niri msg` subprocess, one `which`-style lookup.

### 4.2 Snapshot (`src/snapshot.rs`)
- `struct Snapshot { focused_window: Option<u64>, focused_workspace: Option<u64>, focused_output: Option<String>, focused_activity: Option<String> }`.
- Built once via `niri msg --json windows`, `... workspaces`, and (only if `FORK`) `... activities`, parsed into **minimal local serde structs** (only the fields read — not a re-export of niri-ipc types). This duplication is intentional and is the dependency-contract price (§4.5).
- Interface: `fn capture(caps: Capabilities) -> Result<Snapshot>`. The activity field is `None` on upstream. Each focused-* field is `None` when nothing is focused.

### 4.3 Verb registry (`src/registry.rs` + `src/verbs/`)
- A verb is a value: `struct Verb { name: &'static str, label: &'static str, category: Category, requires: Capabilities, dispatch: fn(&Snapshot) -> Result<()> }`.
- `Category` enum (Workspace / Activity / … ) exists from Stage 1 but is only used for grouping in Stage 2; Stage 1 ignores it for ordering.
- The registry is a static list of `Verb`s. `enabled(caps)` filters to verbs whose `requires` is satisfied by `caps`.
- A verb dispatches **only** if `caps.contains(verb.requires)` — enforced centrally at the dispatch boundary, not per-verb. Tests assert no verb can dispatch with unmet capabilities.

### 4.4 Dispatch + menu (`src/main.rs`, `src/cli.rs`, `src/menu.rs`)

**Snapshot is captured first, before any picker (including the menu's own fuzzel) opens.** Non-negotiable: opening fuzzel steals keyboard focus and changes what "focused" means, so a snapshot taken after the menu would capture fuzzel's state, not the user's. Order in both paths: probe capabilities → capture snapshot → then menu-or-direct-dispatch.

- `jiji-do` (no arg): requires `FUZZEL`; captures the snapshot, then renders a fuzzel menu of `enabled(caps)` verbs (label text); on selection, calls the verb's dispatch fn **with the already-captured snapshot**. (Menu does **not** re-exec `jiji-do <verb>` — it calls the same dispatch path inline, so both entry points share one test fixture and avoid double-process cost.)
- `jiji-do <verb>`: parses to the verb; if `caps` unmet → stderr diagnostic + exit 69 (§3.4); else capture snapshot, dispatch.
- `--debug`: prints the resolved `Capabilities` and, per verb, `kept` or `filtered (missing: X)`. Stays out of stderr on normal runs.

## 5. Pilot verbs

- **`switch-workspace`** (native; `requires = NIRI_SOCKET | FUZZEL`): fuzzel over the workspace list (from the snapshot / a `niri msg --json workspaces` read), then `niri msg action focus-workspace <ref>`.
- **`switch-activity`** (passthrough; `requires = NIRI_SOCKET | FUZZEL | FORK | NIRI_ACTIVITIES`): shells out to `jiji-activities switch` (which runs its own picker). Hidden on upstream / when `jiji-activities` is missing.

These two were chosen because they exercise both verb kinds (native dispatch via `niri msg`; passthrough via `jiji-activities`) and the full capability-gate spread (`switch-activity` requires all four flags; `switch-workspace` only two).

## 6. CLI surface

```sh
jiji-do                  # fuzzel menu of enabled verbs
jiji-do switch-workspace # direct dispatch (native)
jiji-do switch-activity  # direct dispatch (passthrough, gated)
jiji-do --debug          # capability + filter report
```

clap-derive, mirroring jiji-activities' structure (clap + anyhow). Per-verb subcommands are generated from the registry where practical, or kept in sync with it (the registry is the single source of truth for which verbs exist).

## 7. Capability model & exit codes

| Condition | Exit | Behavior |
|---|---|---|
| `$NIRI_SOCKET` unset / connect fails | 69 | stderr `"niri socket unavailable: …"`; nothing works without it. |
| `fuzzel` missing, picker verb or menu invoked | 69 | stderr `"picker unavailable: fuzzel not on $PATH"`. Immediate-dispatch verbs (none in Stage 1) would still work. |
| gated verb invoked directly, capability unmet | 69 | stderr names the missing capability (e.g. `"switch-activity requires the jiji fork + jiji-activities"`). |
| normal dispatch | 0 | — |

The binary **never errors on missing deps in menu mode** — it filters the verb out. Direct invocation of an unavailable verb is the only path that exits 69 on a capability miss.

## 8. Dependency contract (load-bearing — pin in `jiji-do` CLAUDE.md)

1. **No `niri-ipc` Cargo dep.** All compositor interaction is via `niri msg` subprocesses.
2. **No `jiji-activities` Cargo dep.** Activities verbs are subprocess passthroughs.
3. **All snapshots are JSON over `niri msg --json …`**, parsed with `serde_json` against minimal local structs (only the read fields).

These three are what let one binary run on both upstream niri and the jiji fork. A lint/grep test should assert `Cargo.toml` gains no `niri-ipc` dep.

## 9. Testing

- `assert_cmd` integration tests + a sentinel-picker shim (a tempdir `#!/bin/sh` script named `fuzzel`, env-clear + scoped `$PATH`), the same pattern jiji-activities uses.
- Cover: (a) both pilot verbs dispatch correctly under a sentinel picker; (b) the menu lists only enabled verbs for a given capability set; (c) a gated verb invoked directly with unmet capabilities exits 69 with the right stderr; (d) `--debug` reports kept/filtered correctly; (e) capability-gate enforcement — no verb dispatches with unmet capabilities.
- Capability probing is faked in tests by injecting a `Capabilities` value rather than touching the real environment (keeps tests hermetic; avoids the env-var-mutex isolation hazard seen in jiji-activities' `ipc::tests`).

## 10. Naming note (pre-compositor-rename)

This spec uses `niri msg` / `$NIRI_SOCKET` per the initiative's §11 glossary. The compositor exports both `$JIJI_SOCKET` and `$NIRI_SOCKET` today, so `niri msg` works. These become `jiji msg` / `$JIJI_SOCKET` only when the compositor source-rename sub-phase lands (deferred, Phase D-adjacent). `jiji-do` itself takes the `jiji-` name from the start (greenfield, no rename debt).

## 11. Exit criteria (Stage 1)

- Repo skeleton in place; builds.
- Capability prober, `Snapshot`, registry, both pilot verbs, menu, `--debug` all implemented.
- Integration test suite (§9) green.
- `Cargo.toml` carries no `niri-ipc` / `jiji-activities` dep (contract test passes).
- One round of loop review per the unified jiji loop (`/jiji:land-subphase jiji-do`).

## 12. Process follow-ups (rewritten for the unified jiji loop)

The launcher initiative originally anticipated a dedicated `jiji-do` sub-agent loop. The 2026-05-28 loop unification (`../../docs/superpowers/plans/2026-05-28-jiji-loop-unification.md`) replaced the per-project loops with one role-based agent set that routes by language via `loops.conf`, so bootstrapping jiji-do now costs **one registry row and zero new agents** (it is Rust → `jiji-rust-implementer`):

- This design moved to `repos/jiji-do/docs/design.md` as the owning DD, carrying a Phase 1.0 design-ratification gate (mirrors the jiji-activities ratification pattern) — see `## Implementation phases` below.
- ~~Bootstrap a `jiji-do` sub-agent loop (`.claude/agents/jiji-do-{architect,implementer,fixer,scribe}.md` + `.claude/commands/jiji-do/`).~~ **Obsolete under the unified loop.** Replaced by a single workspace `loops.conf` row: `jiji-do|rust|repos/jiji-do|repos/jiji-do/docs/design.md|repos/jiji-do`. The scope-discipline additions the initiative flagged (curate-don't-enumerate, capability-gate completeness, no-`niri-ipc`-link) live as per-codebase discipline in this repo's `CLAUDE.md`, read by `jiji-rust-implementer`.

---

## Implementation phases

The loop-drivable ledger. `/jiji:land-subphase jiji-do` (architect → implementer → review → fixer → scribe) lands these boxes; the architect scans from the topmost unchecked `[ ]` and groups consecutive qualifying boxes into a landing unit.

**Step recipes** for each implementation phase below live in `../../docs/superpowers/plans/2026-05-27-jiji-do-stage1.md` — **Phase 1.N corresponds to that plan's Task N** (file layout, exact code blocks, commit shape). That plan's Task 12 (per-loop bootstrap) is obsolete under the unified loop and is dropped.

### Phase 1.0 — Design ratification

- [x] **§3 decisions ratified** (name `jiji-do`; ordering deferred to Stage 2; snapshot-once semantics; capability-miss = exit 69). Ratified 2026-05-27 in the approved spec; re-affirmed in this owning DD. *(Pre-checked — design was approved pre-loop; no in-loop ratification needed.)*

### Phase 1.1 — Repo skeleton (plan Task 1)

- [x] `Cargo.toml` (no `niri-ipc`), `LICENSE`, `CLAUDE.md` (dependency contract + implementer discipline), `docs/design.md` (this DD), `src/main.rs` placeholder, `.gitignore`. *(Landed in the 2026-05-28 bootstrap.)*

### Phase 1.2 — Subprocess helpers (plan Task 2)

- [ ] `src/proc.rs`: `run_capture(cmd, args)` and `which(bin)` thin subprocess helpers.

### Phase 1.3 — Capabilities (plan Task 3)

- [ ] `src/capabilities.rs`: `Capabilities` bitflags + `probe()` (the four Stage 1 flags per §4.1).

### Phase 1.4 — Snapshot (plan Task 4)

- [ ] `src/snapshot.rs`: `Snapshot` + minimal serde briefs + pure `from_json(...)` + `capture(caps)` (per §4.2; minimal local structs, no `niri-ipc`).

### Phase 1.5 — Verb registry (plan Task 5)

- [ ] `src/registry.rs`: `Category`, `Verb`, the static verb list, `enabled(caps)` (per §4.3; central capability-gate enforcement).

### Phase 1.6 — `niri` action helper + `switch-workspace` (plan Task 6)

- [ ] `src/niri.rs` action dispatch + `src/verbs/switch_workspace.rs` native verb (fuzzel over workspaces → `niri msg action focus-workspace`).

### Phase 1.7 — `switch-activity` passthrough (plan Task 7)

- [ ] `src/verbs/switch_activity.rs`: passthrough verb shelling to `jiji-activities switch`, gated on all four capability flags.

### Phase 1.8 — Error / exit-code model (plan Task 8)

- [ ] `src/error.rs`: `DoError` enum + `exit_code()` (69 for capability misses; per §7).

### Phase 1.9 — CLI + dispatch + menu wiring + `--debug` (plan Task 9)

- [ ] `src/cli.rs` (clap `Cli`/`Cmd`), `src/menu.rs` (`pick_one`), `src/main.rs` (probe → snapshot → menu-or-dispatch → exit-code map), `--debug` report (per §4.4).

### Phase 1.10 — Contract test: no `niri-ipc` dependency (plan Task 10)

- [ ] `tests/cli.rs::no_niri_ipc_dependency`: grep `Cargo.toml` asserts no `niri-ipc` / `jiji-activities` dep (enforces §8).

### Phase 1.11 — End-to-end shim tests (plan Task 11)

- [ ] `tests/shims.rs`: `$PATH`-scoped shim harness exercising the menu + both verbs + capability filtering + exit-69 path (per §9).
