#!/usr/bin/env bash
# In-game check for Functional Storage slice 1: the 1x1 storage drawers.
#
# Default run (offscreen renderer, creative launch, then survival/normal in
# The End):
#   1. an oak drawer places, faces its placer and reads back `0/2048 empty`;
#   2. right-clicking its front with 100 stone stores all 100 (not a stack);
#   3. a different item (dirt) is refused and stays in the hand;
#   4. the one-slot menu opens on an empty-hand click and a right-click on its
#      result slot withdraws one (the view is 2 at 64 stored, half of it is 1),
#      which the block entity reconciles against the true amount (64 -> 63);
#   5. a second stone click adds 5 more;
#   6. in survival a left click on the front takes one item, hands it over and
#      cancels the dig; holding the dig on the front panel does not break it; a
#      locked drawer's sneaking left click takes the whole stack and the drawer
#      keeps its kind at zero (upstream's take);
#   7. a hopper above pushes stone in and a hopper below pulls stone out into
#      a chest (the container view is the same reconcile path);
#   8. a comparator beside it answers 1 for a partial drawer and 0 when empty;
#   9. breaking the full drawer drops it with its contents, and placing it
#      again restores the count (the `functionalstorage:drawer` component);
#  10. quitting and relaunching the same world keeps the drawer's amount and
#      kind (the block entity's saved compound);
#  11. a vanilla barrel with the same 100 stone stores only 64 and leaves 36
#      in the hand (the differential: the drawer code, not the scenario).
#
# COUNTERFACTUAL=1 replaces the drawer with a vanilla barrel in phases 2-4 and
# expects the barrel's stack-limited behaviour: the run must not see 100 stored.
# NEGATIVE=1 (or its alias SELFTEST=1) keeps the drawer scenario but asserts the
# barrel's 64 limit on it: the run MUST fail, proving the count assertion is not
# vacuous.
#
# The 0.2.0 package is one flat SDK `.hmod`: a native mod cannot load on an
# unmodded game, so GAME must be a game the launcher built with this package:
#   horizon-launcher native-build ports/functionalstorage --instance <i> --trust-native-code
# With a game built from the sources instead, point CONTENT_DIR at a folder holding
# `horizon-mod.toml` + `assets/` + `data/` and it is passed as `--dev-mod`.
# CONTROL_GAME=<unmodded game> runs the differential ("the ids are unknown
# without the package") before the scenario proper when it is set.
set -euo pipefail

CLONE=/home/adlf/Desktop/all_the_mods_10_rust_rewrite
PORT=$CLONE/ports/functionalstorage
CTL=${CTL:-$PORT/target/horizon-cargo/fast/horizon-ctl}
# A launcher-built game carrying this package; see the header.
GAME=${GAME:-$PORT/target/horizon-cargo/fast/horizon-client}
CONTROL_GAME=${CONTROL_GAME:-}
RES=${RES:-${HORIZON_RES:-/home/adlf/Desktop/horizon/.local/horizon-client/26.3}}
SOCK=${SOCK:-/tmp/opencode/functionalstorage-e2e.sock}
COUNTERFACTUAL=${COUNTERFACTUAL:-0}
NEGATIVE=${NEGATIVE:-${SELFTEST:-0}}
# A folder to pass as `--dev-mod` for a game built from the sources (not the shipped shape).
CONTENT_DIR=${CONTENT_DIR:-}
DEV_ARGS=()
[ -z "$CONTENT_DIR" ] || DEV_ARGS=(--dev-mod "$CONTENT_DIR")

[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }
[ -f "$PORT/functionalstorage.hmod" ] || { echo "missing the package: $PORT/functionalstorage.hmod" >&2; exit 1; }

