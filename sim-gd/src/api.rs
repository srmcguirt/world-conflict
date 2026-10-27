//! The `Sim` node: the single gdext entry point between Godot and the sim.
//!
//! Contracts (design):
//! - no sim state lives here; `sim_core::World` owns everything;
//! - player actions apply at tick boundaries only, never mid-tick (D9);
//! - every entry point runs inside `catch_unwind`; a caught panic poisons the
//!   Sim so a half-updated world can never be replayed silently (R5/R12).

use std::panic::{self, AssertUnwindSafe};

use godot::prelude::*;
use sim_core::World;
use sim_core::powers::Power;
use sim_core::world::{GRID_HEIGHT, GRID_WIDTH, Overlay};

/// Root simulation node. One per game session, added to the scene tree.
#[derive(GodotClass)]
#[class(no_init, base = Node)]
pub struct Sim {
    world: World,
    /// Set when a panic is caught; blocks further stepping (R5).
    poisoned: bool,
    base: Base<Node>,
}

#[godot_api]
impl Sim {
    /// GDScript constructor: `Sim.create(seed)`.
    #[func]
    fn create(seed: i64) -> Gd<Self> {
        Gd::from_init_fn(|base| Self {
            world: World::new(seed as u64),
            poisoned: false,
            base,
        })
    }

    /// True once a panic has poisoned the Sim; the UI shows the error dialog.
    #[func]
    fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    /// Tile grid width. Wraps east to west (design, Grid).
    #[func]
    fn grid_width(&self) -> i64 {
        GRID_WIDTH as i64
    }

    /// Tile grid height.
    #[func]
    fn grid_height(&self) -> i64 {
        GRID_HEIGHT as i64
    }

    /// Current sim-year.
    #[func]
    fn tick(&self) -> i64 {
        self.world.tick() as i64
    }

    /// Hello-world smoke check: prints from Rust into the Godot console.
    #[func]
    fn hello(&self) {
        godot_print!(
            "[sim] hello from Rust (seed {}, year {})",
            self.world.seed(),
            self.world.tick()
        );
    }

    /// Advances the simulation by `years` whole sim-years.
    ///
    /// `step` never returns mid-tick, so actions applied between calls always
    /// land on a tick boundary (D9).
    #[func]
    fn step(&mut self, years: i64) {
        self.guarded(|world| world.step(years.max(0) as u32));
    }

    /// RGBA bytes for the map image, one 4-byte pixel per tile.
    ///
    /// The UI repaints only when [`revision`](Sim::revision) changes (R6).
    #[func]
    fn paint_overlay(&mut self, mode: i64) -> PackedByteArray {
        match self.guarded(|world| world.paint_overlay(Overlay::from_index(mode))) {
            Some(bytes) => PackedByteArray::from(bytes),
            None => PackedByteArray::new(),
        }
    }

    /// Revision counter for repaint-when-dirty (R6).
    #[func]
    fn revision(&self) -> i64 {
        self.world.revision() as i64
    }

    /// How many peoples exist, for the HUD.
    #[func]
    fn culture_count(&self) -> i64 {
        self.world.cultures().len() as i64
    }

    /// The tile humanity started on, for the H home key (design D2).
    #[func]
    fn eden_tile(&self) -> Vector2i {
        match self.world.eden_tile() {
            Some(tile) => Vector2i::new((tile % GRID_WIDTH) as i32, (tile / GRID_WIDTH) as i32),
            None => Vector2i::new(-1, -1),
        }
    }

    /// The god's own people, or -1 if the world has none.
    #[func]
    fn chosen_culture(&self) -> i64 {
        self.world
            .cultures()
            .iter()
            .find(|culture| culture.favor == sim_core::culture::Favor::Chosen)
            .map(|culture| culture.id as i64)
            .unwrap_or(-1)
    }

    /// A people's name, e.g. "the Karethi".
    #[func]
    fn culture_name(&self, id: i64) -> GString {
        self.world
            .cultures()
            .get(id as usize)
            .map(|culture| GString::from(culture.name.as_str()))
            .unwrap_or_default()
    }

    /// Queues a power from the toolbar. It applies on the next sim-year.
    #[func]
    fn cast_power(&mut self, index: i64, x: i64, y: i64) {
        self.guarded(|world| {
            world.queue_power(
                Power::from_index(index),
                x.max(0) as usize,
                y.max(0) as usize,
            );
        });
    }

