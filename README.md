# jiji-do

`jiji-do` is a Helix-style command launcher for the jiji
Wayland compositor. One keybind opens a fuzzel menu listing every capability-enabled verb;
each verb is also a top-level subcommand so direct keybindings can skip the menu entirely.
The launcher captures focused window, workspace, and activity state *once* at process start —
before any picker grabs keyboard focus — and passes the snapshotted ids to whichever verb it
dispatches. It runs on upstream niri with a reduced verb set and fully on the jiji fork with
`jiji-activities` installed.

See the owning design document at [`docs/design.md`](docs/design.md) and the launcher
initiative overview at [`docs/launcher/initiative.md`](../../docs/launcher/initiative.md)
(workspace-level).

## Capability matrix

`jiji-do` probes four capabilities at startup. Three probes (`FUZZEL`, `FORK`,
`NIRI_ACTIVITIES`) are independent — a miss leaves that flag unset and only reduces the
available verb set. `NIRI_SOCKET` is the exception: a miss aborts immediately with exit 69,
because the compositor socket is required for snapshot capture and every verb. The registry
filters the remaining verbs against the resolved flags, so the menu shows only what is
actually available.

| Capability | Probe | Effect when absent |
|---|---|---|
| `NIRI_SOCKET` | `$NIRI_SOCKET` set and a `niri msg workspaces` call succeeds | Exit 69 immediately — nothing can work without the compositor socket. |
| `FUZZEL` | `fuzzel` on `$PATH` | Menu invocation (`jiji-do` with no arg) exits 69. Picker-based verbs exit 69 on direct dispatch. Immediate-dispatch verbs still work. |
| `FORK` | `niri msg --json activities` returns a success response (the jiji fork carries this IPC; upstream niri does not) | All Activity-category verbs are hidden from the menu and exit 69 on direct dispatch. |
| `NIRI_ACTIVITIES` | `jiji-activities` on `$PATH` and `jiji-activities --version` exits 0 | All Activity-category verbs are hidden from the menu and exit 69 on direct dispatch. |

The combined gate for activities verbs is `FORK` **and** `NIRI_ACTIVITIES`. Either missing
hides the entire Activity category. Run `jiji-do --debug` to see the resolved flags and the
kept/filtered decision for every verb.

## Install

No remote or crates.io listing exists yet (Phase D — pushing the jiji fork to GitHub — is
pending). Clone the workspace and install from source:

```sh
git clone <workspace-repo>
cd repos/jiji-do
cargo install --path .
```

**Runtime dependencies:**

- `fuzzel` — required for the menu (`jiji-do` with no arg) and for all picker-based verbs.
  Install with your package manager (e.g. `apt install fuzzel` on Debian sid).
- `jiji-activities` — optional; unlocks the Activity-category verbs. Build from
  `repos/jiji-activities/` in the same workspace.
- jiji fork — optional; the `FORK` capability gates activity IPC. Running against upstream
  niri hides Activity-category verbs but leaves Workspace and Mode verbs fully functional.

**Fish completions** will be installable via `jiji-do completions fish` once the
`completions` subcommand lands in a later release (Phase 4.2). Until then, copy the output
of `cargo run -- --help` to seed a manual completion file if needed.

## Usage

```sh
jiji-do                          # Open the fuzzel menu (requires FUZZEL)
jiji-do <verb>                   # Dispatch a verb directly
jiji-do <verb> <name>            # Direct dispatch with a name argument (e.g. create-activity)
jiji-do --debug                  # Print resolved capabilities and per-verb kept/filtered status
```

`--debug` is the diagnostic surface for capability decisions. It prints which flags were
detected and, for each registered verb, whether it is kept or filtered and why. It iterates
the full `REGISTRY` — including menu-hidden verbs such as `list-activities` — so it is the
one surface where those verbs appear alongside their capability requirements. It never
appears on stderr during normal dispatch.

### Verbs

Verbs are grouped by category. The menu shows all enabled verbs in the order the
`Category` enum is declared (`Workspace → Window → Mode → Activity`); `Window` is reserved
between Workspace and Mode with no verbs registered yet. Verbs marked **direct-CLI only**
have `menu_visible: false` and never appear in the fuzzel menu — invoke them directly from
a keybinding or shell.

#### Workspace

| Verb | Label | Capabilities required |
|---|---|---|
| `switch-workspace` | Switch workspace | `NIRI_SOCKET`, `FUZZEL` |
| `focus-workspace-previous` | Focus previous workspace | `NIRI_SOCKET` |

