# Validation

> **0.2.2 (SDK package) — current.** Horizon `0a875da91dbb0fdcb7ba8c42c11341aa229af51a` (Nightly
> 2026-11-20) + mixins `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, 26.3, launcher 0.2.92.
> `cargo test` **34 passed** (20 runtime, 14 support) after a red run of **30 passed / 5 failed**;
> clippy 0 warnings; `cargo fmt --check` clean; two `cargo horizon-mod build` runs **byte-identical** —
> `functionalstorage.hmod` **99,246 B**, sha256
> `244f5b8aed23b1fe98cf2bbd348bb011df470bcd2fdc32087df6718f767d1df0`, **97 entries**, no packed
> `Cargo.lock`; `check` exit 0; `fits --build 0a875da` **`made for`** and `fits --build 0b14f8a`
> **`fits`**; `verify-sdk-package.py` **1/1**. In game (`HORIZON_E2E_INSTANCE=functionalstorage-e2e`):
> **`PASS` + `EXIT:0`**, control **`EXIT:1`**, `SELFTEST=1` **`EXIT:1`**,
> `check-e2e-logs.py functionalstorage-e2e --strict` green. The slice: the faithful left-click take
> (one item, a whole stack while sneaking, front panel only, a 3-tick window), the locked drawer's
> kind kept at zero, and the ten carried blocks/items/recipes each exercised in game. Ladder,
> transcripts and the coverage table: `notes.md` → `## Slice 2 — the left-click take (0.2.2)`.
>
> **0.2.1** was `0b14f8a`/`e59da90`, 97,739 B, `9b7d86b9…5854`; **0.2.0** `0b14f8a`,
> 94,766 B, `4ef7d482…1966` (no in-game run). Both are superseded by 0.2.2; a package built at an
> older pin cannot re-run its e2e under the current wrapper.

**Everything below is the 0.1.0 history** (`.hnmod`, `horizon.py`): its toolchain is retired and its
commands no longer exist in this tree. Its e2e runs remain evidence for the behaviour the migration
and this slice kept.

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

The left-click take was a **0.1.0 pin block**: the take's pure decrement was
correct but the block entity write was reverted in every context tried
(`left_click_block`, the block entity tick, `player_tick`, `server_tick`,
`level_tick_post`, `native_extract_item` from `server_tick`), while the same
`mutate` from `right_click_block` persisted. **Resolved at the SDK pin
(2026-11-21):** a block-entity write from `left_click_block` persists at
`0a875da`, so 0.2.2 restores the faithful take (one item, a whole stack while
sneaking, front panel only, a 3-tick window) and the e2e asserts it. The port is
`ported` for the 1x1 drawers; the remaining scope is the other drawer families
(`notes.md` → `## Coverage`).

**Loot-table fix (2026-11-06, still in effect):** the 10 break loot tables were
in the 1.20 `conditions: [{condition: ...}]` shape, which the 26.3 parser skips;
they use the pool-level `condition: {type: ...}` form, so a broken drawer drops
itself.

Skills: superpowers/using-superpowers,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices
