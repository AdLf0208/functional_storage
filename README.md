# Functional Storage for Horizon

Port of **Functional Storage 1.5.8** (`Buuz135/FunctionalStorage`, MIT) to Horizon,
Minecraft Java 26.3. Clean-room Rust rewrite: the upstream source was read for
behaviour only. Local-only until the maintainer approves publishing; publishable
under MIT with this source.

## What is here

One flat Horizon SDK package (`api = "0.2"`, built by the Horizon launcher):

| Path | What it is |
|---|---|
| `horizon-mod.toml` | the only manifest and identity (`id = "functionalstorage"`, `version = "0.2.0"`, the `[native]` pin) |
| `Cargo.toml` | one crate; its only dependency is `horizon-mod-api` from the fetched `.horizon/sdk/` |
| `src/lib.rs` | the one `#[mod_api::horizon_mod] impl Hooks` |
| `src/support.rs` | the game glue: the drawer's block entity, container and menu, the clicks, the break/place contents, the comparator and the readout |
| `src/runtime.rs` | the pure drawer engine (capacity, kind lock, view slot, comparator), unit-tested; byte-identical to the retired 0.1.0 file |
| `assets/`, `data/` | the carried content (10 wood 1x1 drawers: models, textures, blockstates, block declarations, recipes, loot, lang, creative tab, the one-slot drawer menu) |
| `tools/e2e.sh` | the in-game scenario (needs a game the launcher built with this package) |
| `functionalstorage.hmod` | the delivered package |

The retired `.hnmod` layout — the mixin plan, the 23 per-hook helper files, the
embedded content package — is history: `../_shared/evidence/functionalstorage/retired-hnmod-0.1.0/`.

## The 1x1 drawers

The ten `<wood>_1` storage drawers: one item kind, capacity 2048 for a 64-stack
item, right-click insert (300 ms whole-inventory sweep), sneak-right-click lock,
a one-slot menu with the true count, hopper in/out, a comparator answer from
fullness, and contents that survive a break in the `functionalstorage:drawer`
item component and are restored on placement. A left click on the front takes
one item — a whole stack while sneaking — and hands it over; the front panel is
never dug (break the drawer from any other face). An emptied locked drawer keeps
its kind.

The pack's container slot is one stack, so the drawer's true amount lives in the
block entity and the slot is a proportional view; see `notes.md`.

## Build and validate

```sh
CARGO_INCREMENTAL=0 cargo test                        # 34 tests: 20 runtime, 14 support
cargo clippy --all-targets -- -D warnings
cargo fmt --check                                     # not --all: that checks the fetched SDK
rm -f Cargo.lock && cargo horizon-mod build           # target/horizon-mod/functionalstorage.hmod
cp target/horizon-mod/functionalstorage.hmod ./functionalstorage.hmod
cargo horizon-mod check functionalstorage.hmod
cargo horizon-mod fits functionalstorage.hmod --build nightly
```

The in-game leg needs a game the launcher built with this package
(`horizon-launcher native-build ports/functionalstorage --instance <i> --trust-native-code`);
`horizon-launcher` is not installed in this environment, so no 0.2.0 in-game run is
claimed (see `notes.md`).
