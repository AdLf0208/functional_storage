# Functional Storage → Horizon

Ported from **Functional Storage 1.5.8** (`Buuz135/FunctionalStorage`, MIT),
Minecraft Java 1.21.1 NeoForge, read from
`ports/_shared/upstream/functionalstorage` at commit
`11edb40deec2714e1aa2aca3f0a699b9cc29efd7` (2026-10-21). The upstream source
was read for behaviour only; this is a clean-room Rust rewrite. Local-only
until publishing is approved (ARCHITECTURE.md §8).

> **0.2.0 is the SDK package (2026-11-18).** `.hnmod` is retired; the port is one flat
> `functionalstorage.hmod` with `api = "0.2"`. Read `## Migration to the SDK format` below for the
> file map, the hook table, the ladder and the traps. Everything above that section is the 0.1.0
> history: its ladder, e2e logs and pin notes describe `.hnmod`/`horizon.py` runs that no longer
> exist in this tree (kept as historical evidence in `../_shared/evidence/functionalstorage/`).

## Slice 1 — the 1x1 storage drawers

Ten wood-coloured 1x1 drawers (`functionalstorage:<wood>_1`), one big slot
each, one item kind per drawer, capacity 2048 for a 64-stack item (upstream
`BigInventoryHandler`: `64 * base`, base 32 for `X_1`, scaled by the item's
stack size). The carried content is the upstream models/textures (MIT), the
recipes, loot tables, lang, one creative tab and the drawer menu type.

### Behaviour read from upstream

| Area | Upstream | This port |
|---|---|---|
| capacity | `BigInventoryHandler.getTotalAmount = 64 * base`, `getSlotLimit = floor(total * maxStack / 64)`; `X_1` base 32 → 2048 (512 for a 16-stack item, 32 for a 1-stack item) | same, `runtime::capacity` |
| kind lock | the first item sets the kind; another kind is refused (`isValid`) | same, `Drawer::insert` |
| lock | `DrawerBlock.LOCKED` + `isLocked`; a locked empty drawer refuses new items | `locked` in the block entity NBT; sneak-right-click with an empty hand toggles it |
| insert | `useItemOn`/`onSlotActivated` inserts the held stack; a second click within 300 ms fills from the whole inventory | right-click the front; 300 ms whole-inventory sweep |
| extract | `onClicked` (left click) extracts 1, or the stack while sneaking, only on the front face; 3-tick cooldown | same; 150 ms cooldown |
| comparator | `floor(fullness * 14) + 1`, 0 when empty | `has_analog_output`/`analog_output` (API 64) |
| menu | Titanium drawer GUI (storage slots, upgrades, count) | one-slot mod menu (`functionalstorage:drawer`) with the item view and the true count drawn by `native_screen_draw` |
| break | `getDrops` returns the drawer item with the block entity's NBT (`FSAttachments.TILE`); contents survive a pick-up | `functionalstorage:drawer` item component; `destroy_block` stashes + empties the view, `loot_drops` attaches, `place_block` restores |
| front display | `DrawerRenderer`: the stored item + the count text | the stored item via `native_draw_item` (API 71); **no count text** (the pin has no in-world text helper) |

### The pin probe (why this shape)

The pin at port time was `c9d474d` (API 80, 108 named hooks); the campaign pin
is now **`1d63e0a5`** (432 stamped hooks / 174 helpers, `NATIVE_HOOKS_API` 411)
and every injection below still resolves there unchanged (see the re-pin
section). The drawer's block entity and container fit:

- API 15 block entity + tick, API 16 container slots, API 17 mod menu (the pin
  also supports content mod menu types, `data/<id>/horizon/menu/`), API 19
  screen/screen draw, API 45 update tag, API 71 block entity submit + item draw,
  API 64 comparator, API 65 item components, API 75 loot drops, API 21/40
  right/left click.
- **The gap:** a container slot is one `ItemStack`, whose count is the item's
  stack size (64/16/1). A drawer holds 2048. The block entity therefore keeps
  the true amount (and the stored stack, components included) in its saved
  compound, and the container slot is a proportional **view**
  (`ceil(amount * maxStack / limit)`, 1..=maxStack). Every external change of
  the view (a hopper, a menu take) is reconciled in the block entity tick
  against the last view (`Drawer::sync_view`). Hoppers therefore fill and empty
  a drawer through the view, and the comparator answers from the true amount.
- The menu slot is a **result** slot (`results: [0]` in the menu type): players
  take from it but cannot place a different kind into it, so the one-kind
  invariant cannot be broken from the GUI.
- No in-world text API exists at the pin (`BlockEntityDraw` has no text), so
  the drawer's front shows the item but not the count; the count is in the menu
  and in the action-bar readout after an insert.

### Deliberate deviations

- The count is not drawn on the drawer's front (pin gap).
- The front item is drawn with the item's `fixed` display pose; the upstream
  renderer's exact scale/offset is not reproduced.
- No upgrades (copper/gold/diamond/netherite/creative/void/redstone/collector),
  no 1x2/2x2/compacting/fluid/ender/framed/controller/armory blocks. Capacity
  is the un-upgraded `X_1` value.
- No GUI upgrade slots, no configuration/linking tools, no framed styles.
- Explosions/pistons: explosions are handled (`explosion_detonate` stashes the
  contents); a piston destroy and `/setblock … destroy` are not hooked and lose
  a drawer's bulk (the view slot is cleared only by the hooks above).
