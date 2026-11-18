# Validation

> **0.2.0 (SDK package) — current.** SDK nightly `0b14f8acfd97fa2c750e42795249dcc981178ca4`, mixins
> `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, 2026-11-18. `cargo test` **24 passed** (18 runtime, 6
> support), clippy 0 warnings, `cargo fmt --check` clean, `cargo horizon-mod build` twice
> **byte-identical** — `functionalstorage.hmod` **94,766 B**, sha256
> `4ef7d482ae08a16bfb48270101861b4669ded2375e5657ff31dedff1c7a01996`, **97 entries**, `check` and
> `fits` exit 0. **No 0.2.0 in-game run**: `horizon-launcher` is not installed here, so no launcher-built
> game carries the package (`../_shared/evidence/functionalstorage/sdk-e2e-not-run.md`). The ladder for
> this format is in `notes.md` → `## Migration to the SDK format`; the evidence is
> `../_shared/evidence/functionalstorage/sdk-*`.
>
> Everything below is the **0.1.0 history** (`.hnmod`, `horizon.py`): its toolchain is retired and its
> commands no longer exist in this tree. Its e2e runs remain the behaviour's evidence for a
> `src/runtime.rs` that the migration did not change.

Linux, Rust 1.97.0, pin **`1d63e0a5a37a7a708eb0b32b49bf0dfeb28ed8a9`**
(horizon-mixins `5a64af938ec86b00d53d16a408aabc0038fb3e57`, schema 6),
2026-11-08. History: the port was first validated at `83ad384f` (mixins
`e8cd0aa`) and last at the nightly `c9d474dd`; both are superseded by this run.

| Check | Result |
|---|---|
| `cargo fmt --all --check` | Pass |
| `cargo test --locked --offline` (runtime) | **18 passed, 0 failed** |
| `cargo clippy --all-targets --locked --offline -- -D warnings` | Pass, **0 warnings** |
| `python3 tools/horizon.py stage` (pin `1d63e0a`) | `status: ok`, **22 injections, 2 modules**; all 22 hooks resolve in `native-hooks.json` |
| `python3 tools/horizon.py check` | `cargo_exit: 0`; 0 warnings |
| `python3 tools/horizon.py test` (`--lib`) | `test result: ok. 18 passed; 0 failed; 1243 filtered out` |
| `python3 tools/horizon.py build` (fast) | `cargo_exit: 0` |
| `cargo horizon-mod build` (content, pin crate) | `content/functionalstorage.hmod`, **55,499 B**, SHA-256 `924b07602e5436bdf17f5820d2a26b3d290bec25917cb2a7b8fd1ec24eefa677` (byte-identical to the previous artifact) |
| `python3 tools/package.py` (native) | `functionalstorage.hnmod`, **24,028 B**, SHA-256 `64b001b1dbb5f98903e1b3cc23e5090e0b9182cbcd8686e59b0388416f284321`, 26 entries |
| `.hmod` load check | shared game + `--dev-mod functionalstorage.hmod`: item and block exist, `PASS` |
| `.hmod` negative control | same game/world without `--dev-mod`: `unknown item` / `unknown block name`, `PASS` |
| `tools/e2e.sh` | **PASS, `EXIT:0`**: stores/refuses, menu withdrawal 64 → 63, left click hint + no items, hoppers, comparator + stone control, break drop + spill, relaunch 163 → 163 |
| `COUNTERFACTUAL=1 tools/e2e.sh` | **PASS, `EXIT:0`**: the same click stores nothing in a barrel |
| `NEGATIVE=1 tools/e2e.sh` | **`EXIT:1`**, `FAIL: the drawer stored 163, not 0 (the check is alive)` — fail-capable |

Evidence logs: `ports/_shared/evidence/functionalstorage/` (`horizon-check.log`,
`horizon-test.log`, `horizon-build.log`, `repin-e2e-1d63e0a.log`,
`repin-e2e-counterfactual.log`, `repin-e2e-negative.log`, `hmod-load-with.log`,
`hmod-load-without.log`).

Migration deltas at this pin are recorded in `notes.md`: `NativeScreen` gained
`own_background` (set to `false`), the embedded tests moved behind `--lib`, and
the e2e learned the barrel's reply namespace plus a relaunch-persistence phase.

The left-click take is a **pin block**: the take's pure decrement is correct
but the block entity write is reverted in every context tried
(`left_click_block`, the block entity tick, `player_tick`, `server_tick`,
`level_tick_post`, `native_extract_item` from `server_tick`), while the same
`mutate` from `right_click_block` persists. The port removes the left-click
take (hint + menu withdrawal) to avoid duplication. Root cause and probes:
`notes.md`. The e2e asserts the corrected behaviour and passes; the faithful
left-click take stays blocked until the pin exposes an authoritative
left-click/block-entity write, so the port is `ported-partial`.

**Loot-table fix (2026-11-06, still in effect):** the 10 break loot tables were
in the 1.20 `conditions: [{condition: ...}]` shape, which the 26.3 parser skips;
they use the pool-level `condition: {type: ...}` form, so a broken drawer drops
itself.

Skills: superpowers/using-superpowers,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices
