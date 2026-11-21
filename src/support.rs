//! Game-side glue for Functional Storage slice 1 (the 1x1 storage drawers).
//!
//! [`crate::runtime`] is the pure engine; this module is the only place that reads the game and
//! writes it back: the block entity's saved compound and view slot, the one-slot menu and its
//! screen, the insert/sweep/lock clicks, the break-and-place contents, the comparator reading and
//! the `/functionalstorage info` readout.
//!
//! The pin's container slot is one `ItemStack` (at most a stack), while a drawer holds up to 2048.
//! The block entity's saved compound keeps the true amount and the stored stack (components
//! included); the container slot is a proportional view of it (`runtime::Drawer::view_count`). Every
//! external change of that slot (a hopper, a menu take) is reconciled in the block entity's tick
//! (`runtime::Drawer::sync_view`).
//!
//! A broken drawer's contents ride its item component (`functionalstorage:drawer`):
//! `break_block`/`explosion_detonate` read the block entity before it goes and stash the **bulk**
//! (`amount − view`), the container's removal spills the view, `loot_drops` attaches the stash to
//! the drawer item drop, and `ItemPart::on_block_placed_by` restores it.
//!
//! The 0.2.0 package replaced the retired `.hnmod` bundle; the mapping from the old hook helpers to
//! this module is in `notes.md` under `## Migration to the SDK format`.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use mod_api::{
    AnalogOutputEvent, BlockEntityPart, BlockEntityRef, BlockEntitySubmitEvent,
    BlockEntityTickEvent, BlockEntityTickReply, BlockEntityTicksEvent, BlockEntityUpdateTagEvent,
    BlockPart, BlockState, ClientHost, Command, CommandArgumentValue, Component, Compound, Flow,
    Hand, HasAnalogOutputEvent, Host, HostBlockStates, HostSide, HostTransaction,
    InteractionResult, ItemDisplayContext, ItemPart, ItemStack, LevelHost, LevelRef, ModContainer,
    NeighbourUpdate, OnBlockPlacedByEvent, Registrar, RunModCommandEvent, Screen, ScreenDrawEvent,
    ScreenEvent, ServerHost, ServerTickEvent, ServerTickPhase, StateForPlacementEvent, Tag, Text,
};

use crate::runtime::{Drawer, Sync, capacity};

/// The menu type the carried content declares (`data/functionalstorage/horizon/menu/drawer.json`).
pub const DRAWER_MENU: &str = "functionalstorage:drawer";
/// The item component carrying a broken drawer's contents.
pub const DRAWER_COMPONENT: &str = "functionalstorage:drawer";
/// The block entity type's path, in this mod's namespace.
const DRAWER_TYPE: &str = "drawer";
/// The block entity's saved keys.
const KEY_ITEM: &str = "item";
const KEY_AMOUNT: &str = "amount";
const KEY_LOCKED: &str = "locked";
const KEY_LIMIT: &str = "limit";
/// How long an unconsumed break stash is kept before the server tick drops it.
const STASH_LIFETIME: Duration = Duration::from_secs(2);
/// How many ticks a comparator reading is kept without its drawer ticking (10 s).
const SIGNAL_LIFETIME: u64 = 200;
/// Upstream's double-click window for the whole-inventory sweep.
const SWEEP_WINDOW: Duration = Duration::from_millis(300);
/// Upstream's `removeTicks`: a left click on a drawer's front is handled once every three ticks.
const TAKE_COOLDOWN_TICKS: u64 = 3;

/// The ten drawers' paths (the block entity type's `blocks` list), without the namespace.
const DRAWER_PATHS: &[&str] = &[
    "oak_1",
    "spruce_1",
    "birch_1",
    "jungle_1",
    "acacia_1",
    "dark_oak_1",
    "crimson_1",
    "warped_1",
    "mangrove_1",
    "cherry_1",
];

/// A broken drawer's contents, waiting for its loot drop.
struct Stash {
    dimension: String,
    pos: [i32; 3],
    data: Compound,
    at: Instant,
}

/// One drawer's last comparator answer, and the tick it was read at.
#[derive(Clone, Copy)]
struct Signal {
    value: i32,
    seen: u64,
}

/// The glue's per-session state: the break stash, the comparators waiting for an answer refresh,
/// the tick counter and the last whole-inventory insert attempt of each player.
#[derive(Default)]
pub struct Core {
    stash: Vec<Stash>,
    /// `(level, pos)` whose comparator answer changed; refreshed at `level_tick_post`.
    output: Vec<(LevelRef, [i32; 3])>,
    /// The last comparator answer of each drawer, by `(dimension, pos)`.
    signal: HashMap<(String, [i32; 3]), Signal>,
    tick: u64,
    /// The last whole-inventory insert attempt of each player (300 ms).
    last_click: HashMap<u128, Instant>,
    /// The tick each drawer was last left-clicked at, by `(dimension, pos)` (upstream's
    /// `removeTicks`).
    take: HashMap<(String, [i32; 3]), u64>,
}

static STATE: Mutex<Option<Core>> = Mutex::new(None);

/// The shared state; a poisoned lock is fatal, never silently recovered.
fn core() -> std::sync::MutexGuard<'static, Option<Core>> {
    STATE
        .lock()
        .expect("the drawer state lock is never held across a panic")
}

/// Whether `block` is a drawer this slice implements.
fn is_drawer(block: &str) -> bool {
    crate::runtime::is_drawer(block)
}

// ---------------------------------------------------------------------------
// Declarations
// ---------------------------------------------------------------------------

/// Declare the ten drawers: one block entity type covering all of them, one 1-slot container and
/// menu each, the block-entity Part (which answers the block moments too), and the item Parts that
/// restore a placed drawer's contents.
pub fn declare(r: &mut dyn Registrar) {
    let kind = r
        .block_entity_type(DRAWER_TYPE)
        .blocks(DRAWER_PATHS)
        .build();
    for block in crate::runtime::DRAWER_BLOCKS {
        let key = r.block_key(block);
        r.block_container(&key, container());
        // Bind the key first: `item_part(r.item_key(id), part)` is two mutable borrows of the
        // registrar.
        let item = r.item_key(block);
        r.item_part(item, Placed);
    }
    // Without the Part the drawers would sit inert, so a refusal must stop the registration
    // instead of loading a half-working mod; only our own duplicate declaration can refuse here.
    r.block_entity_part(&kind, DrawerPart)
        .expect("the drawer Part attaches to the type this mod just declared");
}

/// One big slot behind the drawer, reachable from every face, and the drawer's one-slot menu:
/// hoppers may both fill and empty the view.
pub fn container() -> ModContainer {
    ModContainer {
        slots: 1,
        faces: std::array::from_fn(|_| vec![0]),
        inputs: vec![0],
        outputs: vec![0],
        menu: Some(DRAWER_MENU.to_owned()),
    }
}

/// Register the component that carries a broken drawer's contents, before content (and so the
/// item's stack codec) loads.
pub fn declare_component(types: &mut Vec<String>) {
    if !types.iter().any(|known| known == DRAWER_COMPONENT) {
        types.push(DRAWER_COMPONENT.to_owned());
    }
}

/// `/functionalstorage info <pos>`: the readout the in-game check asserts on.
///
/// It is an **operator's** command: any position the server has loaded can be named, and the answer
/// names a drawer's stored item and amount, so a normal player has no business reading another
/// machine's contents through chat.
pub fn declare_commands(commands: &mut Vec<Command>) {
    commands.push(Command::op("functionalstorage info <pos:block_pos>", 2));
}

/// The item component a broken drawer's contents ride: the drawer's own saved compound
/// (`item`, `amount`, `locked`, `limit`), so a drawer carried across the migration keeps them.
struct DrawerContents;

impl Component for DrawerContents {
    type Value = Compound;

    fn id(&self) -> &str {
        DRAWER_COMPONENT
    }

    fn decode(&self, value: &Tag) -> Option<Self::Value> {
        match value {
            Tag::Compound(nbt) => Some(nbt.clone()),
            _ => None,
        }
    }

    fn encode(&self, value: Self::Value) -> Tag {
        Tag::Compound(value)
    }
}

/// The Part of every drawer: the block moments (`state_for_placement`, the comparator) and the
/// block-entity moments (the tick, the client tag, the front renderer and the menu's screen).
pub struct DrawerPart;

/// The block entity's block half: a drawer faces its placer, and a comparator reads its fullness.
#[mod_api::horizon_part]
impl BlockPart for DrawerPart {
    /// Upstream's `getNearestLookingDirection().getOpposite()`: the drawer's front faces the player
    /// who placed it.
    fn state_for_placement(
        &self,
        level: &dyn LevelHost,
        event: &mut StateForPlacementEvent<'_>,
    ) -> Option<Option<BlockState>> {
        let block = level.state_block(event.state)?;
        if !is_drawer(block) {
            return None;
        }
        let facing = facing_for_yaw(event.context.yaw);
        // Parse `<block>[facing=<direction>]`, the state the block's own default would take with
        // that property set; `None` leaves vanilla's placement answer alone.
        Some(Some(
            level.block_state_id(&format!("{block}[facing={facing}]"))?,
        ))
    }

    /// `BlockBehaviour.hasAnalogOutputSignal`: a drawer's fullness is readable by a comparator.
    fn has_analog_output(&self, level: &dyn LevelHost, event: &mut HasAnalogOutputEvent) {
        if is_ours(level, event.state) {
            event.output = true;
        }
    }

    /// `BlockBehaviour.getAnalogOutputSignal`: the fullness the drawer's last tick read.
    fn analog_output(&self, level: &dyn LevelHost, event: &mut AnalogOutputEvent) {
        if !is_ours(level, event.state) {
            return;
        }
        let dimension = event.dimension.key().id();
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        let guard = core();
        if let Some(reading) = guard
            .as_ref()
            .and_then(|state| state.signal.get(&(dimension.to_owned(), pos)))
        {
            event.output = reading.value;
        }
    }
}

/// The block entity's half: it ticks, it tells clients what to draw, and it shows the true count.
#[mod_api::horizon_part]
impl BlockEntityPart for DrawerPart {
    /// `EntityBlock.getTicker`: the drawers tick. Never clear a value an earlier mod set.
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick: reconcile the view slot with the true amount, write it back and note the
    /// comparator answer. The payload is left `Changed` only when something moved, so an idle
    /// drawer does not dirty its chunk every tick.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        tick(host, event);
    }

    /// What clients get of a drawer: the item id and one item, for the front renderer.
    ///
    /// The saved stack carries every component it was stored with — written book pages, shulker
    /// contents, potion data — and a chunk-update tag goes to **every** viewer of the chunk, so
    /// sending the saved stack whole disclosed them without opening the drawer. Only the fields the
    /// model needs ride; the rest stays on the server.
    fn block_entity_update_tag(
        &self,
        event: &mut BlockEntityUpdateTagEvent<'_>,
    ) -> Option<Compound> {
        let mut out = Compound::new();
        if let Some(item) = render_item(&event.entity.nbt) {
            out.insert(KEY_ITEM.into(), Tag::Compound(item));
        }
        Some(out)
    }

    /// The drawer's one-slot menu layout.
    fn screen(&self, _host: &dyn Host, _event: &mut ScreenEvent<'_>) -> Option<Screen> {
        Some(screen())
    }

    /// Draws the true count over the drawer menu (the menu's data slots are
    /// `[amount, limit, locked]`) and the lock state under it.
    fn screen_draw(&self, _host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
        draw_screen(event);
    }

    /// Draws the stored item at the drawer's front. The pack has no in-world text helper, so the
    /// count is only in the menu and the action bar.
    fn block_entity_submit(&self, host: &dyn ClientHost, event: &mut BlockEntitySubmitEvent<'_>) {
        submit(host, event);
    }
}

/// The drawer item's Part: a placed drawer takes back the contents its stack carries.
pub struct Placed;

#[mod_api::horizon_part]
impl ItemPart for Placed {
    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        block_placed(host, event);
    }
}

// ---------------------------------------------------------------------------
// The saved compound and the view slot
// ---------------------------------------------------------------------------

/// `key`'s tag, with Java's UTF-16 key spelling (`JavaString` is not exported).
fn tag<'a>(nbt: &'a Compound, key: &str) -> Option<&'a Tag> {
    let units: Vec<u16> = key.encode_utf16().collect();
    nbt.iter()
        .find(|(name, _)| name.0 == units)
        .map(|(_, value)| value)
}

/// The identity key of a stack: its item and every component, as the pure engine compares them.
fn stack_key(host: &dyn Host, stack: &ItemStack) -> String {
    format!(
        "{}|{}|{:?}",
        stack.item().id(host),
        stack.damage(),
        stack.components()
    )
}

/// The item's own stack size, at least 1.
fn max_stack_of(host: &dyn Host, stack: &ItemStack) -> u32 {
    host.max_stack_size(stack.item()).unwrap_or(64).max(1)
}

/// A drawer read from its saved compound, with the stack it stores. A kind that stands at zero is
/// still a kind: a locked drawer keeps it after the last item leaves.
fn read_drawer(host: &dyn Host, nbt: &Compound) -> (Drawer, Option<ItemStack>) {
    let mut drawer = Drawer::new(1);
    drawer.locked = matches!(tag(nbt, KEY_LOCKED), Some(Tag::Byte(1)));
    let stored = match tag(nbt, KEY_ITEM) {
        Some(Tag::Compound(item)) => ItemStack::from_nbt(host, item),
        _ => None,
    };
    if let Some(stack) = &stored {
        let amount = match tag(nbt, KEY_AMOUNT) {
            Some(Tag::Int(n)) if *n > 0 => *n as u32,
            _ => 0,
        };
        let key = stack_key(host, stack);
        if let Some(slot) = drawer.slots.first_mut() {
            slot.item = Some(key);
            slot.amount = amount;
        }
    }
    (drawer, stored)
}

/// Writes a drawer's state into its saved compound: the stored stack at count one, the true
/// amount, the lock and the slot's limit. An emptied **unlocked** drawer drops the item and amount
/// keys; a locked one keeps its kind (at amount 0), as upstream's `extractItem` does.
fn write_drawer(
    host: &dyn Host,
    nbt: &mut Compound,
    drawer: &Drawer,
    stored: Option<&ItemStack>,
    limit: u32,
) {
    match (stored, drawer.item(0).is_some()) {
        (Some(stack), true) => {
            let mut one = stack.clone();
            let _ = one.set_count(1);
            match one.to_nbt(host) {
                Some(item) => {
                    nbt.insert(KEY_ITEM.into(), Tag::Compound(item));
                    nbt.insert(KEY_AMOUNT.into(), Tag::Int(drawer.amount(0) as i32));
                }
                None => {
                    nbt.remove(&KEY_ITEM.into());
                    nbt.remove(&KEY_AMOUNT.into());
                }
            }
        }
        _ => {
            nbt.remove(&KEY_ITEM.into());
            nbt.remove(&KEY_AMOUNT.into());
        }
    }
    nbt.insert(
        KEY_LOCKED.into(),
        Tag::Byte(if drawer.locked { 1 } else { 0 }),
    );
    nbt.insert(KEY_LIMIT.into(), Tag::Int(limit as i32));
}

/// The saved contents of a drawer, as its item component carries them.
fn drawer_data(nbt: &Compound) -> Compound {
    let mut out = Compound::new();
    for key in [KEY_ITEM, KEY_AMOUNT, KEY_LOCKED, KEY_LIMIT] {
        if let Some(value) = tag(nbt, key) {
            out.insert(key.into(), value.clone());
        }
    }
    out
}

/// What a client needs to draw a drawer's stored item: its id, at count one, and nothing else.
///
/// The saved stack is the whole item — components included — and the chunk-update tag reaches every
/// player who can see the chunk; a written book's pages or a shulker's contents must not. The
/// renderer draws the item model, so the id is the field it cannot do without.
fn render_item(nbt: &Compound) -> Option<Compound> {
    let Some(Tag::Compound(item)) = tag(nbt, KEY_ITEM) else {
        return None;
    };
    let mut out = Compound::new();
    out.insert("id".into(), item.get(&"id".into())?.clone());
    out.insert("count".into(), Tag::Int(1));
    Some(out)
}

/// Brings `drawer` up to date with what the container slot holds **now**, and answers the stack the
/// slot holds when the drawer must hand it back (a kind it does not keep).
///
/// The block-entity tick reconciles the slot once a tick, but a menu click or a hopper can take from
/// it between two ticks. A write-back or a break that started from the saved amount alone would
/// restore that take, and a break would pack it into the drop a second time, so both go through
/// here first: [`reconcile`] is the one rule the tick, a click and a break all apply.
fn settle(
    host: &dyn Host,
    entity: BlockEntityRef,
    drawer: &mut Drawer,
    max_stack: u32,
) -> Option<ItemStack> {
    let slot = entity.item_slot(host, 0).ok().flatten();
    let observed = slot
        .as_ref()
        .map(|stack| (stack_key(host, stack), stack.count()));
    let sync = reconcile(
        drawer,
        observed.as_ref().map(|(key, count)| (key.as_str(), *count)),
        max_stack,
    );
    sync.rejected.and(slot)
}

/// The reconciliation rule: the drawer's true amount against what its view slot holds now (`slot` is
/// the stack in it, or `None` when it is empty). A take lowers the amount by the view the drawer last
/// wrote, a matching insert raises it by the difference, and another kind is handed back.
fn reconcile(drawer: &mut Drawer, slot: Option<(&str, u32)>, max_stack: u32) -> Sync {
    drawer.sync_view(0, slot, max_stack)
}

/// The stack the container slot shows for `drawer`: `view_count` of the stored stack, components
/// kept, or `None` when the drawer is empty.
fn view_stack(drawer: &Drawer, stored: Option<&ItemStack>, max_stack: u32) -> Option<ItemStack> {
    let view = drawer.view_count(0, max_stack);
    if view == 0 {
        return None;
    }
    let mut stack = stored?.clone();
    let _ = stack.set_count(view);
    Some(stack)
}

/// The menu's data slots: the true amount, the slot's limit and the lock.
fn menu_data(drawer: &Drawer, max_stack: u32) -> Vec<i32> {
    vec![
        drawer.amount(0) as i32,
        drawer.limit(0, max_stack) as i32,
        i32::from(drawer.locked),
    ]
}

/// The drawer's block entity at `pos`, and its saved compound, when a drawer stands there.
fn read_state(host: &dyn ServerHost, pos: [i32; 3]) -> Option<(Drawer, Option<ItemStack>)> {
    let level = host.level(host.dimension())?;
    let block = level.block(host, pos)?;
    if !is_drawer(&block.id(host)) {
        return None;
    }
    let entity = host.block_entity(level, pos)?;
    let nbt = entity.data(host)?;
    Some(read_drawer(host, &nbt))
}

// ---------------------------------------------------------------------------
// Writing a drawer back
// ---------------------------------------------------------------------------

/// Reads the drawer at `pos` and applies `change`; writes it back. `change` returns a new stored
/// stack when the drawer adopted one.
///
/// The drawer is reconciled with the view slot **before** `change` runs: a menu or a hopper take the
/// tick has not seen yet must not be restored by this write-back.
fn mutate(
    host: &mut dyn ServerHost,
    pos: [i32; 3],
    change: impl FnOnce(&mut Drawer, Option<&ItemStack>) -> Option<ItemStack>,
) -> bool {
    let Some(level) = host.level(host.dimension()) else {
        return false;
    };
    let Some(entity) = host.block_entity(level, pos) else {
        return false;
    };
    let Some(nbt) = entity.data(&*host) else {
        return false;
    };
    let (mut drawer, stored) = read_drawer(&*host, &nbt);
    let max_stack = stored
        .as_ref()
        .map_or(64, |stack| max_stack_of(&*host, stack));
    let rejected = settle(&*host, entity, &mut drawer, max_stack);
    let new_stored = change(&mut drawer, stored.as_ref()).or_else(|| {
        if drawer.item(0).is_some() {
            stored.clone()
        } else {
            None
        }
    });
    write_entity(host, entity, &drawer, new_stored.as_ref());
    // A stack of another kind in the view is handed back rather than overwritten by our own view.
    if let Some(stack) = rejected {
        eject(host, level, pos, stack);
    }
    true
}

/// Writes a drawer back to its block entity: the saved compound, the view slot and the menu's data
/// slots. The menu engine and the tick both read the view, so an outside actor's take is not lost.
fn write_entity(
    host: &mut dyn ServerHost,
    entity: BlockEntityRef,
    drawer: &Drawer,
    stored: Option<&ItemStack>,
) {
    let max_stack = stored.map_or(64, |stack| max_stack_of(&*host, stack));
    let view = view_stack(drawer, stored, max_stack);
    let mut nbt = entity.data(&*host).unwrap_or_default();
    write_drawer(&*host, &mut nbt, drawer, stored, max_stack);
    let _ = entity.replace_data(host, nbt);
    let _ = host.transaction(|host, tx| entity.set_item_slot(host, tx, 0, view));
    let _ = entity.set_data_slots(host, menu_data(drawer, max_stack));
}

/// Drops `stack` at the block's centre, as `Block.popResource` does for a stack the drawer refuses.
fn eject(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], stack: ItemStack) {
    if stack.count() == 0 {
        return;
    }
    let _ = level.spawn_item(
        host,
        f64::from(pos[0]) + 0.5,
        f64::from(pos[1]) + 0.5,
        f64::from(pos[2]) + 0.5,
        stack,
        10,
    );
}

// ---------------------------------------------------------------------------
// The tick: reconcile the view slot, note the comparator answer
// ---------------------------------------------------------------------------

/// One server tick of a drawer.
fn tick(host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
    let pos = event.pos;
    if event.entity.items.is_empty() {
        event.entity.items.resize(1, None);
    }
    let Some(level) = level_in_place(&*host) else {
        return;
    };
    let (mut drawer, stored) = read_drawer(&*host, &event.entity.nbt);
    let max_stack = stored
        .as_ref()
        .map_or(64, |stack| max_stack_of(&*host, stack));
    // 1. Reconcile what an outside actor did to the view slot (a hopper, the menu). 2. Write the
    //    drawer back with the view the true amount now implies.
    let observed = event
        .entity
        .items
        .first()
        .and_then(Option::as_ref)
        .map(|stack| (stack_key(&*host, stack), stack.count()));
    let sync = drawer.sync_view(
        0,
        observed.as_ref().map(|(key, count)| (key.as_str(), *count)),
        max_stack,
    );
    if sync.rejected.is_some()
        && let Some(stack) = event.entity.items[0].take()
    {
        eject(host, level, pos, stack);
    }
    let mut new_stored = match drawer.item(0) {
        None => None,
        Some(key) => match &stored {
            Some(stack) if stack_key(&*host, stack) == key => Some(stack.clone()),
            _ => event.entity.items.first().and_then(Option::clone),
        },
    };
    if drawer.item(0).is_none() {
        new_stored = None;
    }
    let before_nbt = event.entity.nbt.clone();
    let before_items = event.entity.items.clone();
    let before_data = event.entity.data.clone();
    write_drawer(
        &*host,
        &mut event.entity.nbt,
        &drawer,
        new_stored.as_ref(),
        max_stack,
    );
    let view = view_stack(&drawer, new_stored.as_ref(), max_stack);
    event.entity.items[0] = view;
    let data = menu_data(&drawer, max_stack);
    if event.entity.data != data {
        event.entity.data = data;
    }
    if event.entity.nbt == before_nbt
        && event.entity.items == before_items
        && event.entity.data == before_data
    {
        *event.reply = BlockEntityTickReply::Unchanged;
    }
    note_signal(host, pos, drawer.signal(max_stack));
}

/// The level whose block entities the server ticks now. A block-entity tick is not told its level;
/// the level in place is the one being ticked.
fn level_in_place(host: &dyn ServerHost) -> Option<LevelRef> {
    host.levels()
        .list_levels()
        .into_iter()
        .find(|level| level.in_place(host))
}

/// Notes a drawer's comparator answer for `analog_output`, and queues one neighbour refresh when
/// it differs from the last one (a load, a click, a hopper).
fn note_signal(host: &mut dyn ServerHost, pos: [i32; 3], value: i32) {
    let dimension = host.dimension().to_owned();
    let Some(level) = host.level(host.dimension()) else {
        return;
    };
    let mut guard = core();
    let state = guard.get_or_insert_with(Core::default);
    let tick = state.tick;
    let previous = state
        .signal
        .insert((dimension, pos), Signal { value, seen: tick });
    if previous.map(|reading| reading.value) != Some(value) && !state.output.contains(&(level, pos))
    {
        state.output.push((level, pos));
    }
}

/// After a level's tick pass: tell the neighbours of every drawer whose answer changed to read it
/// again (`Level.updateNeighbourForOutputSignal`).
pub fn level_tick_post(host: HostSide<'_>, event: &mut mod_api::LevelTickPostEvent) {
    let HostSide::Server(server) = host else {
        return;
    };
    let level = event.level;
    let positions = {
        let mut guard = core();
        let state = guard.get_or_insert_with(Core::default);
        let mut keep = Vec::new();
        let mut mine = Vec::new();
        for (entry_level, pos) in std::mem::take(&mut state.output) {
            if entry_level == level {
                mine.push(pos);
            } else {
                keep.push((entry_level, pos));
            }
        }
        state.output = keep;
        mine
    };
    for pos in positions {
        let Some(state) = level.block_state(&*server, pos) else {
            continue;
        };
        let _ = level.update_neighbors(server, pos, state, NeighbourUpdate::OutputSignal);
    }
}

/// End of a server tick: expire a stash no loot roll consumed, age the comparator readings and
/// advance the tick counter that prunes them.
pub fn server_tick(_host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
    if !matches!(event.phase, ServerTickPhase::ServerTickEnd { .. }) {
        return;
    }
    let now = Instant::now();
    let mut guard = core();
    let state = guard.get_or_insert_with(Core::default);
    state.tick = state.tick.wrapping_add(1);
    let tick = state.tick;
    state
        .stash
        .retain(|entry| now.duration_since(entry.at) < STASH_LIFETIME);
    state
        .signal
        .retain(|_, reading| tick.saturating_sub(reading.seen) < SIGNAL_LIFETIME);
    state
        .last_click
        .retain(|_, at| now.duration_since(*at) < SWEEP_WINDOW);
}

// ---------------------------------------------------------------------------
// Player interaction
// ---------------------------------------------------------------------------