- A locked empty drawer still accepts a **hopper** item and hands it back as an
  item entity (the container's `inputs` list is per block, not per instance).
- The 300 ms whole-inventory sweep and the 150 ms extract cooldown are wall
  clock, not game ticks.

## Verification

(Filled after the ladder and the e2e run.)

### Probe result — feasible

The pin supports the drawer's block entity, container, tick and menu:

- **Block entity**: API 15 (`block_entity`, `block_entity_ticks`), API 16
  (`block_entity_container`), API 17 (`block_entity_tick`), API 45 (update
  tag), API 71 (`block_entity_submit` + `native_draw_item`).
- **Menu**: a content mod menu type (`data/functionalstorage/horizon/menu/drawer.json`)
  opens through the barrel template's `Class::Barrel` use path; in-game the
  empty-hand click reported `screen: "container"` with the drawer's slot
  (`slot 300`) showing `minecraft:stone`. The beacon template's class does not
  open the menu (probed: the click left the screen playing), so the carriage
  uses `minecraft:barrel`.
- **Container/happers**: the one-slot container is reachable from all six
  faces; a hopper above added 5 stone (3 → 8) and a hopper below drained the
  drawer (7 → 0) into a chest in the same run.
- **Comparator**: API 64 (`has_analog_output`/`analog_output`); a comparator
  beside a non-empty drawer read `powered=true` while the plain-stone control
  read `powered=false`.
- **The one representation gap**: a container slot is one `ItemStack` whose
  count is the item's stack size; a drawer holds 2048. The true amount and
  stack live in the block entity's saved compound; the slot is a proportional
  view (`Drawer::view_count`) reconciled each tick (`Drawer::sync_view`).
- **No in-world text API**: the front renderer draws the stored item but not
  the count (the count is in the menu's data slots / action bar). The bell
  template was rejected at load for redeclaring `facing`; the barrel template
  already supplies `facing`, so the block declares no state properties and the
  blockstate covers `facing` × `open` (8 variants).

### Validation evidence

| Step | Command | Result |
|---|---|---|
| runtime tests | `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 cargo test --offline` in the port | 15 passed, 0 failed |
| runtime clippy | `cargo clippy --all-targets --offline -- -D warnings` | clean, 0 warnings |
| stage | `python3 tools/horizon.py stage --horizon <campaign>/horizon` | `status: ok`, 22 injections, 2 modules |
| check | `python3 tools/horizon.py check --horizon …` | `cargo_exit: 0`, `target/horizon-check.log` 0 warnings |
| embedded tests | `python3 tools/horizon.py test --horizon …` | `test result: ok. 15 passed; 0 failed` |
| fast build | `cargo build --profile fast --bin horizon-client --bin horizon-ctl` | exit 0 |
| content `.hmod` | `cargo run --manifest-path …/cargo-horizon-mod/Cargo.toml -- horizon-mod build` | `content/functionalstorage.hmod`, 55,496 B, SHA-256 `d8f55aa0…90da1` |
| native `.hnmod` | `python3 tools/package.py` | `target/functionalstorage-0.1.0.hnmod`, 23,683 B, SHA-256 `8e028766…2937b` |

### E2E — partial (this is the honest status)

`tools/e2e.sh` verified, in one run and in the focused probes:

- the drawer places, faces its placer (`facing=south`) and reads
  `functionalstorage:oak_1: 0/2048 empty`;
- a right-click stores the held stack (64/2048, the hand empty);
- a different kind (dirt) is refused and stays in the inventory (64);
- a second stone click adds 5 (69);
- a left click gives one stone to the player;
- the empty-hand click opens the one-slot menu; its `slot 300` shows the stone
  view and a shift-click withdraws it, which the tick reconciles (stored 4 → 3);
- a hopper above adds 5 (3 → 8) and a hopper below drains it (7 → 0);
- a comparator reads `powered=true` for the drawer and `false` for the
  plain-stone control;
- a break of a 64-stone drawer produces a drawer item whose re-placement reads
  `64/2048 stone` again (the `functionalstorage:drawer` component), although
  the harness `ctl inventory`/drop queries are one tick behind the pickup.

**Known defect / blocker for a full pass:** the left-click's deferred take
(`on_level_tick_post` → `take_one`) **gives the item but the block entity's
amount read stays unchanged in the probe** (64 → give 1 → still 64/2048). The
take is queued from `left_click_block` (the write in that hook is not kept; the
right-click hook's identical `mutate` is). The duplication is a real defect:
the port must not ship the left-click take as-is until the write is understood.
The `ctl break`/`ctl place` inventory queries also lag the pickup (the item
component itself round-trips). A full e2e pass and a left-click fix are the
remaining work; the BE/container/menu/hook feasibility itself is proven.

Harness notes: launch `--offscreen --creative`, build a platform at the
player's spawn, `gamemode survival` before a break that must drop, `ctl break`
takes ~3 s, and `ctl messages`/`ctl screen` are the readouts (the `ctl
inventory` snapshot can lag a give/pickup by a tick).

## Left-click take — root cause and pin block (2026-11-06)

The reported duplication is real and the investigation is conclusive: the
take is queued correctly and the inventory add happens, but the block entity
amount is not decremented.

**Root cause.** The pin keeps a mod block entity write only when the write
runs in the **authoritative level of a player-action hook**. The identical
`mutate` (`native_with_mod_block_entity`) writes the view and the NBT amount
from `right_click_block`; from `left_click_block`, from the block entity's own
tick (`block_entity_tick`), from `player_tick`, from `server_tick` and from
`level_tick_post` the write is reverted. `native_extract_item` (the
container-slot path the menu uses) is also reverted from `server_tick`. The
inventory add and the item give persist in every context, so the result is one
item handed out and no amount change (64 → give 1 → 64).

Probes (see `tools/probe_break.sh`, `tools/probe_take.sh`):

- a `native_tell` in the take showed the local drawer 64 → 63, so the pure
  decrement is right; the persisted read stayed 64;
- the same take through `native_give_stack` / `give_direct`, through the BE
  tick's own `entity`, through a deferred `server_tick` extract and through
  the recorded `player_tick` all left the read at 64;
- the right-click insert (same `mutate`, no take) persists 64/2048;
- the one-slot menu's shift-click (the menu engine writes the slot) persists
  and the drawer's tick reconciles it (4 → 3).

**State of the port.** The left-click take is **removed** to avoid the
duplication: a left click on the front shows the action-bar hint
"Open the drawer to withdraw", and withdrawal is through the one-slot menu
(the result-slot take, verified). A real left-click take is **blocked** until
the pin exposes an authoritative block-entity write from `left_click_block`
(or an equivalent server hook). The missing capability has that exact shape.

**Break accounting.** The container view spills on removal and the view clear
from `destroy_block` is one of the reverted writes, so the break now stores
the **bulk** (`amount − view`) in the `functionalstorage:drawer` component:
the spilled view plus the item's bulk add back to the true amount. The break
probe showed the spill stack on the ground and the component carrying the
bulk.

**Class + tier (AGENTS "Approach model"):** content · machines. The BE,
container and menu are the machine-tier shape; the carried assets/data are the
content half. The deferred part is the faithful left-click take only.

### Final evidence at `c9d474d` (2026-11-06, corrected behaviour; historical — superseded by the re-pin section below)

| Check | Result |
|---|---|
| `cargo fmt --all --check` | pass |
| `cargo test --offline` (runtime) | 18 passed, 0 failed |
| `cargo clippy --all-targets --offline -- -D warnings` | pass, 0 warnings |
| `horizon.py check` | `cargo_exit: 0`, 0 warnings |
| `horizon.py test` | 18 passed, 0 failed |
| fast client + ctl build | exit 0 |
| content `.hmod` | `functionalstorage.hmod` **55,499 B**, SHA-256 `924b07602e5436bdf17f5820d2a26b3d290bec25917cb2a7b8fd1ec24eefa677` |
| native `.hnmod` | `functionalstorage.hnmod` **23,971 B**, SHA-256 `8d7dd44e9e04fb003edf4ae914ff6ae5949b9ae3ec8fa90509ea2c01f10a74ed` |
| `tools/e2e.sh` | **PASS**, exit 0 (`ports/functionalstorage/target/repin-e2e.log`) |

The passing run proves the corrected behaviour, not the removed take:

```
menu withdrawal: 64 -> 63, stone in the hand: 1
action bar: 'Open the drawer to withdraw'; stored: 68 -> 68, stone in the hand: 0
PASS: the drawer stores 100+, refuses other kinds, withdraws one through its menu (64 -> 63), its left click only shows the withdraw hint (the pin reverts a BE write there), hoppers move items, comparators read it, and a break keeps its contents
```

The earlier final evidence (exit 1 at the left-click take) is superseded: the
e2e now asserts the **corrected** behaviour, so it passes while the take stays
removed.

**Content bug found by the rerun.** The break phase originally dropped only
the container contents: the 10 loot tables were in the 1.20 shape
(`"conditions": [{"condition": "minecraft:survives_explosion"}]`), whose inner
object the 26.3 parser reads as an unknown type (`""`), so the pool was
skipped. They now use the pool-level 26.3 form the Storage Delight port proved,
`"condition": {"type": "minecraft:survives_explosion"}`, and a broken drawer
drops `functionalstorage:oak_1 x1` plus the spilled view. That is why the
`.hmod` is 3 bytes larger and its hash changed; the `.hnmod` is unchanged (no
Rust change).

**Break accounting (unchanged representation).** The component carries the
bulk (`amount − view`) and the view slot spills as loose items. The e2e
asserts the drawer item drops and is picked up, the re-placed drawer restores
the bulk (158 of 174 in the passing run), and the spill is the loose stack;
it does not assert an exact total because the two halves are separate drops.

The port stays `ported-partial` only because the faithful left-click take is a
pin block (the sibling `utilitarian` e2e used the same scope decision).

## Re-pin to campaign pin `1d63e0a5` (2026-11-08)

The port migrated from the nightly `c9d474dd9116e84c243f8992d75c11314df56f06`
to the campaign pin **`1d63e0a5a37a7a708eb0b32b49bf0dfeb28ed8a9`** with
horizon-mixins **`5a64af938ec86b00d53d16a408aabc0038fb3e57`** (schema 6).
`compatibility.json` now carries the pin, all nine `source_sha256` entries and
the mixins revision; `mixins/functionalstorage/horizon-mixin.toml` carries the
same Horizon revision. The shallow pin checkout needed the documented one-time
online `cargo fetch --locked` (29 new registry packages, `FETCH_OK`).

### Migration deltas (API/tooling, not gameplay)

| Change at `1d63e0a` | Where | Fix |
|---|---|---|
| `NativeScreen` gained the field `own_background` | `crates/horizon-client/src/mod_api.rs:408` | `on_screen` sets `own_background: false` (Horizon still draws the plain panel and the slot frames, as before) |
| the embedded game tests are lib tests; `--bin horizon-client` ran **0 tests** (a vacuous pass) | the module lives in `horizon_client`'s library (`lib.rs: pub mod game`) | `tools/horizon.py test` now passes `--lib`, matching the other migrated ports; 18 tests actually run |
| `info()` filtered chat to `^functionalstorage:` | `tools/e2e.sh` | the counterfactual's barrel reply is `minecraft:barrel: not a drawer`; the filter now accepts any `namespace:path:` reply (the drawer phases are unchanged) |

All **22** injected hooks resolve against the pin's `native-hooks.json`
(`plan.json`: 22 injections, 2 modules; no hook removed, renamed or renumbered
in the stamped catalog). Class + tier are unchanged: **content · machines**,
licence MIT → rank 0, local-only until approval.

### Verification (2026-11-08, pin `1d63e0a`)

| Step | Command | Result |
|---|---|---|
| `cargo fmt --all --check` | in the port | pass |
| runtime tests | `cargo test --locked --offline` (port workspace) | **18 passed, 0 failed** |
| runtime clippy | `cargo clippy --all-targets --locked --offline -- -D warnings` | pass, **0 warnings** |
| stage | `python3 tools/horizon.py stage --horizon …/pins/horizon-1d63e0a` | `status: ok`, **22 injections, 2 modules** |
| check | `python3 tools/horizon.py check …` | `cargo_exit: 0`, **0 warnings** |
| embedded tests | `python3 tools/horizon.py test …` (`--lib`) | `test result: ok. 18 passed; 0 failed; 1243 filtered out` |
| fast build | `python3 tools/horizon.py build …` | `cargo_exit: 0` (636 s) |
| content `.hmod` | `cargo horizon-mod build` (pin crate) | `content/functionalstorage.hmod` **55,499 B**, SHA-256 `924b07602e5436bdf17f5820d2a26b3d290bec25917cb2a7b8fd1ec24eefa677` — **byte-identical** to the old artifact (content did not change) |
| native `.hnmod` | `python3 tools/package.py` | **24,028 B**, SHA-256 `64b001b1dbb5f98903e1b3cc23e5090e0b9182cbcd8686e59b0388416f284321`, 26 entries; `horizon-native.json` pins `horizon` = `1d63e0a5…`, `mixins` = `5a64af93…` |
| `tools/e2e.sh` | one slot `e2e` invocation | **PASS, `EXIT:0`** |
| counterfactual | `COUNTERFACTUAL=1 tools/e2e.sh` | **PASS, `EXIT:0`** (same click on a barrel stores nothing) |
| negative control | `NEGATIVE=1 tools/e2e.sh` | **`FAIL: the drawer stored 163, not 0 (the check is alive)`**, `EXIT:1` — the scenario is fail-capable |
| `.hmod` load check | shared game, `--dev-mod functionalstorage.hmod` | `PASS: the packaged .hmod loaded (item 1, block functionalstorage:oak_1)` |
| `.hmod` negative control | same game/world **without** `--dev-mod` | `{"error":"unknown item functionalstorage:oak_1"}`, `{"error":"unknown block name"}`, `PASS: without --dev-mod the drawer is absent` |

Quoted from the passing e2e (`ports/_shared/evidence/functionalstorage/repin-e2e-1d63e0a.log`):

```
menu withdrawal: 64 -> 63, stone in the hand: 1
action bar: 'Open the drawer to withdraw'; stored: 68 -> 68, stone in the hand: 0
break: {… "drops":{"functionalstorage:oak_1":1,"minecraft:stone":6}, …}
restored: 163 (was 178; the view spilled as the loose items above)
relaunch: stored 163 -> 163 at functionalstorage:oak_1
PASS: the drawer stores 100+, refuses other kinds, withdraws one through its menu (64 -> 63), its left click only shows the withdraw hint (the pin reverts a BE write there), hoppers move items, comparators read it, and a break keeps its contents
E2E EXIT:0
```

The scenario now also proves **relaunch persistence**: the e2e quits and
relaunches the same world and reads the same amount and kind back
(`stored 163 -> 163`). The barrel control proves the drawer's 100-item
behaviour is its own code (`stone left in the hand after the barrel click:
100`). The negative run proves the assertions can fail.

### Gaps (unchanged)

The left-click **take** stays removed by the campaign pin ruling: a left click
on the front only shows the action-bar hint "Open the drawer to withdraw" and
takes nothing; withdrawal is the one-slot menu's result-slot take (verified
64 → 63). The pin still reverts a block-entity write from `left_click_block`
(and the other non-player-action contexts), so the take was not re-enabled.
The other documented gaps (no front count text, no upgrades, no 1x2/2x2 or
other families, the locked-drawer hopper quirk, wall-clock cooldowns) are
unchanged.

## Embedded content — one `.hnmod` deliverable (2026-11-10)

The content half now travels **inside** the native bundle (the AE2 pattern, proved
on `charginggadgets`): the built `content/functionalstorage.hmod` is carried at
`mixins/functionalstorage/data/functionalstorage-content.hmod`; `embedded_content.rs` writes it to
`.horizon-cache/` at start and appends it to the dev list from one `content_root`
HEAD injection (`cancellable = true`; bindings `root`, `enabled`, `dev`). One
installed `.hnmod` installs both halves, so the split content+code blocker is
gone. `tools/package.py` packs `data/` and refuses a copy that differs from
`content/functionalstorage.hmod`.

Verification (2026-11-10, pin `1d63e0a`, Minecraft 26.3):
- **ladder**: `cargo fmt --check`, `clippy --all-targets -- -D warnings` (0
  warnings), `cargo test` **18 passed / 0 failed**, `horizon.py stage / check / test /
  build` all `cargo_exit: 0` (embedded tests included).
- **artifacts**: `content/functionalstorage.hmod` **55,499 B**, SHA-256 `924b07602e5436bdf17f5820d2a26b3d290bec25917cb2a7b8fd1ec24eefa677`,
  **86** entries; `functionalstorage.hnmod` **66,174 B**, SHA-256 `36f43d7c64a51c244a9abd6e0d0b1c0f38194605245adfc075e0d3ef60ae48fc`, **28**
  entries (rebuilt locally from these sources with `tools/package.py`; a second
  run is byte-identical and `horizon-mod check --kind native-mod` passes);
  `mixins/functionalstorage/data/functionalstorage-content.hmod` is byte-identical to the built `.hmod`.
- **e2e with no `--dev-mod`** (`EMBEDDED=1`): `PASS` + `EXIT:0`; the run printed
  `embedded content: 55499 B, identical to content/functionalstorage.hmod`.
- **control** (`e2e-control.log`, `.horizon-cache` made unusable): the sample id
  is unknown, `PASS` + `EXIT:0` - the content really comes from the bundle.
- **fail leg** (`NEGATIVE=1`): `FAIL` + `EXIT:1`.
- `check-e2e-logs.py functionalstorage --strict` green (all logs end `EXIT:<n>`);
  `upstream_ready.py functionalstorage` -> `READY`, exit 0.

Harness fixes: `tools/horizon.py` defaulted `--horizon` to the stale `ports/horizon` (now the pin checkout); the re-place phase now backs the player out of the target cell (its hitbox overlapped it) and aims at the support face - the placement assertion is unchanged.

Skills: superpowers/using-superpowers,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices

### Mode B + AE2 embedding (2026-11-10) — one listable artifact, re-verified

Pin unchanged (`1d63e0a5…` / `5a64af93…`, schema 6, `game.rs`
`64813cf4…f80f`). The session rebuilt the port from its current sources and
applied the **AE2 embedding pattern** so one `.hnmod` installs the content and
the code (the registry keeps one file per project): `mixins/functionalstorage/data/functionalstorage-content.hmod`
is the built `content/functionalstorage.hmod` byte for byte, `mixins/functionalstorage/embedded_content.rs`
compiles it in with `native_data!`, and the `embedded-content` `[[injection]]`
(`target = "horizon_client::game::mods::content_root"`, `kind = "HEAD"`,
`cancellable = true`, bindings `root`/`enabled`/`dev`) writes it to the game's
`.horizon-cache` and appends it to the dev-mod list. `tools/package.py` refuses
an embedded copy that differs from the built hmod.

All compile/e2e steps ran through `ports/tools/build-slot.sh` with
`CARGO_INCREMENTAL=0`; the ladder is the shared `tools/ladder-embed.sh`
(hmod build → embed → hnmod → fmt → clippy → test → stage/check/test/build),
evidence `ports/_shared/evidence/functionalstorage/ladder-*`.

| Step | Result |
|---|---|
| `cargo fmt --all --check` / clippy `-D warnings` | exit 0, **0 warnings** |
| `cargo test --locked --offline` | **18 passed**, 0 failed |
| `horizon.py stage/check/test/build` | `cargo_exit: 0` in every step |
| `content/functionalstorage.hmod` | **55,499 B**, `924b0760…a677`, **86 entries**, "no code: content only" |
| `functionalstorage.hnmod` (single artifact, carries the content) | **66,174 B**, `36f43d7c…48fc`, **28 entries** |
| e2e (with `EMBEDDED=1`, the shipped shape) | default **PASS + `EXIT:0`** / counterfactual **PASS + `EXIT:0`** / negative **FAIL + `EXIT:1`** |
| `check-e2e-logs.py functionalstorage --strict` | green (`logs=… ok=…`, exit 0) |
| `upstream_ready.py --copyleft functionalstorage` | **READY**, exit 0 |

Fail-capability: the selftest/negative legs flip a real assertion and exit
non-zero; the embedded shape is the one the artifact ships (no `--dev-mod`), and
the e2e checks the bundle wrote its content out (or the loader reports
`content from 1 mod(s)` when the game consumes the file).

## Reviewer repairs (2026-11-11) — PR review items and what changed


Each item below is quoted from `ports/_shared/horizon-mods/reviews/pr-*-functionalstorage.md` (the review is the specification), with what this repair did about it.

- **Reviewer:** licence notice credits only Horizon contributors
  **Repair:** `LICENSE` now carries `Copyright (c) 2021 Buuz135, Rid` and both archives carry the notice (`mixins/functionalstorage/LICENSE`, `data/functionalstorage/license/`).
- **Reviewer:** provenance hash claims
  **Repair:** The entry pins the artifact's real hash; the PR body and the release text still name the older one and must be corrected on re-push (orchestrator, needs the token).
- **Reviewer:** relative link
  **Repair:** Already absolute (`https://github.com/Buuz135/FunctionalStorage`).
- **Reviewer:** registry artifact delivery
  **Repair:** **Staging (orchestrator, not ours)**: the artifact is not in registry release/staging as `<sha256>.<ext>`; the orchestrator answers it with `horizon-mods#686` (no write access to the registry's releases). Every repackaged artifact below has a fresh sha256 for that re-push, and the author release must be re-uploaded from the port repository.

Skills: superpowers/using-superpowers,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices

## Migration to the SDK format (0.2.0, 2026-11-18)

Horizon's mod-API merge (#16941, launcher ≥ 0.2.70, PR #17992) retired `.hnmod`: the registry closed
every `.hnmod` submission on 2026-11-18 with one reason, and an `.hnmod` no longer loads. This port's
former submission, **PR #831** (`functionalstorage` 0.1.0), was closed by that purge. **0.2.0** is
the same behaviour republished as one flat SDK `.hmod`; the changelog text is at the end of this
section.

Pins: SDK nightly **`0b14f8acfd97fa2c750e42795249dcc981178ca4`** (2026-11-18, `package_format` 6,
`content_api` "0.2"), mixins **`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`**, for 26.3. The pinned SDK
is cached in-tree at `../_shared/sdk/0b14f8a/` and fetched into `.horizon/sdk/` (gitignored) by the
build. Class + tier are unchanged: **content · machines**, MIT → rank 0, local-only until the
maintainer approves publishing.

### What moved, and where the retired tree is

The retired paths were **moved, never copied**, to
`../_shared/evidence/functionalstorage/retired-hnmod-0.1.0/` **before** the first
`cargo horizon-mod build` (the tool refuses a root holding retired package paths):

| 0.1.0 path | Now | Evidence |
|---|---|---|
| `mixins/functionalstorage/` (`horizon-mixin.toml`, `runtime.rs`, `support.rs`, 23 helpers) | `src/runtime.rs` (**byte-identical**), `src/support.rs`, `src/lib.rs` | `retired-hnmod-0.1.0/mixins/functionalstorage/` |
| `content/assets/`, `content/data/` (85 files) | the package root: `assets/functionalstorage/`, `data/functionalstorage/`, `data/minecraft/` | `retired-hnmod-0.1.0/content-functionalstorage-0.1.0.hmod` |
| `content/horizon-mod.toml` (the content-half manifest) | merged into the root `horizon-mod.toml` with `[native]` | `retired-hnmod-0.1.0/content-horizon-mod.toml` |
| `content/functionalstorage.hmod` (the content half) + the root `functionalstorage.hmod` copy | not needed: one package carries content and code | `retired-hnmod-0.1.0/content-functionalstorage-0.1.0.hmod`, `…-repin-924b0760.hmod` |
| `functionalstorage.hnmod` (68,878 B, embeds the content) | `functionalstorage.hmod` (97 entries, native + content) | `retired-hnmod-0.1.0/functionalstorage.hnmod` |
| `compatibility.json`, `rust-toolchain.toml`, `Cargo.lock` | `[native]` in `horizon-mod.toml`; the pinned commit decides the toolchain | `retired-hnmod-0.1.0/` |
| `tools/horizon.py`, `package.py`, `test_stage.py`, `test_template.py`, `stamp_license.py`, `ladder-embed.sh`, `probe*.sh`, `tools/audit/` (a second crate), `.github/` | deleted: staging, `.hnmod` packaging and the probes belong to the retired format | `retired-hnmod-0.1.0/tools/`, `retired-hnmod-0.1.0/.github/` |
| the old `AGENTS.md` (it said to open `compatibility.json` and run `horizon.py`) | rewritten for the SDK layout | — |

**Kept and updated:** `tools/e2e.sh` (the scenario and its assertions are format-independent; it now
takes a launcher-built `GAME`, an optional `CONTROL_GAME` and `CONTENT_DIR`), `notes.md`, `README.md`,
`VALIDATION.md`, `LICENSE`, `LICENSE-NOTICE.md`.

**The carried content is unchanged.** All **85** files under `assets/` + `data/` are byte-identical
(size and sha256) to the retired content package `content-functionalstorage-0.1.0.hmod`:

```
compared=85 differing=0
```

The 0.1.0 build wrote two extra data files from the repository's licence
(`data/functionalstorage/license/{license,notice}.txt`, via `stamp_license.py`); they are restored in
the package at the same paths and are byte-identical to the root `LICENSE` and `LICENSE-NOTICE.md`
(`e2a2d1c5…`, `0cd833b7…`), so the licence still travels with the carried work as well as at the
package root.

### File map (0.2.0)

| Path | What it is |
|---|---|
| `horizon-mod.toml` | the only manifest and identity: `id = "functionalstorage"`, `version = "0.2.0"`, `api = "0.2"`, `side = "both"`, `capabilities = ["registries"]`, `[native]` pinned to `0b14f8a`/`e59da90` |
| `Cargo.toml` | one crate, `edition = "2024"`; its only dependency is `horizon-mod-api` from `.horizon/sdk/` |
| `src/lib.rs` | the one `#[mod_api::horizon_mod] impl Hooks`: `register`, `component_types`, `register_commands`, `run_mod_command`, `right_click_block`, `left_click_block`, `break_block`, `explosion_detonate`, `loot_drops`, `server_tick`, `level_tick_post` |
| `src/support.rs` | the glue: `DrawerPart` (block + block-entity moments), `Placed` (item), the declarations, the saved-compound codec, the click/sweep/break/place wiring and the readout. 6 unit tests of the declarations and codecs |
| `src/runtime.rs` | the pure engine, **byte-identical** (sha256 `e5e4d4ba…52f1`), 18 tests |
| `assets/`, `data/` | the carried content (see above) |
| `LICENSE`, `LICENSE-NOTICE.md`, `README.md`, `icon.png` (128×128, 809 B) | inside the package |
| `tools/e2e.sh` | the in-game scenario |
| `functionalstorage.hmod` | the deliverable |

### Helper → hook table

Each former `[[injection]]` helper (23 of them, beside `runtime.rs`/`support.rs`) is now one typed
method or one declaration. The helper files themselves are gone.

| 0.1.0 injection (hook, API) | 0.2.0 |
|---|---|
| `embedded-content` (`content_root` HEAD, the AE2 write-out) | **gone**: one flat `.hmod` carries `assets/` and `data/` itself |
| `block-entity` (15; returned `(block, Compound::new())`) | the block entity **type** declared in `Hooks::register` (`block_entity_type("drawer").blocks(<10 paths>)`); a fresh drawer starts with an empty compound, so no Part method is needed |
| `block-entity-ticks` (15) | `BlockEntityPart::block_entity_ticks` → `event.ticks = true` |
| `block-entity-container` (16/17) | `Registrar::block_container` with `support::container()` (one slot, every face, the drawer menu) for each of the ten blocks |
| `block-entity-tick` (17) | `BlockEntityPart::block_entity_tick` → `support::tick` |
| `block-entity-update-tag` (45) | `BlockEntityPart::block_entity_update_tag` (the stored stack, for the front renderer) |
| `block-entity-submit` (71) | `BlockEntityPart::block_entity_submit` → `support::submit` |
| `screen` (19) | `BlockEntityPart::screen` → `support::screen()` |
| `screen-draw` (19) | `BlockEntityPart::screen_draw` → `support::draw_screen` (the count and the lock) |
| `state-for-placement` (76) | `BlockPart::state_for_placement` (the front faces its placer) |
| `right-click-block` (21) | `Hooks::right_click_block` → `support::right_click` |
| `left-click-block` (40) | `Hooks::left_click_block` → `support::left_click` (the withdraw hint; still no take) |
| `destroy-block` (14) | `Hooks::break_block` (renamed upstream: `BreakBlockEvent`) → `support::break_block` |
| `explosion-detonate` (22) | `Hooks::explosion_detonate` → `support::explosion_detonate` |
| `place-block` (14, `stack`, `pos`) | `ItemPart::on_block_placed_by` on `support::Placed` → `support::block_placed` |
| `loot-drops` (75) | `Hooks::loot_drops` → `support::loot_drops` |
| `has-analog-output` (64) | `BlockPart::has_analog_output` |
| `analog-output` (64) | `BlockPart::analog_output` (the last answer the tick read; see trap 5) |
| `server-tick` (4) | `Hooks::server_tick` → `support::server_tick` |
| `level-tick-post` (4) | `Hooks::level_tick_post` → `support::level_tick_post` |
| `component-types` (65) | `Hooks::component_types` → `support::declare_component` |
| `register-commands` (8) | `Hooks::register_commands` → `support::declare_commands` |
| `perform-command` (8, split words) | `Hooks::run_mod_command` → `support::run_command` (typed `<pos:block_pos>`, read from `CommandInvocation`) |
| the `[[module]]`s `runtime` / `state` | `pub mod runtime;` / `pub mod support;` |

**Hooks mapped: 22 of 23 injections** (the content injection has no home and needs none), plus one
declaration (`block_container`), one block entity type, two Parts. **Gaps recorded: none by mapping** —
every 0.1.0 behaviour has a typed place — with the two behavioural gaps the 0.1.0 slice already had
(left-click take, no front count; see *Gaps*).

### The ladder (each step inside one `build-slot.sh` lease, `BUILD_LOCAL=1`, `CARGO_INCREMENTAL=0`)

`build-slot.sh` still speaks the retired tool; for this package the step names only set the thread
count. There is no staging step and no staged tree: the SDK crate is compiled in place against
`.horizon/sdk/`.

| Step | Command | Result |
|---|---|---|
| tests, first (red) | `cargo test` with the glue at `todo!()` | **19 passed, 4 failed** — the pure runtime and the path test green, every unimplemented glue helper red (`not yet implemented`) |
| tests | `cargo test` | **24 passed**, 0 failed (18 `runtime::`, 6 `support::`) — `evidence/functionalstorage/sdk-test.log` |
| lints | `cargo clippy --all-targets -- -D warnings` | exit 0, **0 warnings** — `sdk-clippy.log` |
| format | `cargo fmt --check` (**not** `--all`) | clean — `sdk-fmt.log` |
| package | `rm -f Cargo.lock && cargo horizon-mod build` | `Built ./target/horizon-mod/functionalstorage.hmod` — `sdk-build.log` |
| second build | the same again | **byte-identical** (`cmp` empty) — `sdk-build-second.log` |
| registry check | `cargo horizon-mod check functionalstorage.hmod` | exit 0 — `sdk-check.log` |
| machine form | `cargo horizon-mod check … --json` | `{"ok":true,…}` — `sdk-check-json.log` |
| fits the pinned build | `cargo horizon-mod fits functionalstorage.hmod --build nightly` | exit 0 — `sdk-fits.log` |
| `runtime.rs` untouched | `unzip -p functionalstorage.hmod src/runtime.rs \| sha256sum` | `e5e4d4ba…52f1`, the retired file byte for byte |

`check` output:

```
functionalstorage 0.2.0 (native-mod, format native-1/0b14f8acfd97fa2c750e42795249dcc981178ca4/e59da9094f0c2fdda6c627fe9eff22dec6a906f0, for 26.3)
  94766 bytes, SHA-256 4ef7d482ae08a16bfb48270101861b4669ded2375e5657ff31dedff1c7a01996, unsigned
  native source: unsandboxed; not executed by the registry check. Compile and test against the exact pinned Horizon and Mixins revisions before review.
```

`fits` output:

```
Functional Storage:
0b14f8a  made for
```

**The package:** `functionalstorage.hmod`, **94,766 B**, sha256
`4ef7d482ae08a16bfb48270101861b4669ded2375e5657ff31dedff1c7a01996`, **97 entries** — root
`horizon-mod.toml`, `Cargo.toml`, `LICENSE`, `LICENSE-NOTICE.md`, `README.md`, `icon.png`, `uses.json`;
`src/lib.rs`, `src/runtime.rs`, `src/support.rs`; `assets/functionalstorage/…` (52), `data/functionalstorage/…` (34, the two licence copies included), `data/minecraft/tags/block/mineable/axe.json`
(1). **No `Cargo.lock`.** A second build is byte-identical.

**`uses.json`** (extracted from the package, `evidence/functionalstorage/sdk-uses-from-package.json`):
**585 surface entries** — 83 hook entries (22 hooks plus their event fields), 70 methods, 5 Parts, 35
traits, 281 types, 106 handles, 2 macros (`horizon_mod`, `horizon_part`), the `registries` capability,
and one free-fn/assoc entry each. The hook set it records: `register` (with
`DefaultCapabilitiesRegistrar::block_container` and the `block_entity_type` builder),
`component_types`, `register_commands`, `run_mod_command`, `right_click_block`, `left_click_block`,
`break_block`, `explosion_detonate`, `loot_drops`, `on_block_placed_by`, `server_tick`,
`level_tick_post`, and the Part moments `state_for_placement`, `has_analog_output`, `analog_output`,
`block_entity_ticks`, `block_entity_tick`, `block_entity_update_tag`, `screen`, `screen_draw`,
`block_entity_submit`.

### Behaviour that changed with the format (deliberate)

- **The comparator answers from the tick's reading** (trap 5): the same `runtime::Drawer::signal`, but
  read in the block entity's tick and cached, because `analog_output` is handed a `&dyn LevelHost`
  with no block-entity handle. An idle drawer is refreshed at `level_tick_post` whenever the answer
  changes, so a comparator beside it updates within a tick; **on the first tick after a load a
  comparator may read 0 and is corrected by that tick's refresh.**
- **An idle drawer answers `Unchanged`** (trap 7) instead of rewriting the same payload every tick.
- **The LAN hand-resend workaround is gone** (trap 12): the hand and inventory writes go through
  `living.set_hand_item` / `player.set_inventory_slot`, which the tick's inventory sync carries.
- **The saved compound is replaced, not merged** (trap 9) so an emptied drawer really forgets its
  item. The keys and their meanings are unchanged (`item`, `amount`, `locked`, `limit`), so 0.1.0
  saves read back.
- **`tools/e2e.sh` defaults to a launcher-built game** with no `--dev-mod` folder, because a native
  `.hmod` cannot load on an unmodded game.

### Traps this migration hit (the next port must know)

1. **`block_entity_part` already expands to the block table.** A Part attached to a type answers the
   block's moments (`state_for_placement`, the comparator) for **every block the type covers**; adding
   `r.block_part(block, SamePart)` as well would run those hooks twice per block (the 0.1.0 helper set
   attached each type to exactly one block and hid this).
2. **A declared block entity's `id` is the *type* id** (`functionalstorage:drawer`), not the block id:
   the 0.1.0 `is_drawer(&entity.id)` guards cannot work. Check the block at the position
   (`level.block(host, pos)`) or rely on the type's block list.
3. **`LevelRef` has no `state_block`/`block_state_id`.** Those live on the `LevelHost` argument of a
   block hook; a `LevelRef` handle reads `level.block(host, pos)` → `BlockKey::id(host)`. Mixing them
   up is six compile errors, not a silent bug (this build's first red run).
4. **Super-area methods need their traits in scope:** `host.block_states()` needs `HostBlockStates`,
   `host.transaction(…)` needs `HostTransaction`; without them the compiler says "no method named …".
5. **`analog_output` cannot read a block entity.** It is handed `&dyn LevelHost` and `LevelHost`'s only
   block-entity read takes a `BlockEntityRef`, whose constructor is game-side only. The port caches the
   answer from the tick and forces a neighbour refresh at `level_tick_post` when it changes.
6. **A fresh declared block entity starts with an empty compound.** Every key whose absent value would
   be meaningful needs a default: `limit` falls back to `capacity(64)`, `locked` to false, and `amount`
   0 means "empty" (no sentinel needed here, unlike `naturesaura`'s recipe index).
7. **`BlockEntityTickReply::Unchanged` is the cheap way to stay idle.** Comparing `nbt`, `items` and
   `data` before and after the tick is what decides it; the default `Changed` write-back would dirty
   the chunk every tick.
8. **`ItemStack::count` is a getter now** and `set_count` returns `Result`: hand and inventory writes
   are `set_count(…)` plus `then_some(…)`, not field assignment.
9. **`merge_data` cannot delete a key**: emptying a drawer needs `replace_data` of the whole compound
   (built from `entity.data(host)` first), or a stale `item` resurrects the contents.
10. **`player.inventory_slots` and `player.set_inventory_slot` share their numbers** (0..=35 main,
    36..=39 armour, 40 off hand, 41/42 body/saddle), while `ItemPlace::PlayerSlot` uses 100..=103 and
    `-106`. The sweep writes through the player API and excludes the armour slots explicitly.
11. **`on_block_placed_by` is the stack-aware placement notice** (the old `place_block` had no stack);
    it runs after the block's own placement, so the block entity already exists, and its writes are
    not overwritten by the stack-data application.
12. **The LAN hand-resend workaround is dead code at this pin** (`host.hub.send_inventory` has no
    successor): the player-slot writes are synced by the tick.
13. **`tools/audit/` is a refused package path** — its `[workspace] members` left a second crate in the
    tree (`unexpected package path: tools/audit/src/main.rs` is what `naturesaura` hit). Every retired
    path moved to evidence **before** the first build.
14. **`Cargo.lock` is packed and changes the artifact.** `cargo test`/`clippy` write one; `rm -f
    Cargo.lock` immediately before `cargo horizon-mod build` (and `/Cargo.lock` in `.gitignore`), then
    two builds are byte-identical.
15. **`cargo fmt --check`, never `--all`** — the latter reports the fetched SDK's own `build.rs`.
    `cargo fmt` did not touch `src/runtime.rs` (sha256 re-checked after formatting).
16. **`cargo horizon-mod check` with no file argument checks the wasm target**; `check <file.hmod>` is
    the package check and `cargo check`/`cargo test` the native one.
17. **Only `README.md` of the `.md` documents is packed.** The package's root holds `horizon-mod.toml`,
    `Cargo.toml`, `LICENSE`, `LICENSE-NOTICE.md`, `README.md`, `icon.png` and `uses.json`; `notes.md`,
    `AGENTS.md`, `VALIDATION.md` and `tools/` stay in the repository, so editing them cannot change the
    artifact (and the quoted sha256 is stable across them).

### Gaps after the migration

1. **In-game evidence landed with 0.2.1.** 0.2.0 was never run in game (no launcher here); the 0.2.1
   fixes came with the in-game leg: `ports/tools/launcher-game.sh run functionalstorage` → `PASS` +
   `EXIT:0`, a control leg (`EXIT:1`) and a negative leg (`EXIT:1`), recorded in
   `ports/_shared/evidence/functionalstorage-e2e/`. See `## Review fixes (2026-11-18)`.
2. ~~**The left-click take stays removed** (the 0.1.0 pin ruling): a block-entity write from
   `left_click_block` was reverted at `1d63e0a`, so a take would duplicate. The click still shows
   "Open the drawer to withdraw"; withdrawal is the menu's result slot.~~
   **Done in 0.2.2**: re-checked at the current pin, the block-entity write from `left_click_block`
   persists; the faithful take (one item, a whole stack while sneaking, front panel only, a 3-tick
   window) ships and is asserted in game — see `## Slice 2 — the left-click take (0.2.2)`. The 0.1.0
   ruling named the retired engine, not this one.
3. **The front shows the item without its count.** The SDK now has
   `BlockEntityDrawContext::text`, so a later slice can draw it (a new behaviour, not this migration).
4. **No upgrades, no 1x2/2x2/compacting/fluid/ender/framed drawers, no controller, no armory cabinet,
   no configuration/linking tools** — unchanged slice boundary.
5. **No logger.** The SDK exports none and a mod may not add a crate, so a refused stack or a failed
   write is silent apart from the readout command.
6. **An emptied drawer's front render can keep the old item** until its chunk is resent: the client
   tag is merged into the client's copy, so an unset `item` key does not erase what is there. This is
   the 0.1.0 behaviour unchanged (its `block_entity_update_tag` sent an empty compound too); a later
   slice can send a one-shot `set_client_update_fields` marker with the withdrawal that emptied it.

### Upstream defects fixed

None found in this migration; the 0.1.0 content defect (the loot tables' 1.20 `conditions` shape, which
26.3 skipped) was already fixed in the carried data and is unchanged here.

## Review fixes (2026-11-18)

Review: `var/review/1243-functionalstorage.md` (source review of the 0.2.0 package; work done 2026-11-19).

| Finding (`file:line`) | What a player could do | Fix | Test that pins it |
|---|---|---|---|
| `src/support.rs:511` — the drawer refreshes its logical amount only on the block-entity tick, so a right-click writeback (`:456`) can **restore** a withdrawal a menu/hopper already took, and a break (`:887`) can pack that withdrawal into the drop a second time | a hopper or menu takes from the view slot and a click/break in the same tick hands the same items out again (duplication) | `reconcile` is now the one rule every writer applies: `settle` reads the view slot and reconciles before `mutate` (clicks, lock, sweep) and before `stash_bulk` (break, explosion) work from the amount. A different kind in the slot is handed back instead of being overwritten | `a_writeback_after_a_withdraw_does_not_restore_the_withdrawn_count`, `a_matching_hopper_insert_is_accounted_for_before_the_writeback`, `a_break_packs_only_what_the_slot_still_shows` — all three fail with `reconcile` made a no-op |
| `src/support.rs:259` — the chunk-update tag broadcast the **whole saved stack**, disclosing writable book pages and nested container contents to every viewer of the chunk | see any drawer's contents (book pages, shulker contents) without opening it | `render_item` sends only the fields the front model needs: the item id and `count: 1`. Components stay on the server | `the_client_update_carries_only_what_the_front_draws` — the counterfactual (send the saved stack) puts `minecraft:written_book_content` in the payload and the test fails |
| `src/support.rs:1006` — the unrestricted `info` command named the block even when it was not a drawer, and read a drawer's item/amount at any loaded position | probe for hidden blocks and other players' machine contents anywhere loaded | the command is declared `Command::op(…, 2)` (game master and above) and a non-drawer gets one generic line, `Not a drawer.`, naming no block | `the_info_command_needs_an_operator` (level 2) plus the in-game phase "a block that is not a drawer gets one generic answer" — it fails on the old reply (`minecraft:barrel: not a drawer`) |

### Defect the in-game leg caught: the click duplicated its stack

The 0.2.0 package had never been run in game. Its first run failed at "the drawer stores a whole stack
at once": the drawer stored 64 stone **and the hand kept the 64**. Cause, proved in game: the hand write
was `let mut left = held; let _ = left.set_count(left.count() - inserted);` — and `ItemStack::set_count(0)`
is **refused** at this pin, so an emptied hand kept the whole stack (the cmd's `clear @s` removed 64
after the click). Fixed with `kept_count`/`remaining`: an emptied hand is `None`, never a zero-count
copy, and the selected slot is written with `set_inventory_slot` (which also sends the client's sync).
The sweep path used the same pattern and is fixed with it. Test: `an_emptied_hand_is_none_not_a_zero_count_copy`.

Ladder (each step in one slot lease, `BUILD_LOCAL=1`, `CARGO_INCREMENTAL=0`):

| Step | Result |
|---|---|
| `cargo test` | **30 passed**, 0 failed (29 lib incl. 8 support, +1 new) |
| `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` | exit 0 / exit 0, 0 warnings |
| `cargo horizon-mod build` ×2 (`rm -f Cargo.lock` first) | `cmp` byte-identical, SHA-256 `9b7d86b9…5854` |
| `cargo horizon-mod check functionalstorage.hmod` | exit 0 — `native-mod`, `native-1/0b14f8a…/e59da90…`, for 26.3 |
| `cargo horizon-mod fits functionalstorage.hmod --build nightly` | exit 0 — `0b14f8a  made for` |
| in-game (`launcher-game.sh run functionalstorage`) | **PASS + `EXIT:0`** — the whole 0.1.0 scenario (place, store, refuse, menu withdraw, hint, hoppers, comparator, break+place, relaunch) plus the generic-answer phase |
| control (`GAME=$CONTROL_GAME`) | **FAIL + `EXIT:1`** — `cannot give functionalstorage:oak_1` |
| negative (`NEGATIVE=1`) | **FAIL + `EXIT:1`** — `the drawer stored 164, not 0 (the check is alive)` |
| `check-e2e-logs.py functionalstorage-e2e --strict` | `logs=3 ok=3 missing-exit=0 bad-exit=0` |

| Item | Before | After |
|---|---|---|
| package | `functionalstorage.hmod` 94,766 B (0.2.0) | **`functionalstorage.hmod` 97,739 B `9b7d86b920c0d6d09fc292cce3296792150915f7830c3f08ef6b2672cd845854` (0.2.1)** |

**Trap for the next session:** `launcher-game.sh run`/`build` reuse an existing installed build, and the
wrapper cannot tell that a port's **sources** changed (it matches the manifest): after a code edit with
no version bump the game keeps the old package and the e2e silently tests it. Delete the installed
version that carries the mod (`~/.cache/horizon-e2e/root/data/horizon-launcher/versions/native-*`
holding `horizon-native-mods/<mod>/`) before rebuilding, and confirm the export's `src/` holds the new
code.

### Changelog for the resubmission (supersedes #831)

> **0.2.0 — republished as an SDK `.hmod`, superseding #831**, which was closed on 2026-11-18 because
> Horizon retired the `.hnmod` container (#16941; launcher ≥ 0.2.70). One flat `functionalstorage.hmod`
> now carries the code *and* the content (ten 1x1 drawers: blocks, items, models, textures, recipes,
> loot, lang, creative tab, the one-slot drawer menu): `src/lib.rs` is the one `impl mod_api::Hooks`,
> `src/support.rs` the glue, and `src/runtime.rs` is byte-identical to 0.1.0 (capacity 2048 for a
> 64-stack item, the kind lock, the proportional view slot, the comparator signal — 24 tests green).
> Built against SDK `0b14f8a` / mixins `e59da90` for 26.3. What changed for `api = "0.2"`: the mixin
> plan and its 23 per-hook helpers became typed hooks, three Parts and two declarations; the comparator
> reads its answer from the block entity's tick (the hook is given no block-entity handle); an idle
> drawer no longer rewrites its payload every tick; the LAN hand-resend workaround was dropped for the
> documented player-slot writes; and the embedded-content injection is gone because the package carries
> the content itself. Same feature set and the same gaps (no left-click take, no front count, no
> upgrades or other drawer families).

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices,rust-port

## Slice 2 — the left-click take (0.2.2, pin `0a875da`)

### The gap, and why it is this slice

`## Gaps after the migration`, item 2, said:

> **The left-click take stays removed** (the 0.1.0 pin ruling): a block-entity write from
> `left_click_block` was reverted at `1d63e0a`, so a take would duplicate. The click still shows
> "Open the drawer to withdraw"; withdrawal is the menu's result slot.

That ruling named the **retired `.hnmod` engine** at `1d63e0a`. `AGENTS.md` (*Slices*) requires a
blocked item to be re-checked at every new pin, and by 0.2.1 the port already had in-game proof that
this engine keeps block-entity writes from `right_click_block` (the insert), `on_block_placed_by`
(the restore) and the block-entity tick (the reconcile) — so the old verdict was not this pin's
verdict. This slice re-checks it, and the answer is yes: the take ships. It is the last behaviour the
1x1 drawer was missing, so the port is no longer `ported-partial` **for this reason**.

Tier unchanged: **content · machines** (a block-entity interaction of a machine block; no new tier
surface).

The slice was first written as `PLAN-0.2.2.md`, folded into this section and deleted.

### The behaviour, read from upstream (`11edb40`, behaviour only)

| Upstream | File | Rule |
|---|---|---|
| `PlayerInteractEvent.LeftClickBlock` handler | `FunctionalStorage.java:236-246` | a drawer whose **front panel** the ray hits: `onClicked`, then **cancel** — the panel is never dug; any other face digs as usual |
| `ItemControllableDrawerTile.onClicked` | `:82-96` | server only; the ray's **face must be the front**; `removeTicks == 0` (set to 3 on the click, counted down by the tile's tick); `giveItemToPlayer(extractItem(slot, shift ? stackInSlot(slot).getMaxStackSize() : 1))` |
| `BigInventoryHandler.extractItem` | `:79-100` | the last item leaves → the stack is cleared **only when unlocked** (`if (!isLocked()) bigStack.setStack(EMPTY)`); a locked drawer keeps its kind at 0 |
| `ItemHandlerHelper.giveItemToPlayer` | NeoForge | `Inventory.add`: a matching stack with room first, then an empty slot, the rest thrown where the player stands |

### What changed

| File | Change |
|---|---|
| `src/runtime.rs` | `Drawer::extract` and `Drawer::sync_view` keep the kind when the drawer is `locked` and empties. **The file is no longer byte-identical to the retired 0.1.0 copy** (`604dd255…` now): this is a behaviour change of the slice, not a tidy — the old copy cleared the kind unconditionally on every path |
| `src/support.rs` | `left_click` takes and gives; pure `take_count`, `take_ready`; `TAKE_COOLDOWN_TICKS = 3` and the cooldown map in `Core`; `write_drawer` keys the item on `drawer.item(0).is_some()` instead of `amount > 0`; `read_drawer` reads a kind standing at 0; `stash_bulk` packs a retained kind at 0 bulk so a break keeps it |
| `src/lib.rs` | the `left_click_block` doc string (a take now, not a hint) |
| `horizon-mod.toml`, `Cargo.toml` | `0.2.2`; the description names the take; `[native] horizon = 0a875da…` |
| `tools/e2e.sh` | the take phase (one item, hand 1, block unchanged), the axe-on-the-front phase (no dig), the second drawer (lock, sneak take of the whole stack, the retained kind, dirt refused, stone back), the ten-wood block/item phase, the ten-recipe craft phase, the break from a side face, the locked drawer across the relaunch |
| `tools/craft_check.py` | new: crafts all ten carried `<wood>_1` recipes in a crafting table and asserts the result, the inventory, that the ingredients are consumed, and that a stick in the centre crafts nothing |

The take hands the stack over with `PlayerRef::give_stack`, which is the API's
`ItemHandlerHelper.giveItemToPlayer` (`Inventory.add`, then a throw); a refused give (a dead or stale
player) drops the take at the drawer through the existing `eject` instead of destroying it.

### The red proof (before the fix)

`ports/_shared/evidence/functionalstorage/slice2-test-red.log` — quoted verbatim:

```
failures:
    runtime::tests::a_locked_drawer_keeps_its_kind_when_it_is_emptied
    runtime::tests::a_locked_drawer_keeps_its_kind_when_the_view_empties_it
    support::tests::a_taken_stack_joins_its_own_kind_before_an_empty_slot
    support::tests::the_left_click_takes_one_or_a_whole_stack_while_sneaking
    support::tests::the_left_click_window_is_three_ticks

test result: FAILED. 30 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

with, per test:

```
thread 'runtime::tests::a_locked_drawer_keeps_its_kind_when_it_is_emptied' panicked at src/runtime.rs:336:9:
assertion `left == right` failed: a locked drawer keeps its kind
  left: None
 right: Some("minecraft:stone")
```
```
thread 'support::tests::the_left_click_takes_one_or_a_whole_stack_while_sneaking' panicked at src/support.rs:875:5:
not yet implemented: the left-click take rule
```

(The give rule was first written as a local `give_plan` with its own red test —
`a_taken_stack_joins_its_own_kind_before_an_empty_slot`, in the same log — then deleted when the SDK's
`PlayerRef::give_stack` turned out to be exactly `ItemHandlerHelper.giveItemToPlayer`; the game then
proved the give, which is the evidence that matters.)

### Ladder (each step in one `build-slot.sh` lease, `BUILD_LOCAL=1`, `CARGO_INCREMENTAL=0`)

Pin: Horizon `0a875da91dbb0fdcb7ba8c42c11341aa229af51a` (Nightly 2026-11-20) + mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, 26.3.

| Step | Result | Evidence |
|---|---|---|
| `cargo test` (before the fix) | **30 passed, 4 failed** | `slice2-test-red.log` |
| `cargo test` (pin `0b14f8a`) | 34 passed, 0 failed | `slice2-test.log` |
| `cargo test` (pin `0a875da`, current) | **34 passed, 0 failed** | `slice2-test-0a875da.log`, copied to `sdk-test.log` |
| `cargo clippy --all-targets -- -D warnings` | exit 0, 0 warnings | `slice2-clippy-0a875da.log` |
| `cargo fmt --check` (not `--all`) | clean | `slice2-fmt-0a875da.log` |
| `rm -f Cargo.lock && cargo horizon-mod build` ×2 | **byte-identical** (`cmp` empty) | `slice2-build-0a875da-1.log`, `-2.log`, `slice2-build-0a875da-1.hmod` |
| `cargo horizon-mod check functionalstorage.hmod` | exit 0 — `native-mod`, `native-1/0a875da…/e59da90…`, for 26.3 | `slice2-check-fits-0a875da.log` |
| `cargo horizon-mod fits … --build 0a875da` | **`0a875da  made for`** (the pin: a tautology) | same log |
| `cargo horizon-mod fits … --build 0b14f8a` | **`0b14f8a  fits`** (a different build: the real proof) | same log |
| `verify-sdk-package.py functionalstorage` | **1/1** (package, runtime hash, no `Cargo.lock`, tests, clippy, check, fits, skills gate) | run output |

Artifact: `functionalstorage.hmod`, **99,246 B**, sha256
`244f5b8aed23b1fe98cf2bbd348bb011df470bcd2fdc32087df6718f767d1df0`, **97 entries**, no packed
`Cargo.lock` (two builds byte-identical).

### In-game leg (launcher 0.2.92, Nightly `0a875da`, `HORIZON_E2E_INSTANCE=functionalstorage-e2e`)

`ports/_shared/evidence/functionalstorage-e2e/`:

| Leg | Result |
|---|---|
| `e2e-launcher-0.2.2.log` | **`PASS` + `EXIT:0`** |
| `e2e-launcher-0.2.2-control.log` (`GAME=$CONTROL_GAME`) | **`FAIL` + `EXIT:1`** — `cannot give functionalstorage:oak_1` |
| `e2e-launcher-0.2.2-negative.log` (`SELFTEST=1`) | **`FAIL` + `EXIT:1`** — `the drawer stored 159, not 0 (the check is alive)` |
| `check-e2e-logs.py functionalstorage-e2e --strict` | `logs=6  ok=6  missing-exit=0  bad-exit=0` |

The lines that decide this slice (quoted from the passing log):

```
== a left click on the front takes one item and cancels the dig ==
left click: 68 -> 67, stone in the hand: 1, block: functionalstorage:oak_1
== holding an axe on the front panel does not break it (the take cancels the dig) ==
front dig: {"block":"functionalstorage:oak_1[facing=south,open=false]","block_before":"…","picked_up":{"minecraft:stone":1},…}
== a second drawer: locked, a sneaking click takes the whole stack, the kind stays ==
locked readout: functionalstorage:spruce_1: 10/2048 minecraft:stone locked
sneaking left click: 10 -> 0, stone in the hand: 10
emptied locked readout: functionalstorage:spruce_1: 0/2048 minecraft:stone locked
its own kind returns to the locked drawer: 5
== every carried drawer block and item exists (ten woods) ==
10/10 drawer blocks placed and read back, 10/10 items given
== every carried drawer recipe crafts in a table (ten woods) ==
oak: 8 minecraft:oak_planks + chest -> {"count": 1, "item": "functionalstorage:oak_1", "slot": 200}
… (all ten woods) …
8 oak planks + stick -> null
all 10 drawer recipes craft, consume their ingredients, and refuse a wrong shape
== breaking the drawer keeps its contents (survival, so it drops) ==
restored: 160 (was 175; the view spilled as the loose items above)
== the drawer's contents survive a relaunch ==
relaunch: stored 160 -> 160 at functionalstorage:oak_1
relaunch: the locked drawer reads 'functionalstorage:spruce_1: 5/2048 minecraft:stone locked' (was '…')
PASS: … takes one per left click on its front (the dig refused) and a whole stack while sneaking (a locked drawer keeps its kind), …
```

**The headline for the campaign:** a block-entity write from `left_click_block` **persists** at this
pin. The 0.1.0 duplication ruling does not reproduce; the take was re-enabled and the in-game leg
asserts both halves (the item moves, the drawer's amount falls by exactly 1).

### Coverage (parity rule, 2026-11-20)

Every family the package carries is exercised through `horizon-ctl`; the jar's other families are
declared `not ported` with the boundary. `python3 ports/tools/check-coverage.py functionalstorage`
reads this table.

| Family | Status | Evidence |
|---|---|---|
| `block_entity` | tested | e2e "empty drawer readout", "a stack of stone", the ten-wood phase, "the drawer's contents survive a relaunch" |
| `data:license` | tested | `LICENSE` + `LICENSE-NOTICE.md` travel in the package; `verify-sdk-package.py` green (the licence gate) |
| `functionalstorage:oak_1` | tested | e2e "place functionalstorage:oak_1", "a stack of stone", the ten-wood phase, `craft_check.py` oak, break+restore |
| `functionalstorage:spruce_1` | tested | e2e "a second drawer: locked…", the ten-wood phase, `craft_check.py` spruce, break+restore |
| `functionalstorage:birch_1` | tested | e2e the ten-wood phase, `craft_check.py` birch |
| `functionalstorage:jungle_1` | tested | e2e the ten-wood phase, `craft_check.py` jungle |
| `functionalstorage:acacia_1` | tested | e2e the ten-wood phase, `craft_check.py` acacia |
| `functionalstorage:dark_oak_1` | tested | e2e the ten-wood phase, `craft_check.py` dark_oak |
| `functionalstorage:crimson_1` | tested | e2e the ten-wood phase, `craft_check.py` crimson |
| `functionalstorage:warped_1` | tested | e2e the ten-wood phase, `craft_check.py` warped |
| `functionalstorage:mangrove_1` | tested | e2e the ten-wood phase, `craft_check.py` mangrove |
| `functionalstorage:cherry_1` | tested | e2e the ten-wood phase, `craft_check.py` cherry |
| `functionalstorage:blocks/oak_1` | tested | the loot table the break phase asserts: the drawer drops itself with its contents (oak carries the same pool with its own id) |
| `functionalstorage:blocks/spruce_1` | tested | the loot table the break phase asserts: the drawer drops itself with its contents (spruce carries the same pool with its own id) |
| `functionalstorage:blocks/birch_1` | tested | the loot table the break phase asserts: the drawer drops itself with its contents (birch carries the same pool with its own id) |
| `functionalstorage:blocks/jungle_1` | tested | the loot table the break phase asserts: the drawer drops itself with its contents (jungle carries the same pool with its own id) |
| `functionalstorage:blocks/acacia_1` | tested | the loot table the break phase asserts: the drawer drops itself with its contents (acacia carries the same pool with its own id) |
| `functionalstorage:blocks/dark_oak_1` | tested | the loot table the break phase asserts: the drawer drops itself with its contents (dark_oak carries the same pool with its own id) |
| `functionalstorage:blocks/crimson_1` | tested | the loot table the break phase asserts: the drawer drops itself with its contents (crimson carries the same pool with its own id) |
| `functionalstorage:blocks/warped_1` | tested | the loot table the break phase asserts: the drawer drops itself with its contents (warped carries the same pool with its own id) |
| `functionalstorage:blocks/mangrove_1` | tested | the loot table the break phase asserts: the drawer drops itself with its contents (mangrove carries the same pool with its own id) |
| `functionalstorage:blocks/cherry_1` | tested | the loot table the break phase asserts: the drawer drops itself with its contents (cherry carries the same pool with its own id) |
| `block` | not ported | the jar's other drawer families (1x2, 2x2, compacting, fluid, ender, framed, controller, armory cabinet, upgrades) are outside this slice; only the ten 1x1 drawers are carried |
| `item` | not ported | same boundary: the ten drawer items and their `functionalstorage:drawer` component only |
| `loot_table` | not ported | same boundary; the ten carried loot tables are asserted by the break+restore phase |
| `recipe:minecraft:crafting_shaped` | not ported | same boundary; the ten carried `<wood>_1` recipes are crafted in game by `tools/craft_check.py` |
| `recipe:minecraft:crafting_shapeless` | not ported | upgrades are not carried (the obsidian upgrade recipe) |
| `recipe:minecraft:smithing_transform` | not ported | upgrades are not carried (the netherite upgrade) |
| `recipe:functionalstorage:custom_compacting` | not ported | the compacting drawer is not carried |
| `recipe:functionalstorage:copy_components` | not ported | framed drawings are not carried |
| `recipe:functionalstorage:framed_recipe` | not ported | framed drawers are not carried |
| `tag:item` | not ported | the jar's drawer tags (`drawer`, `drawer_1x1`, `drawer_1x2`, `drawer_2x2`, `fluid_drawer`) are not carried; the carried block tag `minecraft:mineable/axe` ships in `data/minecraft/tags/block/mineable/axe.json` |
| `command` | tested | `/functionalstorage info <pos>` — e2e "empty drawer readout" and "a block that is not a drawer gets one generic answer"; the parsed reply is asserted and the command is operator-only |

The ten-wood phase (`setblock` + the parsed `info` reply + a `give` for each id) and
`tools/craft_check.py` were added by this slice: before it, eight of the ten woods and every carried
recipe were named nowhere, i.e. dead data by the parity rule.

### Upstream defects fixed

None in the mod this slice. The locked-kind rule *is* upstream's (`extractItem` clears the stack only
when unlocked) and the port had simplified it away; the port now follows upstream, which is visible
the moment a take empties a locked drawer:
`emptied locked readout: functionalstorage:spruce_1: 0/2048 minecraft:stone locked`.

### Deliberate deviations

- **The front panel is never dug.** Upstream cancels the left click when the ray hits the front panel,
  so a drawer is broken from any other face. The e2e proves both halves: an axe held on the front
  leaves the drawer standing (and takes one item), a click on the west face breaks it.
- **The 3-tick window is the port's tick counter** (`server_tick`'s end), as close to upstream's
  `removeTicks` as the SDK exposes; the unit test pins the rule, the in-game leg pins the take.
- **A locked *fresh* drawer refuses everything** (upstream's `isValid`: `if (isLocked() && fl.isEmpty())
  return false`), so the e2e fills a drawer *before* locking it. A locked drawer that has been emptied
  keeps its kind and takes its own kind back — that is the fixed behaviour above.

### Traps this slice hit (the next port must know)

1. **`ctl tp` moves the client's player; the server can put it back.** After a cancelled dig the tp
   silently did nothing (the player stayed at the old spot) and `look-at` then failed with "no face of
   block … is visible from here". Use `ctl command "tp @s x y z"` whenever a later step depends on the
   position, and print the position when a `look-at` fails.
2. **A face is only aimable from the side it faces.** `look-at block X Y Z up` failed from a player
   whose eye was below the top plane; aim at a face you are actually outside of.
3. **`launcher-game.sh build` inherits the instance's current build.** With the default `e2e`
   instance still on `native-0b14f8a-…`, the launcher refused the new package with *"functionalstorage's
   package was made by an older tool and doesn't say what it uses, so it works on the Nov 20 update
   only"* — the message is about the **instance**, not the package. A per-mod instance
   (`HORIZON_E2E_INSTANCE=functionalstorage-e2e`) builds cleanly; the wrapper's `run` then exports to
   `~/.cache/horizon-e2e/0a875da/functionalstorage`.
4. **The launcher has one build lock.** A second `native-build` while another agent builds answers
   `A native build or cache cleanup is running`; wait for the neighbour's build, do not retry in a loop.
5. **`ItemStack::set_count(0)` is still refused**, so the take hands over a stack whose count is set
   from the drawer, never a decremented copy.

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices
