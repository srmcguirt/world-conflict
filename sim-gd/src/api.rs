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