/// A right click (`PlayerInteractEvent.RightClickBlock`): the front face inserts the held stack; a
/// second click within 300 ms sweeps the whole inventory; sneak with an empty hand toggles the
/// lock; an empty hand lets the drawer's menu open.
pub fn right_click(host: HostSide<'_>, event: &mut mod_api::RightClickBlockEvent<'_>) -> Flow {
    if event.cancelled {
        return Flow::Continue;
    }
    let click = event.click;
    if click.hand != Hand::Main {
        return Flow::Continue;
    }
    let HostSide::Server(server) = host else {
        return Flow::Continue;
    };
    let pos = click.pos;
    let Some(level) = server.level(server.dimension()) else {
        return Flow::Continue;
    };
    let Some(state) = level.block_state(&*server, pos) else {
        return Flow::Continue;
    };
    let Some(block) = level.block(&*server, pos) else {
        return Flow::Continue;
    };
    if !is_drawer(&block.id(&*server)) {
        return Flow::Continue;
    }
    let facing = {
        let states = server.block_states();
        state
            .property(&states, "facing")
            .ok()
            .flatten()
            .map(str::to_owned)
    };
    let Some(facing) = facing else {
        return Flow::Continue;
    };
    if facing_offset(&facing) != click.hit.face {
        return Flow::Continue;
    }
    let uuid = click.player;
    let Some(player) = server.player(uuid) else {
        return Flow::Continue;
    };
    let sneaking = player.is_secondary_use_active(&*server).unwrap_or(false);
    let Some(held) = click.stack.cloned() else {
        if sneaking
            && mutate(server, pos, |drawer, _| {
                drawer.toggle_locked();
                None
            })
        {
            let locked = read_state(&*server, pos).is_some_and(|(drawer, _)| drawer.locked);
            let _ = player.action_bar(
                server,
                Text::literal(if locked {
                    "Drawer locked"
                } else {
                    "Drawer unlocked"
                }),
            );
            *event.cancellation_result = InteractionResult::Success;
            return Flow::Cancel;
        }
        return Flow::Continue;
    };
    let key = stack_key(&*server, &held);
    let max_stack = max_stack_of(&*server, &held);
    let mut inserted = 0;
    let changed = mutate(server, pos, |drawer, stored| {
        inserted = drawer.insert(0, &key, held.count(), max_stack);
        if inserted > 0 && stored.is_none() {
            let mut one = held.clone();
            let _ = one.set_count(1);
            Some(one)
        } else {
            None
        }
    });
    if !changed {
        return Flow::Continue;
    }
    if inserted == 0 {
        *event.cancellation_result = InteractionResult::Success;
        let _ = player.action_bar(server, Text::literal("The drawer holds another item"));
        return Flow::Cancel;
    }
    // What the hand keeps, and the write that takes the stored part out of it. `set_inventory_slot`
    // writes the selected slot as `/item replace` does and sends it with the tick's inventory sync.
    let hand = remaining(&held, inserted);
    if let Some(slot) = player.selected_slot(&*server) {
        let _ = player.set_inventory_slot(server, slot as u32, hand);
    }
    // Upstream's double-click window: a second click within 300 ms fills the drawer from the whole
    // inventory.
    let sweep = {
        let now = Instant::now();
        let mut guard = core();
        let state = guard.get_or_insert_with(Core::default);
        let sweep = state
            .last_click
            .get(&uuid)
            .is_some_and(|then| now.duration_since(*then) < SWEEP_WINDOW);
        state.last_click.insert(uuid, now);
        sweep
    };
    if sweep {
        sweep_inventory(server, pos);
    }
    let (drawer, stored) = read_state(&*server, pos).unwrap_or_default();
    let amount = drawer.amount(0);
    let limit = stored.as_ref().map_or(capacity(64), |stack| {
        drawer.limit(0, max_stack_of(&*server, stack))
    });
    *event.cancellation_result = InteractionResult::Success;
    let _ = player.action_bar(server, Text::literal(format!("{amount} / {limit}")));
    Flow::Cancel
}

/// A left click on a drawer's front panel: upstream's take — one item, or the stored item's whole
/// stack while sneaking — handed to the player, and the dig cancelled so the front panel is never
/// mined. A click on any other face digs the drawer as usual.
///
/// Upstream `ItemControllableDrawerTile.onClicked` runs the take on the server only, only when the
/// ray hits the front face, and at most once every three ticks (`removeTicks`). It clears the stored
/// stack only when the drawer is unlocked, so a locked drawer keeps its kind at zero.
pub fn left_click(host: &mut dyn ServerHost, event: &mut mod_api::LeftClickBlockEvent) -> Flow {
    if event.cancelled {
        return Flow::Continue;
    }
    let pos = event.pos;
    let Some(level) = host.level(host.dimension()) else {
        return Flow::Continue;
    };
    let Some(state) = level.block_state(&*host, pos) else {
        return Flow::Continue;
    };
    let Some(block) = level.block(&*host, pos) else {
        return Flow::Continue;
    };
    if !is_drawer(&block.id(&*host)) {
        return Flow::Continue;
    }
    let facing = {
        let states = host.block_states();
        state
            .property(&states, "facing")
            .ok()
            .flatten()
            .map(str::to_owned)
    };
    let Some(facing) = facing else {
        return Flow::Continue;
    };
    if facing_offset(&facing) != event.face {
        return Flow::Continue;
    }
    // From here on the click is on the front panel: the dig is cancelled (upstream's handler cancels
    // it whenever the ray hits the panel), and the tick window decides the take.
    let dimension = host.dimension().to_owned();
    let now = core().as_ref().map_or(0, |state| state.tick);
    let last = {
        let guard = core();
        guard
            .as_ref()
            .and_then(|state| state.take.get(&(dimension.clone(), pos)).copied())
    };
    if !take_ready(last, now) {
        return Flow::Cancel;
    }
    {
        let mut guard = core();
        let state = guard.get_or_insert_with(Core::default);
        state.take.insert((dimension.clone(), pos), now);
    }
    let Some((_, stored)) = read_state(&*host, pos) else {
        return Flow::Cancel;
    };
    let Some(stored) = stored else {
        return Flow::Cancel;
    };
    let Some(player) = host.players().acting_player() else {
        return Flow::Cancel;
    };
    let max_stack = max_stack_of(&*host, &stored);
    let sneaking = player.is_secondary_use_active(&*host).unwrap_or(false);
    let want = take_count(sneaking, max_stack);
    let mut taken = 0;
    let changed = mutate(host, pos, |drawer, _| {
        taken = drawer.extract(0, want, max_stack);
        None
    });
    if !changed || taken == 0 {
        return Flow::Cancel;
    }
    // The take is the stored stack (its components included) at the count that left the drawer.
    let mut give = stored;
    if give.set_count(taken).is_err() {
        return Flow::Cancel;
    }
    // `PlayerRef::give_stack` is the API's `ItemHandlerHelper.giveItemToPlayer`: `Inventory.add`
    // fills matching stacks, then empty slots, and throws what does not fit from the player's eyes.
    // A refused give (a dead or stale player) drops the take at the drawer instead of destroying it.
    if player.give_stack(host, give.clone()).is_err() {
        eject(host, level, pos, give);
    }
    Flow::Cancel
}

/// How many items a left click takes: one, or the item's whole stack while sneaking
/// (`ItemControllableDrawerTile.onClicked`: `isShiftKeyDown() ? stackInSlot(slot).getMaxStackSize() : 1`).
fn take_count(sneaking: bool, max_stack: u32) -> u32 {
    if sneaking { max_stack.max(1) } else { 1 }
}

/// Whether a drawer may be clicked again: upstream's `removeTicks` is set to 3 on a click and
/// counted down by the tile's tick, so two takes are at least [`TAKE_COOLDOWN_TICKS`] apart.
fn take_ready(last: Option<u64>, now: u64) -> bool {
    match last {
        None => true,
        Some(last) => now.wrapping_sub(last) >= TAKE_COOLDOWN_TICKS,
    }
}