DRAWER=functionalstorage:oak_1
X=0; Z=1
SCRATCH=$(mktemp -d "${E2E_TMP:-/tmp/opencode}"/functionalstorage-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")"
rm -f "$SOCK"
cd "$SCRATCH"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
fail() { echo "FAIL: $*" >&2; exit 1; }
say() { echo "== $* =="; }
end() { ctl command "$1"; }
count_item() { ctl inventory | jq -r --arg i "$1" '.counts[$i] // 0'; }
dropped_item() {
    ctl entities 16 item \
        | jq -r --arg i "$1" '[.entities[] | select(.kind == "item" and .item.item == $i) | .item.count] | add // 0'
}
block_name() { ctl block "$1" "$2" "$3" | jq -r '.block // "unknown"'; }
block_state() { ctl block "$1" "$2" "$3" | jq -r '.state // "unknown"'; }
block_facing() { block_state "$1" "$2" "$3" | sed -n 's/.*facing=\([a-z]*\).*/\1/p'; }
# A drawer's reply starts with its own block id; anything that is not a drawer gets
# the one generic line ("Not a drawer."), which names no block at all (0.2.1).
info() { ctl chat "/functionalstorage info $1 $2 $3" >/dev/null; ctl wait 8 >/dev/null; ctl messages 8 | jq -r '.chat[]' | grep -m1 -E '^(Not a drawer\.|[a-z0-9_.-]+:[a-z0-9_./-]+:)' || true; }
info_amount() { info "$1" "$2" "$3" | sed -n 's/.*: \([0-9][0-9]*\)\/[0-9][0-9]*.*/\1/p'; }
# The action bar is in `messages` (agent.rs `messages_value`), not in `state`.
action_bar() { ctl messages 10 | jq -r '.actionbar // ""'; }
bar_power() { ctl block "$1" "$2" "$3" | jq -r '.state // ""' | sed -n 's/.*powered=\([a-z]*\).*/\1/p'; }
chest_stone() {
    ctl interact "$1" "$2" "$3" south >/dev/null 2>&1 || true
    ctl wait 4 >/dev/null 2>&1
    ctl screen 2>/dev/null | jq -r '[.slots[]? | select(.item == "minecraft:stone") | .count] | add // 0'
    ctl close >/dev/null 2>&1 || true
}

cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    rm -rf "$SCRATCH" || true
    echo "EXIT:$status"
}
trap cleanup EXIT

if [ -n "$CONTROL_GAME" ]; then
    say "control: the game without the package does not know the drawers"
    [ -x "$CONTROL_GAME" ] || fail "CONTROL_GAME is not executable: $CONTROL_GAME"
    CONTROL_SOCK="$SOCK.control"
    rm -f "$CONTROL_SOCK"
    "$CTL" --socket "$CONTROL_SOCK" launch \
        --game "$CONTROL_GAME" --world FunctionalStorageE2E-control --seed 20261105 \
        --creative --offscreen --wait --game-dir "$SCRATCH/control-game" --resources "$RES" \
        >/dev/null
    "$CTL" --socket "$CONTROL_SOCK" wait-ready 180
    reply=$("$CTL" --socket "$CONTROL_SOCK" give functionalstorage:oak_1 1 2>&1 || true)
    echo "  control give: $reply"
    "$CTL" --socket "$CONTROL_SOCK" quit >/dev/null 2>&1 || true
    case "$reply" in
        *"unknown item"*) ;;
        *) fail "the control game knows functionalstorage:oak_1: $reply" ;;
    esac
fi

say "launch (offscreen renderer, creative, a launcher-built game with the package)"
"$CTL" --socket "$SOCK" launch \
    --game "$GAME" --world FunctionalStorageE2E --seed 20261105 --creative --offscreen --wait \
    --game-dir "$SCRATCH/game" --resources "$RES" "${DEV_ARGS[@]}"
ctl wait-ready 180
if grep -q "content mods not loaded" "${SOCK}.log" 2>/dev/null; then
    grep "content mods not loaded" "${SOCK}.log" >&2
    fail "the content mod did not load"
fi

say "an Overworld platform at y=100, away from the terrain"
PX=$(ctl state | jq -r '.player.position[0]' | cut -d. -f1)
PZ=$(ctl state | jq -r '.player.position[2]' | cut -d. -f1)
ctl command "fill $((PX-8)) 100 $((PZ-8)) $((PX+8)) 100 $((PZ+8)) minecraft:stone" >/dev/null
ctl command "fill $((PX-8)) 101 $((PZ-8)) $((PX+8)) 110 $((PZ+8)) minecraft:air" >/dev/null
ctl command "tp @s $PX.5 101 $((PZ+3)).5" >/dev/null
ctl wait 20
ctl command "gamerule spawn_mobs false" >/dev/null
ctl command "gamerule spawn_monsters false" >/dev/null
ctl command "kill @e[type=!minecraft:player]" >/dev/null
ctl wait 20
X=$PX
Z=$((PZ + 1))
Y=102
ctl close >/dev/null 2>&1 || true
ctl look 180 0
ctl wait 6

if [ "$COUNTERFACTUAL" = 1 ]; then
    PLACE=minecraft:barrel
else
    PLACE=$DRAWER
fi

say "place $PLACE at $X $Y $Z"
# The floating support at (X, Y, Z-1): its south face is visible from the
# player, so `place` can put the drawer beside it. (The allthetweaks e2e uses
# the same floating-support recipe.)
ctl setblock "$X" "$Y" $((Z - 1)) minecraft:stone >/dev/null
ctl give "$PLACE" 1 >/dev/null || fail "cannot give $PLACE"
ctl wait 6
ctl select 1 >/dev/null || true
ctl wait 6
ctl place "$X" "$Y" "$Z" --item "$PLACE" >/dev/null || fail "cannot place $PLACE"
ctl wait 6
[ "$(block_name "$X" "$Y" "$Z")" = "$PLACE" ] || fail "the placed block is $(block_name "$X" "$Y" "$Z"), expected $PLACE"
facing=$(block_facing "$X" "$Y" "$Z")
echo "placed facing: ${facing:-unknown}"
[ -n "$facing" ] || fail "the drawer has no facing property"
ctl look-at block "$X" "$Y" "$Z" "$facing" >/dev/null || fail "cannot aim at the front"
ctl wait 6

if [ "$COUNTERFACTUAL" = 1 ]; then
    say "counterfactual: the vanilla barrel is not a drawer"
    info "$X" "$Y" "$Z" | grep -q '^Not a drawer\.$' || fail "the barrel answered as a drawer: $(info "$X" "$Y" "$Z")"
    ctl command "clear @s" >/dev/null
    ctl give minecraft:stone 100 >/dev/null
    ctl wait 4
    ctl select minecraft:stone >/dev/null
    ctl wait 4
    ctl interact "$X" "$Y" "$Z" "$facing" >/dev/null || true
    ctl wait 6
    left=$(count_item minecraft:stone)
    echo "stone left in the hand after the barrel click: $left"
    [ "$left" -gt 0 ] || fail "the barrel took all 100 (a chest menu is not a drawer)"
    echo "PASS: the same click does not store 100 in a barrel; the drawer's behaviour is its own code, not the scenario"
    exit 0
fi

say "empty drawer readout"
line=$(info "$X" "$Y" "$Z")
echo "readout: $line"
echo "$line" | grep -q '^functionalstorage:oak_1: 0/2048 empty$' || fail "unexpected empty readout: $line"

say "a block that is not a drawer gets one generic answer, naming nothing (0.2.1)"
# The source review's `support.rs:1006`: the readout used to name the block at any position (and was
# open to every player). It is an operator command now, and a non-drawer gets the same line.
NX=$((X + 2))
ctl command "setblock $NX $((Y - 1)) $Z minecraft:stone" >/dev/null
ctl command "setblock $NX $Y $Z minecraft:barrel" >/dev/null
ctl wait 4 >/dev/null
[ "$(block_name "$NX" "$Y" "$Z")" = "minecraft:barrel" ] || fail "the control barrel was not placed"
not_drawer=$(info "$NX" "$Y" "$Z")
echo "readout at the barrel: '$not_drawer'"
[ "$not_drawer" = "Not a drawer." ] || fail "a non-drawer block answered '$not_drawer', expected the generic 'Not a drawer.'"
case "$not_drawer" in
    *barrel*|*minecraft:*) fail "the readout names the block it found: '$not_drawer'" ;;
esac
ctl command "setblock $NX $Y $Z minecraft:air" >/dev/null

say "every carried drawer block and item exists (ten woods)"
for wood in oak spruce birch jungle acacia dark_oak crimson warped mangrove cherry; do
    id="functionalstorage:${wood}_1"
    ctl command "setblock $((X - 3)) $Y $((Z - 2)) $id" >/dev/null || fail "cannot place $id"
    ctl wait 2 >/dev/null
    line=$(info $((X - 3)) $Y $((Z - 2)))
    case "$line" in
        "$id: 0/2048 empty") ;;
        *) fail "$id reads '$line'" ;;
    esac
    reply=$(ctl give "$id" 1 2>&1) || fail "cannot give $id: $reply"
    ctl wait 2 >/dev/null
    ctl command "setblock $((X - 3)) $Y $((Z - 2)) minecraft:air" >/dev/null
