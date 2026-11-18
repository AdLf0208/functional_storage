# Working on Functional Storage (native Horizon mod, SDK format)

> **Build slots (v2):** every build/test/lint step goes through the campaign runner
> (`ports/tools/build-slot.sh` from the campaign root). It **blocks in the kernel** until a slot
> is free — a freed slot is taken in ~0.07 s — so **never poll**: no `while ! slot; sleep` loop, no
> background build, never `--target-dir`. Read `../_shared/build-slots-usage.md` before running
> anything; observe with `status` / `watch` / `events` / `stats`. The runner does **not** cd into
> `--port`, so pass `-- bash -c 'cd ports/functionalstorage && …'`, and use `BUILD_LOCAL=1` (the far
> box has no copy of this port). The runner's step names speak the retired `.hnmod` tool; for this
> package they only set the thread count.

This repository is the port, not Horizon and not horizon-mixins. Never modify the original
checkout or player resources; do not commit `target/`, `.horizon/`, exported sources or Minecraft
assets.

**There is no staging step and no mixin plan.** One flat `.hmod` holds `horizon-mod.toml` (the only
identity, `api = "0.2"`, `[native]` pins), `Cargo.toml` (its only dependency is `horizon-mod-api`
from the fetched `.horizon/sdk/`), `src/lib.rs` (the one `#[mod_api::horizon_mod] impl Hooks`),
`src/support.rs` (the game glue) and `src/runtime.rs` (the pure logic, unit-tested). `assets/` and
`data/` are the carried content and travel in the same file. The retired `.hnmod` layout is
history: `../_shared/evidence/functionalstorage/retired-hnmod-0.1.0/`.

Keep every gameplay rule and number in `src/runtime.rs` with its test; the glue only reads the game,
calls `runtime`, and writes the result back. **`src/runtime.rs` is byte-identical to the retired
port's file — do not "tidy" it.** A declaration test belongs next to its code in `src/support.rs`.

Validate, in this order, each inside a lease:

```sh
CARGO_INCREMENTAL=0 cargo test                        # 24 tests: 18 runtime, 6 support
cargo clippy --all-targets -- -D warnings
cargo fmt --check                                     # NOT --all: that checks the fetched SDK
rm -f Cargo.lock && cargo horizon-mod build           # target/horizon-mod/functionalstorage.hmod
cp target/horizon-mod/functionalstorage.hmod ./functionalstorage.hmod
cargo horizon-mod check functionalstorage.hmod        # the registry's own check
cargo horizon-mod fits functionalstorage.hmod --build nightly
sha256sum functionalstorage.hmod                      # quote it in notes.md
```

A `Cargo.lock` is packed and changes the artifact: remove it immediately before the build and keep it
gitignored. The in-game leg needs a game the launcher built with this package
(`horizon-launcher native-build`), which is not installed here — do not claim an in-game result
without one.