/// What is left of `stack` after `taken` of it left: `None` when nothing is left.
///
/// `ItemStack::set_count(0)` is **refused** at this pin, so `stack.set_count(count - taken)` leaves an
/// emptied stack at its old count: the drawer stored the stack and the hand kept it, an item
/// duplication the 0.2.1 in-game leg caught (the server still held the 64 stone after the click). An
/// emptied hand is `None`, never a zero-count copy.
fn remaining(stack: &ItemStack, taken: u32) -> Option<ItemStack> {
    let left = kept_count(stack.count(), taken)?;
    let mut rest = stack.clone();
    rest.set_count(left).ok()?;
    Some(rest)
}

/// The count a stack keeps after `taken` of its `have` left: `None` when nothing is left.
///
/// An emptied stack is `None`, never a zero-count copy: `set_count(0)` is refused, so a "keep the
/// rest" written as `stack.set_count(count - taken)` silently keeps the whole stack.
fn kept_count(have: u32, taken: u32) -> Option<u32> {
    let left = have.saturating_sub(taken);
    (left > 0).then_some(left)
}

/// Inserts every matching stack of the acting player's inventory (upstream's 300 ms second click).
fn sweep_inventory(host: &mut dyn ServerHost, pos: [i32; 3]) {
    let Some((_, stored)) = read_state(&*host, pos) else {
        return;
    };
    let Some(stored) = stored else {
        return;
    };
    let key = stack_key(&*host, &stored);
    let max_stack = max_stack_of(&*host, &stored);
    let Some(player) = host.players().acting_player() else {
        return;
    };
    let slots = player.inventory_slots(&*host).unwrap_or_default();
    for (slot, stack) in slots {
        // The main inventory (0..=35) and the off hand (40): the armour and body slots are not
        // swept, as the retired `native_take_items` path did not reach them.
        if !matches!(slot, 0..=35 | 40) {
            continue;
        }
        if stack_key(&*host, &stack) != key {
            continue;
        }
        let mut take = 0;
        let _ = mutate(host, pos, |drawer, _| {
            take = drawer.insert(0, &key, stack.count(), max_stack);
            None
        });
        if take == 0 {
            break;
        }
        let _ = player.set_inventory_slot(host, slot, remaining(&stack, take));
    }
}

// ---------------------------------------------------------------------------
// Breaking, placing and the command
// ---------------------------------------------------------------------------

/// A player breaks a drawer (`BreakBlockEvent`): keep the bulk of its contents for the loot drop.
/// The view slot is left alone: the container's removal spills it, and the two halves add back to
/// the true amount (the 0.1.0 run verified this accounting in game).
pub fn break_block(host: &mut dyn ServerHost, event: &mut mod_api::BreakBlockEvent<'_>) -> Flow {
    if !event.cancelled && is_drawer(&event.state.block().id(&*host)) {
        let dimension = host.dimension().to_owned();
        stash_bulk(host, &dimension, event.pos);
    }
    Flow::Continue
}

/// An explosion is about to break blocks: keep the drawers' bulk for their loot drops.
pub fn explosion_detonate(
    host: &mut dyn ServerHost,
    event: &mut mod_api::ExplosionDetonateEvent<'_>,
) {
    let dimension = event.dimension.to_owned();
    let positions: Vec<[i32; 3]> = event.blocks.clone();
    for pos in positions {
        let Some(level) = host.level(&dimension) else {
            return;
        };
        let Some(block) = level.block(&*host, pos) else {
            continue;
        };
        if is_drawer(&block.id(&*host)) {
            stash_bulk(host, &dimension, pos);
        }
    }
}

/// Stashes the part of a drawer's contents the container does not spill: `amount − view`.
///
/// The drawer is reconciled with the view slot first, like a click's write-back: a take the tick has
/// not seen yet has already left the slot, and packing the saved amount into the drop would hand the
/// withdrawal out a second time.
fn stash_bulk(host: &mut dyn ServerHost, dimension: &str, pos: [i32; 3]) {
    let Some(level) = host.level(dimension) else {
        return;
    };
    let Some(entity) = host.block_entity(level, pos) else {
        return;
    };
    let Some(nbt) = entity.data(&*host) else {
        return;
    };
    if tag(&nbt, KEY_ITEM).is_none() {
        return;
    }
    let (mut drawer, stored) = read_drawer(&*host, &nbt);
    let max_stack = stored
        .as_ref()
        .map_or(64, |stack| max_stack_of(&*host, stack));
    let _ = settle(&*host, entity, &mut drawer, max_stack);
    let view = entity
        .item_slot(&*host, 0)
        .ok()
        .flatten()
        .map_or(0, |stack| stack.count());
    let bulk = drawer.amount(0).saturating_sub(view);
    if bulk == 0 && drawer.item(0).is_none() {
        return;
    }
    let mut data = drawer_data(&nbt);
    data.insert(KEY_AMOUNT.into(), Tag::Int(bulk as i32));
    let dimension = dimension.to_owned();
    let mut guard = core();
    let state = guard.get_or_insert_with(Core::default);
    state
        .stash
        .retain(|entry| !(entry.dimension == dimension && entry.pos == pos));
    state.stash.push(Stash {
        dimension,
        pos,
        data,
        at: Instant::now(),
    });
}

/// Attaches a broken drawer's contents to its item drop.
pub fn loot_drops(host: &mut dyn ServerHost, event: &mut mod_api::LootDropsEvent<'_>) {
    let Some((pos, state)) = event.context.block else {
        return;
    };
    let block = state.block().id(&*host);
    if !is_drawer(&block) {
        return;
    }
    let Some(index) = event
        .drops
        .iter()
        .position(|stack| stack.item().id(&*host) == block)
    else {
        return;
    };
    let dimension = event.context.dimension.to_owned();
    let data = {
        let mut guard = core();
        let state = guard.get_or_insert_with(Core::default);
        let found = state
            .stash
            .iter()
            .position(|entry| entry.dimension == dimension && entry.pos == pos);
        found.map(|index| state.stash.remove(index).data)
    };
    let Some(data) = data else {
        return;
    };
    if data.is_empty() {
        return;
    }
    let _ = event.drops[index].set(&DrawerContents, data);
}

/// A drawer is placed from a stack: restore the contents its component carries.
fn block_placed(host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
    let block = event.state.block().id(&*host);
    if !is_drawer(&block) {
        return;
    }
    let Some(contents) = event.stack.get(&DrawerContents) else {
        return;
    };
    let Some(level) = host.level(host.dimension()) else {
        return;
    };
    let Some(entity) = host.block_entity(level, event.pos) else {
        return;
    };
    let mut data = entity.data(&*host).unwrap_or_default();
    for (key, value) in &contents {
        data.insert(key.clone(), value.clone());
    }
    let _ = entity.replace_data(host, data);
    host.block_entity_sync(event.pos);
}