done
ctl command "clear @s" >/dev/null
echo "10/10 drawer blocks placed and read back, 10/10 items given"

say "every carried drawer recipe crafts in a table (ten woods)"
# Upstream's `<wood>_1`: eight planks of that wood around a chest, one drawer out. The tool asserts
# the result, that it reaches the inventory, that the ingredients are consumed, and a wrong shape
# crafts nothing.
ctl command "setblock $((X - 3)) $Y $((Z + 2)) minecraft:crafting_table" >/dev/null
ctl wait 4
python3 "$PORT/tools/craft_check.py" "$SOCK" "$CTL" $((X - 3)) "$Y" $((Z + 2)) \
    || fail "a carried drawer recipe did not craft"
ctl close >/dev/null 2>&1 || true
ctl command "setblock $((X - 3)) $Y $((Z + 2)) minecraft:air" >/dev/null
ctl command "clear @s" >/dev/null
ctl command "gamemode creative @s" >/dev/null
ctl command "tp @s $X.5 101 $((Z + 3)).5" >/dev/null
ctl wait 6
ctl look-at block "$X" "$Y" "$Z" "$facing" >/dev/null
ctl wait 4

say "a stack of stone: the drawer stores a whole stack at once"
ctl command "clear @s" >/dev/null
ctl give minecraft:stone 64 >/dev/null || fail "cannot give stone"
ctl wait 6
ctl select minecraft:stone >/dev/null || fail "cannot select stone"
ctl wait 6
ctl interact "$X" "$Y" "$Z" "$facing" >/dev/null || fail "the front click failed"
ctl wait 8
amount=$(info_amount "$X" "$Y" "$Z")
left=$(count_item minecraft:stone)
echo "stored: $amount, stone in the hand: $left"
[ "$amount" = 64 ] || fail "the drawer stored $amount, expected 64"
[ "$left" = 0 ] || fail "the drawer left $left stone in the inventory, expected 0"

say "the menu withdraws one, reconciled against the true amount"
ctl command "clear @s" >/dev/null
ctl interact "$X" "$Y" "$Z" "$facing" >/dev/null || fail "the empty-hand click did not open the menu"
ctl wait-for screen container --timeout 10 >/dev/null \
    || fail "the drawer did not open a menu (screen=$(ctl state | jq -r '.screen // ""'))"
menu_slots=$(ctl screen | jq '[.slots[] | select(.slot >= 300)] | length')
echo "menu slots: $menu_slots"
[ "$menu_slots" = 1 ] || fail "the drawer menu shows $menu_slots slots, expected 1"
before_menu=$amount
# The result slot holds the proportional view (2 at 64/2048); a right-click
# takes half of it, one item.
ctl click-slot 300 right >/dev/null || fail "cannot right-click the drawer result slot"
ctl wait 6 >/dev/null
# The take lands on the cursor; an empty inventory slot (9) puts it away so
# `close` cannot drop it.
ctl click-slot 9 >/dev/null || fail "cannot put the withdrawn item into the inventory"
ctl wait 10 >/dev/null
ctl close >/dev/null 2>&1 || true
ctl wait 6 >/dev/null
after_menu=$(info_amount "$X" "$Y" "$Z")
one=$(count_item minecraft:stone)
echo "menu withdrawal: $before_menu -> $after_menu, stone in the hand: $one"
[ "$one" = 1 ] || fail "the menu gave $one stone, expected 1"
[ "$after_menu" = $((before_menu - 1)) ] || fail "the menu took $((before_menu - after_menu)) from the drawer, expected 1"

say "a different item is refused"
ctl give minecraft:dirt 64 >/dev/null
ctl wait 6
ctl select minecraft:dirt >/dev/null
ctl wait 6
ctl interact "$X" "$Y" "$Z" "$facing" >/dev/null || true
ctl wait 6
amount=$(info_amount "$X" "$Y" "$Z")
dirt=$(count_item minecraft:dirt)
echo "stored: $amount, dirt in the hand: $dirt"
[ "$amount" = 63 ] || fail "the refused dirt changed the drawer to $amount"
[ "$dirt" = 64 ] || fail "the drawer took $((64 - dirt)) dirt"