#### Mode

| Verb | Label | Capabilities required |
|---|---|---|
| `toggle-debug-tint` | Toggle debug tint | `NIRI_SOCKET` |

#### Activity

All Activity verbs require `NIRI_SOCKET`, `FORK`, and `NIRI_ACTIVITIES` at minimum.
Picker-based verbs additionally require `FUZZEL`.

| Verb | Label | Capabilities required | Notes |
|---|---|---|---|
| `switch-activity` | Switch activity | `NIRI_SOCKET`, `FUZZEL`, `FORK`, `NIRI_ACTIVITIES` | |
| `switch-activity-previous` | Switch to previous activity | `NIRI_SOCKET`, `FORK`, `NIRI_ACTIVITIES` | |
| `move-window-to-activity` | Move window to activity | `NIRI_SOCKET`, `FUZZEL`, `FORK`, `NIRI_ACTIVITIES` | |
| `move-window-here` | Move window to workspace here | `NIRI_SOCKET`, `FUZZEL`, `FORK`, `NIRI_ACTIVITIES` | |
| `move-workspace-to-activity` | Move workspace to activity | `NIRI_SOCKET`, `FUZZEL`, `FORK`, `NIRI_ACTIVITIES` | |
| `assign-workspace` | Assign workspace to activities | `NIRI_SOCKET`, `FUZZEL`, `FORK`, `NIRI_ACTIVITIES` | |
| `save-activity` | Save activity | `NIRI_SOCKET`, `FORK`, `NIRI_ACTIVITIES` | |
| `list-activities` | List activities | `NIRI_SOCKET`, `FORK`, `NIRI_ACTIVITIES` | **Direct-CLI only** — not shown in the fuzzel menu. |
| `create-activity` | Create activity | `NIRI_SOCKET`, `FUZZEL`, `FORK`, `NIRI_ACTIVITIES` | Accepts `<name>` positional; prompts via fuzzel if omitted. |
| `remove-activity` | Remove activity | `NIRI_SOCKET`, `FUZZEL`, `FORK`, `NIRI_ACTIVITIES` | |

## Example keybindings

These are examples only — `jiji-do` has no default install action that sets any keybindings.
Add to your niri/jiji `config.kdl` as appropriate for your layout.

```kdl
binds {
    // Open the command palette (Helix-style: colon = command mode)
    Mod+colon { spawn "jiji-do"; }

    // Alternative for US layouts where colon requires Shift
    // Mod+Space { spawn "jiji-do"; }
}
```

`Mod+colon` is unbound in the standard jiji config and the colon-key convention for "command
mode" is familiar from Vim and Helix. `Mod+Space` is a common alternative for keyboards
where reaching `Shift+;` is awkward.

### Binding verbs directly

Individual verbs can be bound to keys to skip the menu entirely. The snapshot is still
captured at process start, so the focused-state guarantee holds for direct invocations too.

```kdl
binds {
    // Native verb — immediate dispatch (no picker)
    Mod+colon { spawn "jiji-do"; }
    Mod+grave { spawn "jiji-do" "focus-workspace-previous"; }

    // Passthrough verb — gated on FORK + NIRI_ACTIVITIES
    // (hidden / exits 69 on upstream niri or without jiji-activities)
    Mod+ctrl+grave { spawn "jiji-do" "switch-activity"; }
}
```

## Dependency contract

`jiji-do` has **no** `niri-ipc` or `jiji-activities` Cargo dependency. All compositor
interaction is via `niri msg` subprocesses; all activities interaction is via `jiji-activities`
subprocesses. This is a deliberate architectural constraint: the same binary works against
upstream niri (reduced verb set) and the jiji fork (full verb set) without recompilation or
conditional linking. A compile-time dep on either library would couple `jiji-do`'s release
cycle to the compositor's ABI and undermine the cross-fork compatibility guarantee.

A grep test (`tests/cli.rs::no_forbidden_dependencies`) enforces that neither forbidden dep
appears in `Cargo.toml`.

## Contributing / status

`jiji-do` is pre-1.0. The design is in [`docs/design.md`](docs/design.md). Development is
driven by the unified jiji loop (`/jiji:land-subphase jiji-do`). Phases 1–3 are complete
(skeleton, capability detection, all 13 verbs registered); Phase 4 (polish: README, fish
completions, packaging) is in progress.

This repository has no remote yet — Phase D (pushing the jiji fork and its tools to GitHub)
is a future session. Until then, clone from the workspace as described in **Install** above.
