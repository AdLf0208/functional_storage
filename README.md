# Functional Storage — a Horizon port

**Functional Storage** is a Minecraft mod. FunctionalStorage. This repository is a **port of it to [Horizon](https://github.com/horizon-rust/horizon)**, the Rust engine for Minecraft Java 26.3. It carries the complete source of the port plus the content files it ships, and installs as a single `.hmod` package.

Upstream: [Buuz135/FunctionalStorage](https://github.com/Buuz135/FunctionalStorage) — licence **MIT**.

| | |
|---|---|
| Upstream mod | Functional Storage ([source](https://github.com/Buuz135/FunctionalStorage)) |
| Ported to | Horizon 0b14f8a, Minecraft Java 26.3, mod API `0.2` |
| Package | `functionalstorage.hmod` (install it with the Horizon launcher) |
| Licence | **MIT** — the upstream mod's licence; see [Licence and credit](#licence-and-credit) |

## What this port implements

### Slice 1

The ten `<wood>_1` storage drawers: one item kind, capacity 2048 for a 64-stack
item, right-click insert (300 ms whole-inventory sweep), sneak-right-click lock,
a one-slot menu with the true count, hopper in/out, a comparator answer from
fullness, and contents that survive a break in the `functionalstorage:drawer`
item component and are restored on placement. The left-click take is refused at
this pin (a block-entity write from that hook is reverted, so the take would
duplicate): the click shows "Open the drawer to withdraw" and the menu's result
slot is the withdrawal.

The pack's container slot is one stack, so the drawer's true amount lives in the
block entity and the slot is a proportional view; see `notes.md`.

## Not yet ported

- **No 0.2.0 in-game run in this environment.** The e2e needs a game the launcher built with this
- **The left-click take stays removed** (the 0.1.0 pin ruling): a block-entity write from
- **The front shows the item without its count.** The SDK now has
- **No upgrades, no 1x2/2x2/compacting/fluid/ender/framed drawers, no controller, no armory cabinet,
- **No logger.** The SDK exports none and a mod may not add a crate, so a refused stack or a failed
- **An emptied drawer's front render can keep the old item** until its chunk is resent: the client

## Installing

1. Install the [Horizon launcher](https://github.com/horizon-rust/horizon/releases) (0.2.70 or newer).
2. Add `functionalstorage.hmod` to the instance's mods.
3. The package is pinned to a specific Horizon build; the launcher checks that pin.

## Licence and credit

Functional Storage was created by its upstream authors; the code, models and assets in the original mod
remain theirs and are used here under **MIT**. The Rust in this repository is a clean-room
rewrite for Horizon, not a translation of the original source.

- `LICENSE` — the upstream licence text.
- `LICENSE-NOTICE.md` — what this port carries and where it came from.
- The upstream project: https://github.com/Buuz135/FunctionalStorage

## Technical details

```
horizon-mod.toml   package manifest (id, version, api, [native] pin)
Cargo.toml         the crate; depends on `horizon-mod-api` from the fetched SDK
src/               lib.rs (the one `impl Hooks`), runtime.rs (pure logic + tests), glue
assets/ data/      the content the package ships
tools/             content generators and the e2e scenario
```

- Build: `cargo horizon-mod build` (packages; it does not compile against the game).
- Test: `cargo test`, `cargo clippy --all-targets -- -D warnings`.
- In-game: `ports/tools/launcher-game.sh run functionalstorage` in the campaign tree, or install the
  `.hmod` and play; the harness scenario is `tools/e2e.sh`.

This repository is generated from the campaign's port at `ports/functionalstorage/`; the port's own
`notes.md` is the full record (decisions, traps, evidence).
