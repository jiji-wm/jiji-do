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

- [x] `src/proc.rs`: `run_capture(cmd, args)` and `which(bin)` thin subprocess helpers. Landed in `13c2f31`.

### Phase 1.3 — Capabilities (plan Task 3)

- [x] `src/capabilities.rs`: `Capabilities` bitflags + `probe()` (the four Stage 1 flags per §4.1). Landed in `4b425b9`.

### Phase 1.4 — Snapshot (plan Task 4)

- [x] `src/snapshot.rs`: `Snapshot` + minimal serde briefs + pure `from_json(...)` + `capture(caps)` (per §4.2; minimal local structs, no `niri-ipc`). Landed in `6b122da`.

### Phase 1.5 — Verb registry (plan Task 5)

- [x] `src/registry.rs`: `Category`, `Verb`, the static verb list, `enabled(caps)` (per §4.3; central capability-gate enforcement). Landed in `10d5f01`.

### Phase 1.6 — `niri` action helper + `switch-workspace` (plan Task 6)

- [x] `src/niri.rs` action dispatch + `src/verbs/switch_workspace.rs` native verb (fuzzel over workspaces → `niri msg action focus-workspace`). Landed in `b17cd23`.

### Phase 1.7 — `switch-activity` passthrough (plan Task 7)

- [x] `src/verbs/switch_activity.rs`: passthrough verb shelling to `jiji-activities switch`, gated on all four capability flags. Landed in `411c634`.

### Phase 1.8 — Error / exit-code model (plan Task 8)

- [x] `src/error.rs`: `DoError` enum + `exit_code()` (69 for capability misses; per §7). Landed in `5b1d546`.

### Phase 1.9 — CLI + dispatch + menu wiring + `--debug` (plan Task 9)

- [x] `src/cli.rs` (clap `Cli`/`Cmd`), `src/menu.rs` (`pick_one`), `src/main.rs` (probe → snapshot → menu-or-dispatch → exit-code map), `--debug` report (per §4.4). Landed in `308f286`.

### Phase 1.10 — Contract test: no `niri-ipc` dependency (plan Task 10)

- [x] `tests/cli.rs::no_forbidden_dependencies`: grep `Cargo.toml` asserts no `niri-ipc` / `jiji-activities` dep (enforces §8). Landed in `9208348`. *(Test renamed from `no_niri_ipc_dependency` — now also asserts no `jiji-activities` dep.)*

### Phase 1.11 — End-to-end shim tests (plan Task 11)

- [x] `tests/shims.rs`: `$PATH`-scoped shim harness exercising the menu + both verbs + capability filtering + exit-69 path (per §9). Landed in `d655290`; follow-up `a0eaccc` fixes the fuzzel cancel-vs-failure silent-failure and adds a discriminating regression test.

**Reviewed:** 2026-05-28 (`13c2f31`, `4b425b9`, `6b122da`, `10d5f01`, `b17cd23`, `411c634`, `5b1d546`, `308f286`, `9208348`, `d655290`, `a0eaccc`). Phases 1.2–1.11 in ten feature commits plus one review follow-up, baseline 0 → 23 tests (14 unit + 3 cli + 6 shims). Reviewed across code quality, silent-failure surface, test coverage, and dependency-contract enforcement. **Finding worth surfacing (fuzzel cancel-vs-failure silent-failure, `src/menu.rs`):** the initial `pick_one` implementation mapped every non-success exit from fuzzel to a clean `Ok(None)` cancel — signal kills, Wayland display errors, and other genuine failures were silently swallowed as user-cancel. Fixed in `a0eaccc` by discriminating fuzzel exit-1 (conventional cancel) from all other non-zero or signal-terminated exits; the latter now bail with the exit code and captured stderr. A shim test pins the contract: fuzzel exit-1 during `switch-workspace` → `jiji-do` exits 0, no `focus-workspace` dispatched. Reusable correctness lesson: subprocess launchers must discriminate the picker's conventional cancel code from other failures rather than collapsing all non-success. **Finding worth surfacing (argv-shim accounting for probe invocations):** the capability probe runs `jiji-activities --version` before verb dispatch; argv-recording shim tests must account for this invocation when asserting the complete argv record, or the probe call is silently untracked. The end-to-end shims in `tests/shims.rs` were authored with this in mind after the fixer surfaced the pattern. **DD also amended in this commit:** Phase 1.10 test name corrected from `no_niri_ipc_dependency` → `no_forbidden_dependencies` (also asserts no `jiji-activities` dep); five deferred findings recorded in new Appendix C. All §11 exit criteria satisfied; the §12 Task-12 obsolescence (per-loop agent scaffolding, superseded by the unified loop) was pre-folded at DD authoring and is void by design — no exit-criterion gap. Post-review fixes squashed into `a0eaccc`: `src/menu.rs` fuzzel-cancel discrimination + stderr capture + BrokenPipe attribution; discriminating shim regression test added. Same 23 tests green (14 unit + 3 cli + 6 shims); `cargo clippy --all --all-targets` zero warnings; `cargo +nightly fmt --all` clean. Stage 1 complete; proceed to Stage 2 (curated verb set + category-grouped menu ordering + jiji-activities passthrough breadth) with the reviewed base.

### Phase 2.0 — Stage 2 design ratification (human gate)

Stage 2 begins implementation only after a human ratifies the three design decisions below. The decisions are UX/curation calls that fall outside the architect's lane (`docs/launcher/initiative.md` §3 explicitly pins "curate, don't enumerate" as a deliberate UX call; §10 leaves verb-category ordering as an open question deferred from Stage 1 ratification). Each box below is a `**Proposed:**` gate — ratify in-place by flipping `[ ]` → `[x]`, optionally with an amendment note. Drafts below are the architect's recommended defaults; nothing is decided until a box is flipped.

