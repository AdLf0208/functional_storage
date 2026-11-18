//! Functional Storage for Horizon — the 1x1 storage drawers.
//!
//! Ported from Functional Storage 1.5.8 (`Buuz135/FunctionalStorage`, MIT, Minecraft Java 1.21.1
//! NeoForge). [`runtime`] is the pure engine — the drawer's capacity, kind lock, insertion,
//! extraction, the proportional view slot and the comparator signal, with its tests — and is
//! byte-identical to the retired `.hnmod` port's file. [`support`] is the glue: the block entity,
//! its container and menu, the click/break/place wiring and the readout command.
//!
//! # Where the retired container's hooks went
//!
//! The 2026-11-17 mod-API merge (#16941, launcher ≥ 0.2.70) retired `.hnmod`: the mixin plan, the
//! injection points and the 23 per-hook helper files are gone. A hook is a typed method here, and a
//! moment that belongs to one block or one block entity is a **Part** attached to it.
//!
//! | `.hnmod` hook (API) | here |
//! |---|---|
//! | the `content_root` injection (AE2 pattern) | gone: one flat `.hmod` carries `assets/` and `data/` itself |
//! | `block_entity` (15) | the block entity type declared in [`Mod::register`] (a fresh drawer starts with an empty compound) |
//! | `block_entity_ticks` (15) | [`support::DrawerPart`]'s `BlockEntityPart::block_entity_ticks` |
//! | `block_entity_container` (16/17) | `Registrar::block_container` in [`Mod::register`] |
//! | `block_entity_tick` (17) | `BlockEntityPart::block_entity_tick` |
//! | `block_entity_update_tag` (45) | `BlockEntityPart::block_entity_update_tag` |
//! | `block_entity_submit` (71) | `BlockEntityPart::block_entity_submit` |
//! | `screen` (19) / `screen_draw` (19) | `BlockEntityPart::screen` / `screen_draw` |
//! | `state_for_placement` (76) | `BlockPart::state_for_placement` |
//! | `has_analog_output` / `analog_output` (64) | `BlockPart::has_analog_output` / `analog_output` |
//! | `right_click_block` (21) | [`Mod::right_click_block`] |
//! | `left_click_block` (40) | [`Mod::left_click_block`] |
//! | `destroy_block` (14) | [`Mod::break_block`] (renamed upstream: `BreakBlockEvent`) |
//! | `explosion_detonate` (22) | [`Mod::explosion_detonate`] |
//! | `place_block` (14) | `ItemPart::on_block_placed_by` on [`support::Placed`] |
//! | `loot_drops` (75) | [`Mod::loot_drops`] |
//! | `server_tick` (4) | [`Mod::server_tick`] |
//! | `level_tick_post` (4) | [`Mod::level_tick_post`] |
//! | `component_types` (65) | [`Mod::component_types`] |
//! | `register_commands` (8) | [`Mod::register_commands`] |
//! | `perform_command` (8) | [`Mod::run_mod_command`] (typed arguments, not a split line) |
pub mod runtime;
pub mod support;

use mod_api::{
    self, BreakBlockEvent, ComponentTypesEvent, ExplosionDetonateEvent, Flow, Hooks, HostSide,
    LeftClickBlockEvent, LevelTickPostEvent, LootDropsEvent, RegisterCommandsEvent, Registrar,
    RegistrationContext, RightClickBlockEvent, RunModCommandEvent, ServerHost, ServerTickEvent,
};

/// The mod keeps its state (the break stash, the comparator readings, the click window) in
/// `support`'s shared [`support::Core`], never in this struct: one instance must serve every hook.
#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// Declare what the game answers for the ten drawer blocks (`EntityBlock` plus NeoForge's
    /// `RegisterCapabilitiesEvent`): one block entity type, one 1-slot container and menu each,
    /// the Part that answers the block-entity and block moments, and the item Parts that restore
    /// a drawer's contents on placement.
    fn register(&self, r: &mut dyn Registrar, _cx: &RegistrationContext) {
        support::declare(r);
    }

    /// The item component that carries a broken drawer's contents, registered before content (and
    /// so the item's stack codec) loads.
    fn component_types(&self, _context: &RegistrationContext, event: &mut ComponentTypesEvent<'_>) {
        support::declare_component(event.types);
    }

    /// `/functionalstorage info <x> <y> <z>`.
    fn register_commands(&self, _host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        support::declare_commands(event.commands);
    }

    /// Run the command above and take it.
    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        support::run_command(host, event)
    }

    /// A right click on the drawer's front: insert the held stack, sweep the inventory on a second
    /// click within 300 ms, toggle the lock with a sneak and an empty hand, or let the menu open.
    fn right_click_block(&self, host: HostSide<'_>, event: &mut RightClickBlockEvent<'_>) -> Flow {
        support::right_click(host, event)
    }

    /// A left click on the front: upstream's take-one is refused at this pin (a block-entity write
    /// from this hook is reverted, so the take would duplicate); the click only shows the withdraw
    /// hint, and the menu's result slot is the withdrawal.
    fn left_click_block(&self, host: &mut dyn ServerHost, event: &mut LeftClickBlockEvent) -> Flow {
        support::left_click(host, event)
    }

    /// A player broke a drawer: keep the bulk of its contents for the loot drop.
    fn break_block(&self, host: &mut dyn ServerHost, event: &mut BreakBlockEvent<'_>) -> Flow {
        support::break_block(host, event)
    }

    /// An explosion about to break drawers: same stash for their drops.
    fn explosion_detonate(
        &self,
        host: &mut dyn ServerHost,
        event: &mut ExplosionDetonateEvent<'_>,
    ) {
        support::explosion_detonate(host, event);
    }

    /// The broken drawer's item drop carries its contents component.
    fn loot_drops(&self, host: &mut dyn ServerHost, event: &mut LootDropsEvent<'_>) {
        support::loot_drops(host, event);
    }

    /// End of a server tick: expire a stash that no loot roll consumed, age the comparator readings
    /// and advance the tick counter that prunes them.
    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
        support::server_tick(host, event);
    }

    /// After a level's tick pass: refresh the comparators of the drawers whose signal changed.
    fn level_tick_post(&self, host: HostSide<'_>, event: &mut LevelTickPostEvent) {
        support::level_tick_post(host, event);
    }
}