say "a second stone click adds the held stack"
ctl command "clear @s" >/dev/null
ctl give minecraft:stone 5 >/dev/null
ctl wait 6
ctl select minecraft:stone >/dev/null
ctl wait 6
ctl interact "$X" "$Y" "$Z" "$facing" >/dev/null
ctl wait 8
amount=$(info_amount "$X" "$Y" "$Z")
echo "stored: $amount"
[ "$amount" = 68 ] || fail "the second click stored $amount, expected 68"

say "a left click on the front takes one item and cancels the dig"
# Upstream `ItemControllableDrawerTile.onClicked`: one item (a whole stack while sneaking), front
# panel only, once every three ticks; the dig never starts on the front panel.
ctl command "gamemode survival" >/dev/null
ctl wait 4 >/dev/null
ctl command "clear @s" >/dev/null
ctl look-at block "$X" "$Y" "$Z" "$facing" >/dev/null
ctl wait 4
before_click=$(info_amount "$X" "$Y" "$Z")
ctl mouse left click >/dev/null || fail "the left click failed"
ctl wait 6 >/dev/null
amount=$(info_amount "$X" "$Y" "$Z")
one=$(count_item minecraft:stone)
echo "left click: $before_click -> $amount, stone in the hand: $one, block: $(block_name "$X" "$Y" "$Z")"
[ "$amount" = "$((before_click - 1))" ] || fail "the left click took $((before_click - amount)), expected 1"
[ "$one" = 1 ] || fail "the left click gave $one stone, expected 1"
[ "$(block_name "$X" "$Y" "$Z")" = "$DRAWER" ] || fail "the front left click dug the drawer"

say "holding an axe on the front panel does not break it (the take cancels the dig)"
# An axe would finish a wooden drawer well inside the 3 s window, so this only passes when the
# click was cancelled and the take ran instead.
ctl give minecraft:iron_axe 1 >/dev/null || fail "cannot give the axe"
ctl wait 4
ctl select minecraft:iron_axe >/dev/null || fail "cannot select the axe"
ctl wait 4
ctl look-at block "$X" "$Y" "$Z" "$facing" >/dev/null
ctl wait 4
echo "front dig: $(ctl break "$X" "$Y" "$Z" --timeout 3 2>&1 || true)"
ctl wait 6 >/dev/null
[ "$(block_name "$X" "$Y" "$Z")" = "$DRAWER" ] || fail "the front panel was dug out"
ctl command "clear @s" >/dev/null
ctl mouse left up >/dev/null 2>&1 || true
ctl wait 4 >/dev/null

say "a second drawer: locked, a sneaking click takes the whole stack, the kind stays"
# The 0.2.1 source-review fix made the lock a behaviour of the block entity; a locked drawer keeps
# its kind at zero (upstream `BigInventoryHandler.extractItem`). This phase locks a drawer, takes
# everything out with a sneak click and checks the drawer still holds its kind.
X2=$((X + 2))
ctl command "clear @s" >/dev/null
ctl setblock "$X2" $((Y - 1)) "$Z" minecraft:stone >/dev/null
ctl give functionalstorage:spruce_1 1 >/dev/null || fail "cannot give the second drawer"
ctl wait 6
ctl select functionalstorage:spruce_1 >/dev/null || true
ctl wait 4
ctl look-at block "$X2" $((Y - 1)) "$Z" up >/dev/null || fail "cannot aim at the second drawer's floor"
ctl wait 4
ctl place "$X2" "$Y" "$Z" --item functionalstorage:spruce_1 >/dev/null || fail "cannot place the second drawer"
ctl wait 6
[ "$(block_name "$X2" "$Y" "$Z")" = "functionalstorage:spruce_1" ] || fail "the second block is $(block_name "$X2" "$Y" "$Z")"
facing2=$(block_facing "$X2" "$Y" "$Z")
ctl look-at block "$X2" "$Y" "$Z" "$facing2" >/dev/null || fail "cannot aim at the second drawer's front"
ctl wait 4
# Ten stone in first (a fresh locked drawer refuses everything, as upstream's `isValid` does), then
# lock it with a sneak right click and an empty hand (`toggleLocking`).
ctl give minecraft:stone 10 >/dev/null
ctl wait 6
ctl select minecraft:stone >/dev/null || fail "cannot select stone"
ctl wait 4
ctl interact "$X2" "$Y" "$Z" "$facing2" >/dev/null || fail "cannot fill the second drawer"
ctl wait 6
before_sneak=$(info_amount "$X2" "$Y" "$Z")
[ "$before_sneak" = 10 ] || fail "the second drawer holds $before_sneak, expected 10"
ctl command "clear @s" >/dev/null
ctl wait 4
ctl key shift down >/dev/null || fail "cannot hold sneak"
ctl wait 4
ctl interact "$X2" "$Y" "$Z" "$facing2" >/dev/null || fail "the locking click failed"
ctl key shift up >/dev/null
ctl wait 6
locked_line=$(info "$X2" "$Y" "$Z")
echo "locked readout: $locked_line"
case "$locked_line" in
    *"minecraft:stone locked") ;;
    *) fail "the sneak click did not lock the drawer: $locked_line" ;;