/// Run `/functionalstorage info <pos>`.
pub fn run_command(host: &mut dyn ServerHost, event: &mut RunModCommandEvent<'_>) -> Flow {
    if event.command.literals.first().map(String::as_str) != Some("functionalstorage") {
        return Flow::Continue;
    }
    if event.command.literals.get(1).map(String::as_str) != Some("info") {
        return Flow::Continue;
    }
    let Some(CommandArgumentValue::BlockPos(pos)) = event.command.argument("pos") else {
        let _ = host
            .commands()
            .feedback(Text::literal("usage: /functionalstorage info <x> <y> <z>"));
        return Flow::Cancel;
    };
    let pos = *pos;
    let line = match read_state(&*host, pos) {
        Some((drawer, stored)) => {
            let max_stack = stored
                .as_ref()
                .map_or(64, |stack| max_stack_of(&*host, stack));
            let item = stored
                .as_ref()
                .map_or_else(|| "empty".to_owned(), |stack| stack.item().id(&*host));
            format!(
                "{}: {}/{} {}{}",
                block_id(host, pos),
                drawer.amount(0),
                drawer.limit(0, max_stack),
                item,
                if drawer.locked { " locked" } else { "" }
            )
        }
        // Anything that is not one of our drawers gets the same generic answer: naming the block
        // would let a caller probe for hidden blocks anywhere the server has it loaded, and the
        // stored item and amount are a drawer's business only.
        None => "Not a drawer.".to_owned(),
    };
    let _ = host.commands().feedback(Text::literal(line));
    Flow::Cancel
}

/// The block id at `pos`, or "" where no level is in place.
fn block_id(host: &dyn ServerHost, pos: [i32; 3]) -> String {
    host.level(host.dimension())
        .and_then(|level| level.block(host, pos))
        .map(|block| block.id(host))
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Client side: the front item and the screen
// ---------------------------------------------------------------------------

/// Draws the stored item at the drawer's front (`BlockEntityRenderer.submit`).
fn submit(host: &dyn ClientHost, event: &mut BlockEntitySubmitEvent<'_>) {
    let Some(Tag::Compound(item)) = tag(&event.entity.nbt, KEY_ITEM) else {
        return;
    };
    let Some(stack) = ItemStack::from_nbt(host, item) else {
        return;
    };
    let facing = {
        let states = host.block_states();
        event
            .state
            .property(&states, "facing")
            .ok()
            .flatten()
            .map(str::to_owned)
    };
    let facing = facing.as_deref().unwrap_or("north");
    let yaw = facing_yaw(facing);
    let offset = facing_offset(facing);
    let draw = &mut *event.draw;
    draw.push();
    draw.translate([
        0.5 + offset[0] as f32 * 0.47,
        0.5,
        0.5 + offset[2] as f32 * 0.47,
    ]);
    draw.rotate_about([0.0, 1.0, 0.0], yaw, [0.0, 0.0, 0.0]);
    let _ = draw.item(&stack, ItemDisplayContext::Fixed, Some(yaw), 0);
    draw.pop();
}

/// The drawer's one-slot screen layout.
fn screen() -> Screen {
    Screen {
        width: 176.0,
        height: 166.0,
        slots: vec![Some([80.0, 20.0])],
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: false,
        // Horizon draws the plain panel and the slot frames (the drawer has no GUI texture).
        own_background: false,
        ..Screen::default()
    }
}

/// Draws the true count and the lock state over the drawer menu.
fn draw_screen(event: &mut ScreenDrawEvent<'_>) {
    let amount = event.data.first().copied().unwrap_or(0);
    let limit = event.data.get(1).copied().unwrap_or(0);
    let locked = event.data.get(2).copied().unwrap_or(0) != 0;
    let [x, y] = event.origin;
    event.draw.text(
        &format!("{amount} / {limit}"),
        [x + 8.0, y + 40.0],
        0xFF40_4040,
        false,
    );
    if locked {
        event
            .draw
            .text("Locked", [x + 8.0, y + 52.0], 0xFF8B_0000, false);
    }
}

// ---------------------------------------------------------------------------
// Placement geometry
// ---------------------------------------------------------------------------

/// The horizontal `facing` a drawer takes when a placer with `yaw` places it: the direction toward
/// the placer (its front faces the player), as upstream's
/// `getNearestLookingDirection().getOpposite()` does.
pub fn facing_for_yaw(yaw: f32) -> &'static str {
    let quarter = ((f64::from(yaw) / 90.0 + 0.5).floor() as i64 & 3) as u8;
    match quarter {
        0 => "north",
        1 => "east",
        2 => "south",
        _ => "west",
    }
}

/// The block face a `facing` property names, as a unit offset.
pub fn facing_offset(facing: &str) -> [i32; 3] {
    match facing {
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "west" => [-1, 0, 0],
        "east" => [1, 0, 0],
        _ => [0, 0, -1],
    }
}

/// The yaw the front model turns through for a `facing` property.
fn facing_yaw(facing: &str) -> f32 {
    match facing {
        "east" => 90.0,
        "south" => 180.0,
        "west" => 270.0,
        _ => 0.0,
    }
}