- [x] **Proposed: Verb roster for Stage 2 (native compositor verbs, no fork dep).** The set below is the architect's draft, lifted from `docs/launcher/initiative.md` §4 Stage 2 with one cut. Ratify the list (or amend in-place: strike entries to drop them, add `+ <verb>` lines to extend, add a `(deferred to Stage 4)` note to park).

  **Amended 2026-05-28 (human ratification + curation-principle clarification).** A second curation principle is added on top of "curate, don't enumerate": **exclude verbs that are already on muscle-memory keybinds in the standard niri config.** Rationale: a launcher menu entry that duplicates a one-key shortcut is dead weight — the user reaches for the key, not the menu. The launcher's value lives in (a) discovery verbs without obvious keybinds, (b) picker-based verbs that need a fuzzel choice (already covered by Stage 1's `switch-workspace`/`switch-activity`), (c) infrequent/debug verbs, and (d) Stage 3 activities passthrough (no keybinds exist). Ground truth for "muscle-memory keybind" is `~/.local/share/chezmoi/dot_config/niri/config.kdl.tmpl` — survey done against that config at amendment time.

  Surviving roster after applying the principle (2 verbs):
  - **Workspace nav** (Category::Workspace):
    - `focus-workspace-previous` — immediate; `niri msg action focus-workspace-previous`. Requires `NIRI_SOCKET`. The previous-workspace toggle is **not** bound in the standard config (line 649 is commented out) and is the only nav verb where a menu entry adds value.
  - **Mode toggles** (Category::Mode — new):
    - `toggle-debug-tint` — immediate; `niri msg action toggle-debug-tint`. Requires `NIRI_SOCKET`. Debug/diagnostic verb, no keybind, infrequent use — classic discovery candidate.

  Cuts from initiative §4 Stage 2 — original draft (`focus-window`, `close-window-by-fuzzel`, `move-window-to-monitor`/`move-column-to-monitor`, `move-window-to-workspace`/`move-column-to-workspace`, `toggle-workspace-sticky`) plus the amendment cuts (`focus-workspace-up`, `focus-workspace-down`, `close-window`, `fullscreen-window`, `toggle-window-floating`, `center-focused-column`/`center-column`, `toggle-overview`, `toggle-keyboard-shortcuts-inhibit`). Rationale for amendment cuts: all bound to Mod-prefixed keys in the standard config — `focus-workspace-up/down` to `Mod+PgUp/Dn`, `Mod+K/J`, `Mod+WheelUp/Dn`; `close-window` to `Mod+Q`; `fullscreen-window` to `Mod+Shift+F`; `toggle-window-floating` to `Mod+Semicolon`; `center-column` to `Mod+C` (note: original draft used `center-focused-column` which is the wrong action name — that's a config setting, not an action; the bound action is `center-column`); `toggle-overview` to `Mod+O`; `toggle-keyboard-shortcuts-inhibit` to `Mod+Escape` (with `allow-inhibiting=false` so it works even when inhibit is active).

  **Outcome of ratifying this box (as amended):** Phase 2.1 lands the entire surviving roster as **one** coarse sub-phase — both verbs share the immediate-dispatch shape, registry shape, and shim-test shape; splitting them is over-fragmentation. Phase 2.1 also wires the `Category` variant additions (Workspace already exists; Mode is new) and drops the `#[allow(dead_code)]` on `Category`. The full set of cuts moves to a new Appendix D (deferred Stage 2 verbs) so future sessions don't re-litigate and a future amendment can restore from there with rationale. The launcher's center of gravity shifts to Stage 3 (jiji-activities passthrough), where keybind duplication isn't a concern.

- [x] **Proposed: Verb-category menu ordering policy.** `docs/launcher/initiative.md` §10 enumerates three options (alphabetical, frequency-sorted, category-grouped). The architect recommends **category-grouped, stable within category** for these reasons: (a) it gives the user a learnable spatial map (Workspace verbs always at top, Activity verbs always at the same relative position), (b) frequency-sorted requires state (a usage counter) and re-ranking, which adds storage + reordering logic the launcher otherwise has none of, (c) alphabetical sounds principled but mixes unlike verbs (`close-window` between `center-focused-column` and `focus-workspace-up` — no spatial intuition). Within a category, **registration order** stays the source of truth (it's already the Stage 1 convention; the registry is hand-curated, so the author can put the most-used verb first per category).

  Concrete ordering for the drafted roster (assuming the roster box above is ratified as drafted): **Workspace → Window → Mode → Activity**. Activity verbs (Stage 3) come last so the fork-only category sits at the bottom where users who are on upstream don't see a hole.

  **Outcome of ratifying this box:** Phase 2.1 (or a dedicated micro-sub-phase) wires `Category` into `enabled()` ordering, drops the `#[allow(dead_code)]` on `Category` in `src/registry.rs:24`, adds a unit test pinning the category order.

- [x] **Proposed: Stage 2 vs Stage 3 batching.** `docs/launcher/initiative.md` §4 keeps Stage 2 (native compositor verbs) and Stage 3 (`jiji-activities` passthrough verbs) as separate stages. The workspace CLAUDE.md Resume cue flattens them ("Stage 2 = curated verb set + category-grouped menu ordering + jiji-activities passthrough breadth"). Architect recommends **keeping them separate** for these reasons: (a) Stage 2 verbs are upstream-compatible and exercise the native dispatch path; Stage 3 verbs all require `FORK | NIRI_ACTIVITIES` and exercise the passthrough path — splitting reviews the two dispatch shapes independently, (b) Stage 3 passthrough verbs have already been spec'd at the initiative level (`switch-activity-previous`, `move-window-to-activity`, `move-window-here`, `move-workspace-to-activity`, `assign-workspace`, `create-activity`, `remove-activity`, `save-activity`, `list-activities` — see initiative §4 Stage 3) and need their own Phase 3.x boxes (mirroring Phase 2.x), not a smush into Stage 2, (c) the loop-iteration discipline (one landing unit ~= one PR's worth of cognitive surface) is well-served by the split.

  **Outcome of ratifying this box:** Phase 2.x is native verbs only (workspace nav + window lifecycle + mode toggles + category ordering). Phase 3.x is `jiji-activities` passthrough breadth, authored separately once Phase 2 lands. The workspace CLAUDE.md Resume cue gets a one-liner correction in the next scribe pass.

### Phase 2.1 — Stage 2 surviving roster + Category ordering (one coarse sub-phase)

Lands the entire ratified surviving roster (2 verbs) plus the Category-grouped menu ordering wiring as **one** landing unit, per the Phase 2.0 outcome ("Phase 2.1 lands the entire surviving roster as **one** coarse sub-phase"). Both verbs share the immediate-dispatch shape, so they fold together cleanly; ordering wiring rides along because the new `Category::Mode` variant has to land in the same commit to compile.

- [x] `src/verbs/focus_workspace_previous.rs` — immediate-dispatch native verb shelling to `niri msg action focus-workspace-previous`. Capability: `Capabilities::NIRI_SOCKET` only (no `FUZZEL` — immediate verbs work without a picker per initiative §5). Category: `Workspace`. Landed in `c1fd6d7`.
- [x] `src/verbs/toggle_debug_tint.rs` — immediate-dispatch native verb shelling to `niri msg action toggle-debug-tint`. Capability: `Capabilities::NIRI_SOCKET` only. Category: `Mode`. Landed in `c1fd6d7`.
- [x] `src/niri.rs` — add `pub fn run_action(name: &str) -> anyhow::Result<()>` for zero-arg actions (thin wrapper around `proc::run_capture("niri", &["msg", "action", name])`). Keep `focus_workspace(id)` as-is — it carries an argument. The two new verb modules dispatch through `run_action`. Landed in `c1fd6d7`.
- [x] `src/registry.rs` — add `Category::Window` and `Category::Mode` variants in declarative order `Workspace, Window, Mode, Activity` (Window listed even though no Stage 2 verb sits there — declarative future-proofing for Stage 3 / restore-from-Appendix-D). Derive `PartialOrd, Ord` on `Category` (declaration order = sort order). Drop the `#[allow(dead_code)]` on `Category` at line 23 (now read by the ordering sort). Register the two new verbs in the static `REGISTRY` array. Landed in `c1fd6d7`.
- [x] `src/registry.rs::enabled` — change from registration-order filter to category-grouped stable sort: filter by capability, then `sort_by_key(|v| v.category)`. `sort_by_key` is stable in Rust's std, so registration order survives within each category. Landed in `c1fd6d7`.
- [x] `src/registry.rs::tests` — add a unit test pinning category-grouped ordering against a mixed-category fixture (assert a `Workspace` verb sorts ahead of a `Mode` verb even if the latter is registered first in the static array; assert intra-category registration order is preserved). Landed in `c1fd6d7`.
- [x] `tests/shims.rs` — add one shim test per new verb asserting the recorded `niri msg action <name>` invocation lands (reuses the existing `niri_body` `*)` recording branch — `$3 $4` captures `<name>` for `toggle-debug-tint`, and the existing patterns echo for unmatched `--json` args). Extend `debug_reports_filtering_upstream` with two `.stdout(predicates::str::contains(...))` assertions confirming both new verbs are `kept` on upstream (they require only `NIRI_SOCKET`, so they work on upstream niri). Landed in `c1fd6d7`.
- [x] `CLAUDE.md` — fold the two curation principles into a new "Curation discipline" paragraph under "Implementer discipline": (1) curate-don't-enumerate, (2) exclude verbs on muscle-memory keybinds in `~/.local/share/chezmoi/dot_config/niri/config.kdl.tmpl` (ratified 2026-05-28). Editorial; lands in the same commit. Landed in `c1fd6d7`.

**Reviewed:** 2026-05-28 (`c1fd6d7`, was `748be45` pre-fixer, `dacf520` after first amend). Phase 2.1 — the entire ratified Stage 2 surviving roster (2 verbs) plus Category-grouped menu ordering — landed as one coarse commit per the Phase 2.0 outcome. Reviewed across code quality, silent-failure surface, comment accuracy, test coverage, and type design. **Finding worth surfacing (CLAUDE.md banned-reference cleanup, `CLAUDE.md`):** the new "Curation discipline" sub-section in the initial commit referenced `(initiative §4)`, `Phase 2.0 ratification`, and `Appendix D` — all tokens on the hook's banned-reference list. Rephrased in the final commit to drop the parenthetical, replace the ratification reference with a datestamp-only note (2026-05-28), and refer to "the cut-verb candidates section in `docs/design.md`" without the Appendix-letter naming. **Finding worth surfacing (sort stability test overpromise, `src/registry.rs::tests`):** the initial test `category_grouped_ordering_pins_workspace_before_mode_regardless_of_registration_order` pinned the live REGISTRY output but did not vary registration order, so `sort_unstable_by_key` would have passed it — the test name overclaimed. Added `sort_by_key_preserves_intra_category_registration_order` with four locally-constructed same-category `Verb`s in reverse registration order. Subsequent re-review tightened the rustdoc: even 4 same-category elements is not a true stable-vs-unstable discriminator because pdqsort's insertion-sort fallback handles slices ≤20 elements stably in practice; the rustdoc now describes what the test actually pins (a behavioral order-preservation invariant) rather than the technically-stronger stable-sort claim. **Finding worth surfacing (action-failure shim contract, `tests/shims.rs::niri_action_failure_propagates_nonzero`):** added a shim test pinning that a failing `niri msg action` exits jiji-do with non-zero, not 69 (which is reserved for capability-miss). Historical analogue: the fuzzel cancel-vs-failure pattern in `a0eaccc` — both record the lesson that process-launcher code must discriminate error classes rather than collapsing all non-success. **Finding worth surfacing (`run_action` error contract rustdoc, `src/niri.rs::run_action`):** extended the rustdoc to name the `Err` conditions explicitly (non-zero exit or `niri` missing on `$PATH`), so callers know exactly what a returned `Err` implies without reading the implementation. **Finding worth surfacing (`Window` variant comment rot, `src/registry.rs`):** the initial comment `// no Window verbs yet` is temporal and will silently become false. Replaced with a structural description: `` `#[allow(dead_code)]` suppresses the unused-variant lint until the first Window-category verb is registered. `` This pins the rationale, not the current state, and survives future verb additions without rotting. Post-review fixes squashed into `c1fd6d7` (via `748be45` → `dacf520` fixer pass, then `c1fd6d7` rustdoc correction): CLAUDE.md banned-reference cleanup; `sort_by_key_preserves_intra_category_registration_order` stability test added with rustdoc tightened; `niri_action_failure_propagates_nonzero` shim test added; `run_action` rustdoc `Err` conditions named; `Window` variant comment rephrased to structural. Test count 23 → 28 (14 unit + 3 cli + 6 shims → 16 unit + 3 cli + 9 shims); `cargo +nightly fmt --all` clean; `cargo clippy --all --all-targets` zero warnings. Stage 2 surviving roster complete; the Phase 2.1+ ledger and Phase 3.x (jiji-activities passthrough breadth) boxes are deferred — the next architect pass authors Stage 3 per the center-of-gravity shift established at Phase 2.0 ratification. Proceed to Stage 3 authoring with the reviewed base.

---

> Phases 2.2+ (if any) — no further Stage 2 native verbs are queued after the 2026-05-28 ratification cut. Restoring any of the Appendix D candidates is an explicit human call.

### Phase 3.1 — Stage 3 passthrough core: snapshot-consuming verbs (one coarse sub-phase)

Lands the five `jiji-activities` passthrough verbs whose shape is "dispatch with the launch-time snapshotted id" — `switch-activity-previous` (no snapshot), `move-window-to-activity`, `move-window-here`, `move-workspace-to-activity`, and `assign-workspace`. All five share the passthrough dispatch shape (shell to `jiji-activities <verb>` with `--window`/`--workspace` filled in from the snapshot where applicable); all gate on the activities-capability cluster; all rely on Stage 0's explicit-id flags landed at `f948599` / `2a304a2`. The remaining initiative §4 Stage 3 verbs (`create-activity`, `remove-activity`, `save-activity`, `list-activities`) are deferred to a follow-up sub-phase — they carry undecided design questions (freeform name input via picker, stdout-via-keybind destination) that fall outside the "snapshot id → passthrough" shape.

The five verbs ride together because (a) they share one verb-module shape that the new mini-helper centralizes, (b) capability requirements split cleanly into two sub-shapes only (no-fuzzel toggle vs. with-fuzzel picker passthrough), (c) the registry + shim-test surface is one wave (one new verb-module file per verb plus one or two shim tests per verb), and (d) bundling avoids five round-trips through architect/implementer/scribe for what is genuinely one cognitive unit. Matches the Phase 2.1 precedent for coarse Stage-equivalent landings.

**Snapshot-empty contract (pinned here).** When a snapshot-consuming verb (`move-window-to-activity`, `move-window-here` for `--window`; `move-workspace-to-activity`, `assign-workspace` for `--workspace`) launches with `snapshot.focused_window` / `focused_workspace` = `None`, the verb **bails with stderr `"no focused <window|workspace> at launch"` and exits non-zero (exit 1, NOT 69)**. Exit 69 is reserved for capability misses; "no focused window" is a runtime-data miss. The bail must happen before any `jiji-activities` subprocess fires — falling through to `jiji-activities` with no `--window` would let it re-read focused state from a compositor whose focus has been stolen by the launcher menu's fuzzel, breaking the snapshot-at-launch contract that is the launcher's core value. This mirrors the historical lessons from `a0eaccc` (fuzzel cancel-vs-failure) and Phase 2.1's `niri_action_failure_propagates_nonzero`: discriminate error classes, don't collapse them.

**Cross-loop status note (non-blocking).** The compositor's cross-activity `move-window` path is currently a silent no-op (Phase 2.18 queued in the compositor loop). `jiji-do move-window-to-activity` is correctly wired here but is non-functional in production until that compositor fix lands. Landing Phase 3.1 does not worsen the situation — the verb plumbs through to the same `jiji-activities move-window` that is already exposed, with the existing (broken-against-other-activity) compositor behavior. Surfaced for awareness, not as a gate.

- [x] `src/verbs/switch_activity_previous.rs` — passthrough verb shelling to `jiji-activities switch-previous`. Capability: `Capabilities::NIRI_SOCKET | Capabilities::FORK | Capabilities::NIRI_ACTIVITIES` (no `FUZZEL` — `switch-previous` is a pure toggle and runs no picker). Category: `Activity`. Takes no snapshot field. Body: `crate::proc::run_capture("jiji-activities", &["switch-previous"])?; Ok(())`. Landed in `4a95f3b`.
- [x] `src/verbs/move_window_to_activity.rs` — passthrough verb shelling to `jiji-activities move-window --window=<id>`. Capability: `Capabilities::NIRI_SOCKET | Capabilities::FUZZEL | Capabilities::FORK | Capabilities::NIRI_ACTIVITIES` (FUZZEL because `jiji-activities move-window` spawns its own activity picker when no positional name is supplied). Category: `Activity`. Reads `snapshot.focused_window`; bails per the snapshot-empty contract above if `None`. The launcher passes no positional name and lets `jiji-activities` run its own picker (the snapshot supplies only the window id, not the destination activity). Landed in `4a95f3b`.
- [x] `src/verbs/move_window_here.rs` — passthrough verb shelling to `jiji-activities move-window-here --window=<id>`. Capability: `Capabilities::NIRI_SOCKET | Capabilities::FUZZEL | Capabilities::FORK | Capabilities::NIRI_ACTIVITIES` (FUZZEL because `move-window-here` runs a workspace picker internally — it is unit-shaped with no name positional, picker-only by design). Category: `Activity`. Reads `snapshot.focused_window`; bails per the contract if `None`. Landed in `4a95f3b`.
- [x] `src/verbs/move_workspace_to_activity.rs` — passthrough verb shelling to `jiji-activities move-workspace --workspace=<id>`. Capability: same four-flag set as `move-window-to-activity`. Category: `Activity`. Reads `snapshot.focused_workspace`; bails if `None`. Landed in `4a95f3b`.
- [x] `src/verbs/assign_workspace.rs` — passthrough verb shelling to `jiji-activities assign-workspace --workspace=<id>`. Capability: same four-flag set (FUZZEL because `assign-workspace` is multi-select-rofi-internally — but `jiji-activities` itself probes for rofi when invoked; from jiji-do's side it just shells out and propagates exit codes). Category: `Activity`. Reads `snapshot.focused_workspace`; bails if `None`. Landed in `4a95f3b`.
- [x] `src/verbs/mod.rs` — register the five new modules (`pub mod switch_activity_previous; pub mod move_window_to_activity; pub mod move_window_here; pub mod move_workspace_to_activity; pub mod assign_workspace;`). Landed in `4a95f3b`.
- [x] `src/registry.rs` — add five `Verb` entries to `REGISTRY` after the existing `switch-activity` entry, in initiative-§4 order: `switch-activity-previous`, `move-window-to-activity`, `move-window-here`, `move-workspace-to-activity`, `assign-workspace`. All under `Category::Activity`. The category-grouped sort already lands them at the bottom of the menu; intra-category registration order preserves the listed order. Landed in `4a95f3b`.
- [x] `src/verbs/<each>.rs` — extract a shared `dispatch_with_window(snapshot, verb)` / `dispatch_with_workspace(snapshot, verb)` helper if the `move-*` and `assign-workspace` bodies converge (judgment call during implementation — implementer kept them inlined per-verb: each verb module is 8–15 lines with an explicit bail; the three-call-site threshold was not reached). The `snapshot.focused_window.ok_or_else(|| anyhow::anyhow!("no focused window at launch"))?` shape is the load-bearing part; landed inline in `4a95f3b`.
- [x] `tests/shims.rs` — one shim test per snapshot-consuming verb pinning the argv that `jiji-activities` receives. The shim records `"$@"`; assertions use `lines.contains(&"<expected>")` (per the capability-probe `--version` invocation pattern), with the per-verb expected argv being:
  - `switch-activity-previous` → `"switch-previous"`
  - `move-window-to-activity` → `"move-window --window=11"` (matches the existing `niri_body` fixture's `focused: true` window id 11)
  - `move-window-here` → `"move-window-here --window=11"`
  - `move-workspace-to-activity` → `"move-workspace --workspace=21"` (matches the workspace fixture id)
  - `assign-workspace` → `"assign-workspace --workspace=21"`
  Landed in `4a95f3b`.
- [x] `tests/shims.rs` — one negative test for the snapshot-empty contract, scoped to one representative verb (`move-window-to-activity`): set up a `niri_body`-style shim where the windows JSON has `is_focused: false` everywhere; invoke `jiji-do move-window-to-activity` against the full-capability environment; assert exit code is non-zero AND non-69 (use `predicates::ord::ne(69)` plus a `.failure()` assert), and assert no argv line was recorded for `jiji-activities` (the dispatch must bail before the subprocess). One representative test is enough — the contract is the same for the other three snapshot-consuming verbs; coverage breadth here would be duplicative. **Fixer-added** (`move_workspace_to_activity_bails_when_no_focused_workspace`) per review finding: a Shape B negative bail test for the workspace-snap path was missing from the original commit (`362e7c2`); the fixer squashed it in, making the amended final commit `4a95f3b`. Two negative bail tests total cover both snapshot-field shapes (window and workspace). Landed in `4a95f3b`.
- [x] `tests/shims.rs` — extend `debug_reports_filtering_upstream` to assert the five new verbs are `filtered` on upstream (they all require `FORK`, which is absent there). One `.stdout(predicates::str::contains("<verb>: filtered"))` line per verb. Landed in `4a95f3b`.
- [x] `src/registry.rs::tests` — extend `enabled_filters_by_capability` or add a new test pinning the four-flag activities cluster: with `NIRI_SOCKET | FUZZEL | FORK | NIRI_ACTIVITIES` set, all five new verbs (plus the existing `switch-activity`) appear in `enabled(caps)` output. The category-grouped ordering test already covers Activity-last; it does not need updating for verb count but may need its registry-order assertion extended to spell out the six Activity-category verbs in declaration order. Landed in `4a95f3b`.

**Reviewed:** 2026-05-28 (`4a95f3b`, was `362e7c2` before fixer amend). Phase 3.1 — five `jiji-activities` snapshot-consuming passthrough verbs (`switch-activity-previous`, `move-window-to-activity`, `move-window-here`, `move-workspace-to-activity`, `assign-workspace`) — landed as one coarse commit per the DD's "one cognitive unit" rationale. Reviewed across code quality, silent-failure surface, test coverage, and contract pinning. **Finding worth surfacing (Shape B negative bail test gap, `tests/shims.rs`):** the original commit (`362e7c2`) covered the window-snapshot-empty bail path (`move-window-to-activity` with no focused window, `move_window_to_activity_bails_when_no_focused_window`) but lacked an analogous test for the workspace-snapshot-empty bail path. The fixer added `move_workspace_to_activity_bails_when_no_focused_workspace` before amending to `4a95f3b`. The two tests together cover both snapshot-field shapes; the other two workspace-consuming verbs (`assign-workspace`) share the identical bail path and are covered by behavioral symmetry — one representative per shape is the established DD contract. **Finding worth surfacing (cross-cutting `proc::run_capture` cancel-vs-failure gap, pre-existing):** `proc::run_capture` maps subprocess exit-69 leakage and signal-kill collapse to -1 equivalently to other failure codes — this is a pre-existing concern equally present in the `switch-activity` verb from Phase 2.1, not introduced by Phase 3.1. Not gated here; surfaced for awareness. A future refactor pass (exit-code class discrimination analogous to `a0eaccc`'s fuzzel cancel-vs-failure fix) applies uniformly to all subprocess-dispatching verbs and should be tackled cross-cuttingly rather than per-verb. **Finding worth surfacing (inlined vs. helper judgment call, `src/verbs/`):** implementer kept all five verb modules inlined (8–15 lines each) per the spec's "may keep inlined if ≤3 call sites" clause. The `anyhow!("no focused window/workspace at launch")` bail shape appears four times across the modules; if a fifth verb of this shape is added in Phase 3.2+, the three-call-site threshold for extraction will be reached — that is the natural trigger for introducing `dispatch_with_window` / `dispatch_with_workspace` helpers. Post-review fixes squashed into `4a95f3b`: `move_workspace_to_activity_bails_when_no_focused_workspace` test added at `tests/shims.rs` (Shape B workspace-snap negative bail). Test count 28 → 36 (17 unit + 3 cli + 16 shims; +8 vs. baseline: +1 unit for the four-flag activities cluster test, +7 shims: 5 positive argv-pin + 1 window-bail + 1 workspace-bail; spec predicted +7, fixer's Shape B test is the extra +1). Same 36 tests green; `cargo clippy --all --all-targets` zero warnings. Proceed to Phase 3.2+ authoring (`create-activity`, `remove-activity`, `save-activity`, `list-activities`) with the reviewed base.

---

### Phase 3.2 — Stage 3 remaining verbs design ratification (human gate)

The four remaining `jiji-activities` passthrough verbs (`create-activity`, `remove-activity`, `save-activity`, `list-activities`) carry materially different UX shapes — unlike Phase 3.1's five snapshot-consuming verbs, these do **not** combine into one cognitive unit. Each needs its own architectural call before implementation can be authored. Each box below is a `**Proposed:**` gate — ratify in-place by flipping `[ ]` → `[x]`, optionally with an amendment note. Drafts below are the architect's recommended defaults; nothing is decided until a box is flipped.

**Cross-cutting decision baked in (not part of any individual ratification box):** the `Snapshot` struct stays as-is. It is contractually "focused state at launch" (one window id, one workspace id, one output name, one activity name). The full activities **inventory** that `remove-activity`'s picker needs is a different concept; reading it at verb-dispatch time via `niri msg --json activities` (one extra subprocess on a verb that runs once per user keypress) is cheaper than broadening `Snapshot` into a general-purpose IPC cache. Confirmed by the Phase 3.1 advisor pass; non-controversial.

- [x] **Proposed: `save-activity` shape — snapshot-consuming passthrough.** Identical shape to Phase 3.1's five verbs. Reads `snapshot.focused_activity` (already captured); bails with stderr `"no focused activity at launch"` and exits non-zero (NOT 69, per the Phase 3.1 snapshot-empty contract) if `None`. Shells to `jiji-activities save <name>` with the snapshotted activity name as positional. Capability: `NIRI_SOCKET | FORK | NIRI_ACTIVITIES` (no FUZZEL — no picker spawned by jiji-do; `jiji-activities save` itself is non-interactive). Category: `Activity`.

  **Rationale:** the snapshot already carries `focused_activity`; this verb is the natural use case. The user picks `Save activity` from the menu, the launcher saves *the current activity* (the only sensible interpretation — "save" with no target is ambiguous, and a fuzzel picker for "which activity?" defeats the launcher's snapshot-at-launch value when the natural target is the focused one). The bail-on-`None` follows the Phase 3.1 precedent for `move-window-to-activity` / `move-workspace-to-activity`.

  **Outcome of ratifying:** Phase 3.2a lands `src/verbs/save_activity.rs` (~12 lines, identical pattern to `move_window_to_activity.rs`), registers in `REGISTRY`, adds one positive shim test (`save Work` argv) and one negative bail test (`focused_activity: None` → non-zero, non-69, no argv dispatched).

- [x] **Proposed: `create-activity` shape — freeform-name fuzzel prompt + direct-CLI form.** Two entry points: (1) menu path — `jiji-do` opens its menu, user picks `Create activity`, the launcher spawns `fuzzel --dmenu` with empty stdin (free-text prompt; the established pattern lives in `repos/jiji-activities/src/picker/single_select.rs::prompt_name`), the user types a name and presses Enter, the launcher shells to `jiji-activities create <typed-name>`. Empty input / cancel → exit 0 (clean no-op, mirrors `pick_one` cancel semantics). (2) Direct-CLI path — `jiji-do create-activity <name>` from a keybind skips the prompt. Capability: `NIRI_SOCKET | FUZZEL | FORK | NIRI_ACTIVITIES`. Category: `Activity`. **Snapshot unused.**

  **New surface in jiji-do:** a `menu::prompt_name(prompt: &str) -> anyhow::Result<Option<String>>` helper (mirrors `picker::prompt_name` in jiji-activities — fuzzel `--dmenu` with empty stdin; success+non-empty → `Some(typed)`, success+empty / exit-1 cancel → `None`, other non-zero → `bail!` per the established cancel-vs-failure discipline from `a0eaccc`). Reusable for any future freeform-name verb. The `create-activity` verb body branches on the new optional positional arg.

  **CLI surface change:** the verb takes an optional positional name. The current `cli::Cli` shape is `Cli { debug: bool, verb: Option<String> }` — adding a name positional means either (a) reshape `verb` to a subcommand-derived enum (large diff, opens the door to per-verb args properly), or (b) add a second positional `Cli { debug: bool, verb: Option<String>, verb_arg: Option<String> }` (smaller diff, conservative; the dispatch fn signature for `create-activity` reads the second positional from a `&Cli` parameter or via a side channel). The architect recommends **(b)** for this batch — option (a) is a larger architectural reshape best deferred to a dedicated sub-phase if/when a second name-bearing verb arrives. Either is implementer's call inside the ratified shape; the spec will commit to one.

  **Rationale:** the launcher's value proposition is menu-driven discovery; "you can create an activity from your launcher" is a real workflow. Refusing the menu path and forcing direct CLI only would be the path of least resistance but would leave the verb dead weight in the menu (it would have to be hidden). The freeform-prompt is launcher-flavored and reuses an established pattern from the sibling repo.

  **Outcome of ratifying:** Phase 3.2b lands `src/menu.rs::prompt_name` (~30 lines, near-clone of jiji-activities' `prompt_name`), `src/verbs/create_activity.rs` (~25 lines: prompt if no positional, bail on empty, shell out), CLI surface change per (a) or (b), 2 shim tests (positive: prompt returns "X" → argv `create X`; cancel: empty stdout → exit 0, no argv).

- [x] **Proposed: `remove-activity` shape — picker over existing activity names.** Two entry points: (1) menu path — `jiji-do` opens its menu, user picks `Remove activity`, the launcher reads `niri msg --json activities` (one extra subprocess, not added to `Snapshot` per the cross-cutting decision above), feeds the activity names to `fuzzel pick_one` for selection, then shells to `jiji-activities remove <picked-name>`. Cancel → exit 0. (2) Direct-CLI path — `jiji-do remove-activity <name>` skips the picker (same CLI-surface shape as `create-activity` per that box's recommendation (b)). Capability: `NIRI_SOCKET | FUZZEL | FORK | NIRI_ACTIVITIES`. Category: `Activity`. **Snapshot unused.**

  **Rationale:** removing the currently-focused activity is the wrong default — the user just typed `remove` and intends to clean up an *other* activity (otherwise they'd type `switch` first). A picker over the activity inventory matches `jiji-activities`' own conventions for verbs that select from inventory rather than acting on the focused thing. The inventory is read fresh at dispatch (not snapshotted) because (a) `Snapshot` purity, (b) inventory changes are rare so a stale snapshot would rarely be wrong but always feels wrong, (c) one extra subprocess on a verb that runs at user-keypress cadence is invisible.

  **Outcome of ratifying:** Phase 3.2c lands `src/verbs/remove_activity.rs` (~30 lines: read activities JSON via `proc::run_capture`, minimal serde brief reusing `ActivityBrief` from `snapshot.rs` if pub-exposed, or a new inline brief; `pick_one` the names; shell to `jiji-activities remove <name>`), 2 shim tests (positive: niri activities shim → fuzzel echoes name → argv `remove <name>`; cancel: fuzzel exit-1 → exit 0, no argv). Implementer's judgment whether to lift `ActivityBrief` to `pub(crate)` or inline a duplicate.

- [x] **Proposed: `list-activities` shape — direct-CLI only, hidden from menu.** Registered as a verb (so it gets a subcommand) but flagged not-for-menu so it doesn't appear in the fuzzel list. Direct invocation `jiji-do list-activities` shells to `jiji-activities list` and prints its stdout verbatim. Capability: `NIRI_SOCKET | FORK | NIRI_ACTIVITIES` (no FUZZEL — no picker; also no menu visibility means FUZZEL would be irrelevant either way). Category: `Activity`.

  **Verb-shape change required:** `Verb` currently has no notion of "registered but hidden from menu." Add `menu_visible: bool` field (default `true`). `registry::enabled(caps)` continues to return all enabled verbs (so `--debug` still surfaces `list-activities`); the menu render path in `main.rs` filters additionally on `v.menu_visible` before passing to `menu::render_menu`. Direct dispatch (`registry::find` + `is_enabled`) is unchanged. The architect recommends `menu_visible: bool` over (a) a `Category::Hidden` sentinel (overloads `Category` with non-category semantics; breaks the category-grouped sort invariant by introducing a category that doesn't sort meaningfully) or (b) skipping the registry entirely and routing at `cli.rs` (creates a second dispatch path; breaks the "registry is the single source of truth" invariant pinned in §4.3).

  **Rationale:** the initiative §4 Stage 3 entry explicitly calls `list-activities` "a data verb; rarely menu material but exposed for completeness." A menu pick that prints to stdout is nonsense UX in a fuzzel-launched flow — stdout has no destination when launched from a keybind. Direct-CLI form keeps it useful for shell scripts and one-off introspection; hiding from menu prevents the bad-UX path.

  **Outcome of ratifying:** Phase 3.2d lands `Verb { menu_visible: bool }` field addition (every existing `Verb` literal in `REGISTRY` gets `menu_visible: true` — 9 trivial diffs), filter wiring in `main.rs::run`, `src/verbs/list_activities.rs` (~15 lines: shell to `jiji-activities list`, print stdout to jiji-do's stdout), registry entry with `menu_visible: false`, a test pinning that `list-activities` does NOT appear in the menu render but DOES appear in `--debug` `kept` output, and a positive shim test pinning `jiji-activities list` is invoked.

**Batching after ratification.** Each box is its own sub-phase (3.2a / 3.2b / 3.2c / 3.2d) — unlike Phase 3.1, these do not share a dispatch shape, and the `cli.rs`/`registry.rs` reshapes for boxes 2 and 4 want isolated review. The architect's plan once all four are ratified:

- **Phase 3.2a (`save-activity`):** smallest; identical pattern to Phase 3.1. Lands first.
- **Phase 3.2d (`list-activities` + `Verb { menu_visible }`):** lands the `Verb`-shape change in isolation so the diff for the field addition is reviewed independently of any new verb body. Lands second.
- **Phase 3.2b (`create-activity` + `menu::prompt_name` + CLI-surface change):** new helper plus the larger CLI-surface decision. Lands third.
- **Phase 3.2c (`remove-activity`):** depends on the CLI surface from 3.2b being settled. Lands fourth.

This ordering minimizes per-sub-phase scope: each lands ~25–60 lines of code plus 2–3 tests, no two boxes share a load-bearing shape change.

---

> Phases 3.2+ implementation is gated on the four ratification boxes above. Once ratified (each box flipped `[x]`, optionally amended), the next `/jiji:land-subphase jiji-do` pass authors Phase 3.2a as the first implementable spec.

---

## Appendix C: Deferred suggestions

- **`src/error.rs` — `DoError::MissingCapability(String)` stringly-typed payload** — From review of `a0eaccc` (2026-05-28). Carry a typed `Capabilities` set (the unmet flags), format prose in `Display`. Type-design reviewer rated this HIGH; deferred because the machine-readable consumer (e.g. `--debug` introspection surfacing the unmet set) does not exist yet and exit-69 is already exercised by a test. Revisit in Stage 2 when `--debug` is expanded.
- **`src/snapshot.rs` — `focused_workspace` / `focused_output` joint-coupling** — From review of `a0eaccc` (2026-05-28). Nest output under a `FocusedWorkspace { id, output }` so the unrepresentable state (workspace present, output absent) is unconstructable. Reviewer: "worth fixing before more verbs read the snapshot in Stage 2."
- **`src/snapshot.rs` — `#[derive(Default)]` unused and bypasses the `from_json`/`capture` seam** — From review of `a0eaccc` (2026-05-28). Drop it in Stage 2 so callers cannot construct an empty `Snapshot` outside the intended paths.
- **`src/snapshot.rs` / `tests/shims.rs` — `parse_workspace_choices` edge-case tests missing** — From review of `a0eaccc` (2026-05-28). Null output field → `"? #id"` fallback, empty array, and malformed JSON are unexercised. Add unit tests for all three in Stage 2.
- **`src/proc.rs` / `src/capabilities.rs` — redundant `niri msg` reads between probe and capture** — From review of `a0eaccc` (2026-05-28). The probe issues `niri msg --json workspaces` / `activities` for capability detection; `Snapshot::capture` re-issues the same calls. A future stage could thread probe JSON into the snapshot to halve the subprocess count. Deferred as a perf concern — no correctness impact and the double-read is cheap relative to fuzzel startup.

## Appendix D: Stage 2 verbs cut at Phase 2.0 ratification (restore candidates)

Verbs originally drafted for Stage 2 in `docs/launcher/initiative.md` §4 that did not survive the Phase 2.0 ratification of 2026-05-28. Recorded here so future amendments can restore any of them with one line of rationale rather than re-litigating the cut.

Cut principle applied: **exclude verbs whose action is already on a muscle-memory keybind in the standard niri config** (`~/.local/share/chezmoi/dot_config/niri/config.kdl.tmpl`). A launcher menu entry that duplicates a one-key shortcut is dead weight — the user reaches for the key, not the menu. Restoring a verb here requires either (a) a new rationale for why menu duplication adds value despite the keybind, or (b) evidence the keybind has been removed/remapped in the standard config.

| Action (kebab-case) | Category | Current keybind | Rationale for cut |
|---|---|---|---|
| `focus-workspace-up` | Workspace | `Mod+PgUp`, `Mod+K`, `Mod+WheelScrollUp` | Triple-bound; directional nav is muscle-memory. |
| `focus-workspace-down` | Workspace | `Mod+PgDn`, `Mod+J`, `Mod+WheelScrollDown` | Triple-bound; directional nav is muscle-memory. |
| `close-window` | Window | `Mod+Q` | One-key close is muscle-memory. |
| `fullscreen-window` | Window | `Mod+Shift+F` | Muscle-memory toggle. |
| `toggle-window-floating` | Window | `Mod+Semicolon` | Muscle-memory toggle. |
| `center-column` | Window | `Mod+C` | Muscle-memory. (Original initiative draft said `center-focused-column` — that's a config setting, not an action. The bound action is `center-column`.) |
| `toggle-overview` | Mode | `Mod+O` | One-key overview is muscle-memory. |
| `toggle-keyboard-shortcuts-inhibit` | Mode | `Mod+Escape` (`allow-inhibiting=false`) | Bound with the inhibit-allowed escape hatch so it works even when inhibit is active. |
| `focus-window` (fuzzel) | Window | — | Picker-based discovery verb; *not* a keybind-duplication cut. Held back because Stage 2 was rescoped to a thin discovery slice; restore alongside Stage 3 or a Stage 2.x picker batch if a clear use case emerges. |
| `close-window-by-fuzzel` | Window | — | Picker variant of `close-window`; same restore criteria as `focus-window`. |
| `move-window-to-monitor` | Window | per-direction `move-window-to-monitor-*` variants are commented out in the config (lines 548, 552); only `move-column-to-monitor-*` is bound | Conservative cut to keep Phase 2.1 scope thin. The fuzzel picker variant is restore-worthy if a multi-monitor workflow surfaces it; the per-direction action gap means it's the *only* keybind-free path today. |
| `move-column-to-monitor` | Window | per-direction variants bound to `Mod+Shift+Ctrl+<arrow>` and `Mod+Shift+Ctrl+H/L` | Directional column-to-monitor is keybind-driven; restore the fuzzel picker only if a target-list workflow surfaces. |
| `move-window-to-workspace` (fuzzel) | Window | per-direction variants bound to `Mod+Ctrl+<arrow>`/`Mod+Ctrl+PgUp/Dn`/`Mod+Ctrl+J/K`, plus numbered `Mod+Ctrl+1..9` | Picker variant; directional + numbered already cover the common cases. |
| `move-column-to-workspace` (fuzzel) | Window | same as above | Same rationale. |
| `toggle-workspace-sticky` | Workspace | — | Cut from initiative §4 in the original Phase 2.0 ratification draft (upstream-compatible duplicate of `jiji-activities`' wrapping). Restore if the upstream-fallback story for sticky workspaces becomes load-bearing.|

Note on Stage 3 (`jiji-activities` passthrough verbs): the keybind-exclusion principle does **not** apply there, since `jiji-activities` verbs have no muscle-memory direct keybinds (only spawn-jiji-activities bindings, which already go through the same passthrough path). Stage 3 keeps the full passthrough breadth.