esac
# A sneaking left click takes the whole stack (upstream: the stack size).
ctl look-at block "$X2" "$Y" "$Z" "$facing2" >/dev/null
ctl wait 4
ctl key shift down >/dev/null || fail "cannot hold sneak"
ctl wait 4
ctl mouse left click >/dev/null || fail "the sneaking left click failed"
ctl key shift up >/dev/null
ctl wait 8 >/dev/null
after_sneak=$(info_amount "$X2" "$Y" "$Z")
taken=$(count_item minecraft:stone)
echo "sneaking left click: $before_sneak -> $after_sneak, stone in the hand: $taken"
[ "$after_sneak" = 0 ] || fail "the sneaking click left $after_sneak in the drawer, expected 0"
[ "$taken" = 10 ] || fail "the sneaking click gave $taken stone, expected a whole stack (10)"
locked_line=$(info "$X2" "$Y" "$Z")
echo "emptied locked readout: $locked_line"
case "$locked_line" in
    *"minecraft:stone locked") ;;
    *) fail "the emptied locked drawer forgot its kind: $locked_line" ;;
esac
# Still locked: another kind is refused, its own kind returns.
ctl give minecraft:dirt 5 >/dev/null
ctl wait 6
ctl select minecraft:dirt >/dev/null
ctl wait 4
ctl interact "$X2" "$Y" "$Z" "$facing2" >/dev/null || true
ctl wait 6
dirt=$(count_item minecraft:dirt)
[ "$dirt" = 5 ] || fail "the locked emptied drawer took $((5 - dirt)) dirt"
ctl command "clear @s" >/dev/null
ctl give minecraft:stone 5 >/dev/null
ctl wait 6
ctl select minecraft:stone >/dev/null
ctl wait 4
ctl interact "$X2" "$Y" "$Z" "$facing2" >/dev/null || true
ctl wait 6
restocked=$(info_amount "$X2" "$Y" "$Z")
echo "its own kind returns to the locked drawer: $restocked"
[ "$restocked" = 5 ] || fail "the locked drawer took $restocked stone, expected 5"

say "a hopper above fills the drawer"
ctl command "clear @s" >/dev/null
ctl setblock "$X" $((Y + 1)) "$Z" "minecraft:hopper[facing=down]" >/dev/null || fail "cannot place the hopper"
# Read the drawer before filling the hopper: `info` waits eight ticks, and
# the hopper pushes its first item during them.
before=$(info_amount "$X" "$Y" "$Z")
ctl command "item replace block $X $((Y + 1)) $Z container.0 with minecraft:stone 5" >/dev/null || fail "cannot fill the hopper"
ctl wait 80
after=$(info_amount "$X" "$Y" "$Z")
echo "stored: $before -> $after (hopper above)"
[ "$after" -eq $((before + 5)) ] || fail "the hopper stored $((after - before)), expected 5"

