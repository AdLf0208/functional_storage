#!/usr/bin/env python3
"""Craft every carried 1x1 drawer recipe in a real crafting table.

Called by `tools/e2e.sh` with the open game's control socket. The carried recipe is upstream's
`<wood>_1`: the shaped pattern `PPP / PCP / PPP` with eight planks of that wood around a chest, one
drawer out (`data/functionalstorage/recipe/<wood>_1.json`).

For each of the ten woods it asserts:

1. the grid crafts `<wood>_1` x1 (the result slot, slots 100-108 row-major, result 200);
2. the crafted drawer reaches the inventory when the result is taken;
3. the ingredients are **consumed**: the planks and the chest are gone from the inventory;
4. a wrong shape (a stick where the chest belongs) crafts nothing — the assertion can fail.

Exits 1 with the observed result on the first mismatch.
"""
import json
import subprocess
import sys

RING = [100, 101, 102, 103, 105, 106, 107, 108]
CENTRE = 104
WOODS = [
    "oak",
    "spruce",
    "birch",
    "jungle",
    "acacia",
    "dark_oak",
    "crimson",
    "warped",
    "mangrove",
    "cherry",
]


def main():
    sock, ctl, x, y, z = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5]

    def run(*args):
        out = subprocess.run([ctl, "--socket", sock, *args], text=True, capture_output=True)
        try:
            return json.loads(out.stdout)
        except json.JSONDecodeError:
            raise SystemExit(f"ctl {' '.join(args)}: {out.stdout}{out.stderr}")

    def wait(ticks=4):
        run("wait", str(ticks))

    def slots():
        screen = run("screen")
        if "slots" not in screen:
            raise SystemExit(f"no menu is open: {screen}")
        return screen["slots"]

    def find(item):
        for slot in slots():
            if slot.get("item") == item:
                return slot["slot"]
        raise SystemExit(f"missing ingredient {item} in the inventory")

    def result():
        stack = [s for s in slots() if s["slot"] == 200]
        if not stack or not stack[0].get("item"):
            return None
        return stack[0]

    def setup(items):
        try:
            run("close")
        except SystemExit:
            pass
        wait(5)
        # A fresh table per case, so a failed/untaken result cannot leak into the next grid.
        run("setblock", x, y, z, "minecraft:air")
        run("setblock", x, y, z, "minecraft:crafting_table")
        wait(2)
        # The server `/tp` moves the player even while a screen is closing; `ctl tp` is client-side.
        run("command", f"tp @s {float(x) - 0.5} {y} {float(z) + 0.5}")
        wait(3)
        run("command", "clear @s")
        for item, count in items:
            run("give", item, str(count))
        wait(3)
        run("interact", x, y, z)
        wait(5)

    def place(item, targets):
        slot = find(item)
        run("click-slot", str(slot))
        wait(1)
        for target in targets:
            run("click-slot", str(target), "right")
            wait(1)
        run("click-slot", str(slot))
        wait(2)

    def take_result():
        run("click-slot", "200")
        wait(5)
        run("click-slot", "9")
        wait(3)

    def counts():
        return run("inventory").get("counts", {})

    failures = []

    for wood in WOODS:
        planks = f"minecraft:{wood}_planks"
        drawer = f"functionalstorage:{wood}_1"
        setup([(planks, 8), ("minecraft:chest", 1)])
        place(planks, [str(slot) for slot in RING])
        place("minecraft:chest", [str(CENTRE)])
        got = result()
        print(f"{wood}: 8 {planks} + chest ->", json.dumps(got))
        if not got or got.get("item") != drawer or got.get("count") != 1:
            failures.append(f"{wood}: expected 1 {drawer}, got {got}")
            continue
        take_result()
        held = counts()
        if held.get(drawer, 0) != 1:
            failures.append(f"{wood}: the crafted drawer reached the inventory as {held.get(drawer, 0)}, not 1")
        if held.get(planks, 0) != 0 or held.get("minecraft:chest", 0) != 0:
            failures.append(
                f"{wood}: the ingredients were not consumed "
                f"(planks {held.get(planks, 0)}, chest {held.get('minecraft:chest', 0)})"
            )

    # The negative case: the same grid with a stick where the chest belongs crafts nothing.
    setup([("minecraft:oak_planks", 8), ("minecraft:stick", 1)])
    place("minecraft:oak_planks", [str(slot) for slot in RING])
    place("minecraft:stick", [str(CENTRE)])
    wrong = result()
    print("8 oak planks + stick ->", json.dumps(wrong))
    if wrong is not None:
        failures.append(f"a stick in the centre crafted {wrong['item']} x{wrong['count']}")

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        raise SystemExit(1)
    print(f"all {len(WOODS)} drawer recipes craft, consume their ingredients, and refuse a wrong shape")


main()