/// Whether `state` is one of the drawer blocks.
fn is_ours(level: &dyn LevelHost, state: BlockState) -> bool {
    level.state_block(state).is_some_and(is_drawer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ten_carried_blocks_are_declared_by_path() {
        let full: Vec<String> = DRAWER_PATHS
            .iter()
            .map(|path| format!("functionalstorage:{path}"))
            .collect();
        assert_eq!(full, crate::runtime::DRAWER_BLOCKS);
    }

    #[test]
    fn the_facing_of_a_drawer_turns_with_the_placer() {
        assert_eq!(facing_for_yaw(0.0), "north");
        assert_eq!(facing_for_yaw(90.0), "east");
        assert_eq!(facing_for_yaw(180.0), "south");
        assert_eq!(facing_for_yaw(-90.0), "west");
    }

    #[test]
    fn the_front_face_is_the_facing_direction() {
        assert_eq!(facing_offset("north"), [0, 0, -1]);
        assert_eq!(facing_offset("east"), [1, 0, 0]);
        assert_eq!(facing_yaw("south"), 180.0);
    }

    #[test]
    fn the_container_is_one_slot_reached_from_every_face() {
        let container = container();
        assert_eq!(container.slots, 1);
        assert_eq!(container.inputs, vec![0]);
        assert_eq!(container.outputs, vec![0]);
        assert_eq!(container.menu.as_deref(), Some(DRAWER_MENU));
        for face in container.faces {
            assert_eq!(face, vec![0], "every face reaches the one slot");
        }
    }

    #[test]
    fn the_menu_data_slots_are_the_amount_limit_and_lock() {
        let mut drawer = Drawer::new(1);
        assert_eq!(menu_data(&drawer, 64), vec![0, 2048, 0]);
        drawer.insert(0, "minecraft:stone", 100, 64);
        drawer.toggle_locked();
        assert_eq!(menu_data(&drawer, 64), vec![100, 2048, 1]);
    }

    #[test]
    fn a_saved_compound_reads_back_the_amount_the_lock_and_the_view() {
        // The pure half of the codec: the keys a saved drawer carries, without a host.
        let mut nbt = Compound::new();
        nbt.insert(KEY_AMOUNT.into(), Tag::Int(100));
        nbt.insert(KEY_LOCKED.into(), Tag::Byte(1));
        nbt.insert(KEY_LIMIT.into(), Tag::Int(2048));
        assert_eq!(
            match tag(&nbt, KEY_AMOUNT) {
                Some(Tag::Int(n)) => *n,
                _ => 0,
            },
            100
        );
        assert!(matches!(tag(&nbt, KEY_LOCKED), Some(Tag::Byte(1))));
        assert_eq!(
            drawer_data(&nbt).len(),
            3,
            "only the drawer's own keys ride"
        );
        let mut drawer = Drawer::new(1);
        drawer.insert(0, "minecraft:stone", 1024, 64);
        assert_eq!(view_stack(&drawer, None, 64), None);
    }

    #[test]
    fn the_left_click_takes_one_or_a_whole_stack_while_sneaking() {
        // `ItemControllableDrawerTile.onClicked`: 1, or the stored item's stack size while sneaking.
        assert_eq!(take_count(false, 64), 1);
        assert_eq!(take_count(true, 64), 64);
        assert_eq!(take_count(true, 16), 16);
        assert_eq!(take_count(true, 1), 1);
    }

    #[test]
    fn the_left_click_window_is_three_ticks() {
        assert!(take_ready(None, 100), "a drawer never clicked takes");
        assert!(!take_ready(Some(100), 100));
        assert!(!take_ready(Some(100), 102));
        assert!(take_ready(Some(100), 103), "three ticks after the click");
        assert!(take_ready(Some(100), 200));
        assert!(
            take_ready(Some(u64::MAX - 1), 1),
            "a wrapped tick counter must not lock a drawer out"
        );
    }

    #[test]
    fn the_info_command_needs_an_operator() {
        // The readout names a drawer's stored item and amount at any position the server has
        // loaded, so it is an operator's command (the source review's `support.rs:1006` item).
        let mut commands = Vec::new();
        declare_commands(&mut commands);
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].usage, "functionalstorage info <pos:block_pos>");
        assert_eq!(
            commands[0].level, 2,
            "a machine's contents are a game master's readout, not every player's"
        );
    }

    #[test]
    fn a_writeback_after_a_withdraw_does_not_restore_the_withdrawn_count() {
        // The source review's scenario: 1024 stone is a view of 32 in the slot. A menu takes the whole
        // view, and the block-entity tick has not run yet, so the saved amount still says 1024.
        let mut drawer = Drawer::new(1);
        drawer.insert(0, "minecraft:stone", 1024, 64);
        assert_eq!(drawer.view_count(0, 64), 32);
        // A click's write-back goes through `reconcile` first (the fix); without it the amount below
        // is still 1024 and the withdrawal is handed back.
        let sync = reconcile(&mut drawer, None, 64);
        assert!(sync.changed, "the take is accounted for");
        assert_eq!(drawer.amount(0), 992, "1024 - the 32 that left the slot");
        // The click's own stack (64) is what the drawer then stores.
        assert_eq!(drawer.insert(0, "minecraft:stone", 64, 64), 64);
        assert_eq!(drawer.amount(0), 1056, "992 + the 64 the player stored");
    }

    #[test]
    fn a_matching_hopper_insert_is_accounted_for_before_the_writeback() {
        // The other direction: a hopper put one into the view (31 -> 32) before the tick saw it.
        let mut drawer = Drawer::new(1);
        drawer.insert(0, "minecraft:stone", 1000, 64); // view = ceil(1000*64/2048) = 32
        assert_eq!(drawer.view_count(0, 64), 32);
        let _ = reconcile(&mut drawer, Some(("minecraft:stone", 33)), 64);
        assert_eq!(drawer.amount(0), 1001);
    }

    #[test]
    fn a_break_packs_only_what_the_slot_still_shows() {
        // The break half of the finding: the drop carries `amount - view`, so the amount must be the
        // reconciled one. The menu took the view (32) and the amount is still 1024 until a write-back
        // or a break reconciles it; the bulk is then 992 - 0, not 1024 - 0.
        let mut drawer = Drawer::new(1);
        drawer.insert(0, "minecraft:stone", 1024, 64);
        let _ = reconcile(&mut drawer, None, 64);
        // The slot is empty, so the container's removal spills nothing and `stash_bulk` packs
        // `amount - slot_count`.
        let slot_count = 0;
        assert_eq!(
            drawer.amount(0).saturating_sub(slot_count),
            992,
            "the withdrawn 32 is not packed into the drop a second time"
        );
    }

    #[test]
    fn an_emptied_hand_is_none_not_a_zero_count_copy() {
        // `set_count(0)` is refused at this pin, so `held.set_count(held.count() - inserted)` left an
        // emptied hand holding the whole stack while the drawer had it: the source review's write-back
        // item, caught by the in-game leg as an item duplication (64 stored, 64 still in the hand).
        assert_eq!(kept_count(64, 64), None, "the emptied hand is empty");
        assert_eq!(kept_count(64, 10), Some(54));
        assert_eq!(
            kept_count(64, 0),
            Some(64),
            "a click that stored nothing leaves the stack alone"
        );
        assert_eq!(
            kept_count(64, 100),
            None,
            "more taken than there was leaves nothing, never a negative count"
        );
    }

    #[test]
    fn the_client_update_carries_only_what_the_front_draws() {
        // A written book in a drawer: its pages are components of the saved stack, and the
        // chunk-update tag reaches every viewer of the chunk.
        let mut book = Compound::new();
        book.insert("id".into(), Tag::String("minecraft:written_book".into()));
        book.insert("count".into(), Tag::Int(1));
        let mut components = Compound::new();
        components.insert(
            "minecraft:written_book_content".into(),
            Tag::String("the pages".into()),
        );
        book.insert("components".into(), Tag::Compound(components));
        let mut nbt = Compound::new();
        nbt.insert(KEY_ITEM.into(), Tag::Compound(book));
        nbt.insert(KEY_AMOUNT.into(), Tag::Int(16));

        let sent = render_item(&nbt).expect("the front still gets an item");
        assert_eq!(sent.len(), 2, "exactly `id` and `count` ride: {sent:?}");
        assert_eq!(
            sent.get(&"id".into()),
            Some(&Tag::String("minecraft:written_book".into())),
            "the model needs the item id"
        );
        assert_eq!(sent.get(&"count".into()), Some(&Tag::Int(1)));
        assert!(
            !sent.contains_key(&"components".into()),
            "book pages (and any nested container) stay on the server"
        );
        // An empty drawer sends no item at all.
        assert_eq!(render_item(&Compound::new()), None);
    }
}