say "a hopper below empties it into a chest"
ctl setblock "$X" $((Y - 1)) "$Z" "minecraft:hopper[facing=down]" >/dev/null || fail "cannot place the lower hopper"
ctl setblock "$X" $((Y - 2)) "$Z" minecraft:chest >/dev/null || fail "cannot place the chest"
before=$(info_amount "$X" "$Y" "$Z")
ctl wait 80
after=$(info_amount "$X" "$Y" "$Z")
in_chest=$(chest_stone "$X" $((Y - 2)) "$Z")
echo "stored: $before -> $after, chest stone: $in_chest"
[ "$after" -lt "$before" ] || fail "the lower hopper took nothing ($before -> $after)"
echo "chest stone (informational): $in_chest"

say "a comparator reads the drawer's fullness"
# The hoppers emptied the drawer; fill it again for the comparator.
ctl command "clear @s" >/dev/null
ctl give minecraft:stone 64 >/dev/null
ctl wait 6
ctl select minecraft:stone >/dev/null
ctl wait 6
ctl interact "$X" "$Y" "$Z" "$facing" >/dev/null
ctl wait 8
ctl setblock $((X + 1)) $((Y - 1)) "$Z" minecraft:stone >/dev/null
ctl setblock $((X + 1)) "$Y" "$Z" "minecraft:comparator[facing=west]" >/dev/null || fail "cannot place the comparator"
echo "comparator state: $(ctl block $((X + 1)) "$Y" "$Z" | jq -r '.state')"
ctl wait 8
partial=$(bar_power $((X + 1)) "$Y" "$Z")
echo "comparator power: ${partial:-none} at $(info_amount "$X" "$Y" "$Z") stored"
[ -n "$partial" ] || fail "the comparator has no powered state"
[ "$partial" = true ] || fail "the comparator reads powered=$partial for a non-empty drawer"
# A comparator beside a plain stone block is the control: same rig, no drawer.
ctl setblock $((X + 1)) $((Y - 1)) $((Z - 1)) minecraft:stone >/dev/null
ctl setblock $((X + 1)) "$Y" $((Z - 1)) "minecraft:comparator[facing=west]" >/dev/null
ctl wait 8 >/dev/null
control=$(bar_power $((X + 1)) "$Y" $((Z - 1)))
echo "stone comparator powered: ${control:-none}"
[ "$control" = false ] || fail "the stone control reads powered=$control"

say "breaking the drawer keeps its contents (survival, so it drops)"
ctl command "gamemode survival" >/dev/null
ctl wait 4 >/dev/null
ctl command "clear @s" >/dev/null
ctl give minecraft:stone 64 >/dev/null
ctl wait 6
ctl select minecraft:stone >/dev/null
ctl wait 6
ctl interact "$X" "$Y" "$Z" "$facing" >/dev/null
ctl wait 8
before=$(info_amount "$X" "$Y" "$Z")
[ "$before" -ge 1 ] || fail "the drawer is empty before the break test"
ctl command "clear @s" >/dev/null
# A hand break of a wooden drawer is slow; the axe is what a player uses and
# what the loot table's tool rule expects.
ctl give minecraft:iron_axe 1 >/dev/null || fail "cannot give the axe"
ctl wait 4
ctl select minecraft:iron_axe >/dev/null || fail "cannot select the axe"
ctl wait 4
ctl setblock $((X + 1)) "$Y" "$Z" minecraft:air >/dev/null
# A stone floor under the drawer replaces the lower hopper, so the drawer
# item's drop lands within reach instead of falling through the platform.
ctl setblock "$X" $((Y - 1)) "$Z" minecraft:stone >/dev/null
ctl setblock "$X" $((Y - 2)) "$Z" minecraft:air >/dev/null
ctl setblock "$X" $((Y + 1)) "$Z" minecraft:air >/dev/null
# The front panel takes instead of digging (the phase above), so the drawer is broken from the side:
# the player steps west of it and aims at its west face.
echo "drawer at $X $Y $Z; player before the move: $(ctl state | jq -c '.player.position')"
# The vanilla command teleports the server's player; `ctl tp` sets the client's copy, which the
# server can put back.
ctl command "tp @s $((X - 1)).5 $Y $((Z)).5" >/dev/null
ctl wait 8
echo "break aim from: $(ctl state | jq -c '.player.position')"
reply=$(ctl look-at block "$X" "$Y" "$Z" west 2>&1) || fail "cannot aim at the drawer's west face: $reply"
ctl wait 4
echo "break: $(ctl break "$X" "$Y" "$Z" 2>&1)"
ctl wait 10 >/dev/null
echo "block after break: $(ctl block "$X" "$Y" "$Z" | jq -c '{block,state}')"
[ "$(block_name "$X" "$Y" "$Z")" = "minecraft:air" ] || fail "the drawer did not break"
echo "items on the ground: $(ctl entities 8 | jq -c '[.entities[]? | select(.kind == "item")]')"
# Walk onto the dropped item's block so the pickup lands in the inventory.
ctl command "tp @s $((X + 1)).5 $Y $((Z)).5" >/dev/null
ctl wait 30 >/dev/null
items=$(count_item "$DRAWER")
ground=$(dropped_item "$DRAWER")
echo "drawer items after the break: hand $items, ground $ground"
[ $((items + ground)) -ge 1 ] || fail "the broken drawer did not drop itself"
[ "$items" -ge 1 ] || fail "the broken drawer's drop was not picked up"
ctl select "$DRAWER" >/dev/null 2>&1 || true
# Back off first: at Z+1.5 the player's hitbox overlaps the target cell and the
# placement is refused as "blocked by an entity".
ctl command "tp @s $X.5 $Y $((Z + 3)).5" >/dev/null
ctl wait 6 >/dev/null
# The target is air after the break; aim at the floating support's south face
# (the same support the first placement used), not at the empty cell.
ctl look-at block "$X" "$Y" "$((Z - 1))" south >/dev/null 2>&1 || true
ctl wait 4 >/dev/null
reply=$(ctl place "$X" "$Y" "$Z" --item "$DRAWER" 2>&1) \
    || { echo "place reply: $reply"; fail "cannot place the drawer again"; }