    /// A people's scripture, newest first, already rendered by Rust.
    #[func]
    fn scripture(&self, culture_id: i64, from: i64, count: i64) -> PackedStringArray {
        let lines = self.world.scripture_text(
            culture_id.max(0) as u16,
            from.max(0) as usize,
            count.clamp(1, 200) as usize,
        );
        let mut array = PackedStringArray::new();
        for line in lines {
            array.push(&GString::from(line.as_str()));
        }
        array
    }

    /// A people's doctrine, or an empty string before one crystallizes.
    #[func]
    fn culture_doctrine(&self, id: i64) -> GString {
        self.world
            .cultures()
            .get(id as usize)
            .and_then(|culture| culture.doctrine)
            .map(|doctrine| GString::from(doctrine.name()))
            .unwrap_or_default()
    }

    /// A people's piety, 0 to 1.
    #[func]
    fn culture_piety(&self, id: i64) -> f64 {
        self.world
            .cultures()
            .get(id as usize)
            .map(|culture| culture.piety as f64)
            .unwrap_or(0.0)
    }

    /// How many religions exist.
    #[func]
    fn faith_count(&self) -> i64 {
        self.world.faiths().len() as i64
    }

    /// Anoints a people, at the next tick (design D11).
    #[func]
    fn anoint(&mut self, culture_id: i64) {
        self.guarded(|world| world.queue_anoint(culture_id.max(0) as u16));
    }

    /// How many witnesses are alive, and the newest witness's name.
    #[func]
    fn witness_count(&self) -> i64 {
        self.world.witnesses().len() as i64
    }

    /// The id of the newest living witness, or -1 when there is none.
    #[func]
    fn newest_witness_id(&self) -> i64 {
        self.world
            .witnesses()
            .last()
            .map(|witness| witness.id as i64)
            .unwrap_or(-1)
    }

    /// A witness's generated pixel portrait: RGBA bytes, see
    /// `portrait_width` and `portrait_height`.
    #[func]
    fn portrait(&self, witness_id: i64) -> PackedByteArray {
        let pixels = self
            .world
            .notables()
            .iter()
            .find(|notable| notable.id == witness_id.max(0) as u32)
            .map(|notable| {
                sim_core::traits::portrait(
                    notable,
                    sim_core::traits::dress_of(&self.world, notable.culture_id),
                )
            })
            .unwrap_or_default();
        PackedByteArray::from(pixels)
    }

    /// The portrait's width, in pixels.
    #[func]
    fn portrait_width(&self) -> i64 {
        sim_core::traits::PORTRAIT_W as i64
    }

    /// The portrait's height, in pixels.
    #[func]
    fn portrait_height(&self) -> i64 {
        sim_core::traits::PORTRAIT_H as i64
    }

    /// A people's dress color: their banner warmed by their skin.
    #[func]
    fn culture_dress(&self, id: i64) -> Color {
        let dress = sim_core::traits::dress_of(&self.world, id.max(0) as u16);
        Color::from_rgba(
            dress[0] as f32 / 255.0,
            dress[1] as f32 / 255.0,
            dress[2] as f32 / 255.0,
            1.0,
        )
    }

    #[func]
    fn newest_witness(&self) -> GString {
        let witnesses = self.world.witnesses();
        match witnesses.last() {
            Some(witness) => {
                let people = self
                    .world
                    .cultures()
                    .get(witness.culture_id as usize)
                    .map(|culture| culture.name.clone())
                    .unwrap_or_default();
                GString::from(format!("{}, of the {}", witness.name, people).as_str())
            }
            None => GString::new(),
        }
    }
}

impl Sim {
    /// Runs `f`, poisoning the Sim if it panics (R5/R12).
    fn guarded<T>(&mut self, f: impl FnOnce(&mut World) -> T) -> Option<T> {
        if self.poisoned {
            godot_error!("[sim] poisoned; call ignored. Load the last autosave or quit.");
            return None;
        }

        match panic::catch_unwind(AssertUnwindSafe(|| f(&mut self.world))) {
            Ok(value) => Some(value),
            Err(_) => {
                self.poisoned = true;
                godot_error!(
                    "[sim] panic caught; the world is poisoned. Load the last autosave or quit."
                );
                None
            }
        }
    }
}