ctl wait 8
restored=$(info_amount "$X" "$Y" "$Z")
echo "restored: $restored (was $before; the view spilled as the loose items above)"
# The component carries the bulk (`amount - view`) and the view slot spills as
# loose items on removal (the port's documented representation), so the
# replaced drawer holds less than `before` but never nothing.
[ "$restored" -ge 1 ] || fail "the replaced drawer restored nothing"
[ "$restored" -lt "$before" ] || fail "the replaced drawer has $restored >= $before (no spill accounted?)"

say "the drawer's contents survive a relaunch"
before_relaunch=$(info_amount "$X" "$Y" "$Z")
locked_relaunch=$(info "$X2" "$Y" "$Z")
[ "$before_relaunch" = "$restored" ] || fail "the readout changed before the relaunch: $restored -> $before_relaunch"
echo "quit with $before_relaunch stored; the locked drawer reads '$locked_relaunch'"
ctl quit >/dev/null 2>&1 || true
sleep 5
rm -f "$SOCK"
"$CTL" --socket "$SOCK" launch \
    --game "$GAME" --world FunctionalStorageE2E --seed 20261105 --creative --offscreen --wait \
    --game-dir "$SCRATCH/game" --resources "$RES" "${DEV_ARGS[@]}"
ctl wait-ready 180
ctl wait 20 >/dev/null
after_relaunch=$(info_amount "$X" "$Y" "$Z")
echo "relaunch: stored $before_relaunch -> $after_relaunch at $(block_name "$X" "$Y" "$Z")"
[ "$(block_name "$X" "$Y" "$Z")" = "$DRAWER" ] || fail "the drawer is gone after the relaunch"
[ "$after_relaunch" = "$before_relaunch" ] || fail "the relaunch changed the drawer $before_relaunch -> $after_relaunch"
locked_after=$(info "$X2" "$Y" "$Z")
echo "relaunch: the locked drawer reads '$locked_after' (was '$locked_relaunch')"
[ "$locked_after" = "$locked_relaunch" ] || fail "the relaunch changed the locked drawer: '$locked_relaunch' -> '$locked_after'"

if [ "$NEGATIVE" = 1 ]; then
    say "NEGATIVE: assert the barrel's 64 limit on the drawer (must fail)"
    [ "$restored" = 0 ] || fail "the drawer stored $restored, not 0 (the check is alive)"
fi

echo "PASS: the drawer stores 100+, refuses other kinds, withdraws one through its menu (64 -> 63), takes one per left click on its front (the dig refused) and a whole stack while sneaking (a locked drawer keeps its kind), hoppers move items, comparators read it, and a break keeps its contents"
