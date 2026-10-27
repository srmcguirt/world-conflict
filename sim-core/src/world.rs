//! World state and the fixed tick pipeline.
//!
//! One tick is one sim-year in every era (D24). Systems run in a fixed order:
//! Climate, Population, Migration, Drift/Blend, Traits, Belief, Polity, Tech,
//! War (design, Architecture). Each system is a single `tick(world, rng)`
//! function so it can be tested on its own.

use rand::SeedableRng;
use rand_pcg::Pcg64;

use crate::culture::{self, Culture, NO_CULTURE};
use crate::demography;
use crate::worldgen::{self, Biome, GeneratedWorld};

/// Milestone 1 grid: an equirectangular tile map that wraps east to west.
///
/// The target size is 320x160 (design, Grid); the benchmark gate in Milestone 1
/// decides whether 160x80 is the shipped size.
pub const GRID_WIDTH: usize = 160;
pub const GRID_HEIGHT: usize = 80;

/// Map overlay modes (design, Rendering).
///
/// Only `Biome` exists in Milestone 1; the rest arrive with their systems and
/// fall back to the biome overlay until then.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Overlay {
    Biome,
    Culture,
    Traits,
    Belief,
    Polity,
    Language,
}

impl Overlay {
    /// Maps the UI's overlay index to a mode; unknown values fall back to Biome.
    ///
    /// The index order matches the design's overlay list (biome, culture,
    /// traits, belief, polity, language).
    pub fn from_index(index: i64) -> Self {
        match index {
            1 => Overlay::Culture,
            2 => Overlay::Traits,
            3 => Overlay::Belief,
            4 => Overlay::Polity,
            5 => Overlay::Language,
            _ => Overlay::Biome,
        }
    }
}

/// The simulation world: every mutable field the sim owns.
///
/// Rendering never writes here; the sim never reads rendering state. Fields are
/// one `Vec` per tile attribute, row-major: index = y * GRID_WIDTH + x
/// (design, Architecture -> WorldState).
pub struct World {
    /// World seed. Same seed + same actions => same history (design, Determinism).
    seed: u64,
    /// Sim-year counter. One tick is one sim-year (D24).
    tick: u64,
    /// Bumped by every tick, applied action and load (R6: repaint when dirty).
    revision: u64,
    /// Normalized height in [0, 1); at or below [`worldgen::SEA_LEVEL`] is ocean.
    pub elevation: Vec<f32>,
    /// Normalized moisture in [0, 1).
    pub moisture: Vec<f32>,
    /// Normalized temperature in [0, 1], latitude-driven minus an elevation lapse.
    pub temperature: Vec<f32>,
    /// Generated biome per tile.
    pub biome: Vec<Biome>,
    /// People the tile can carry, corrected for latitude.
    pub carrying_capacity: Vec<f32>,
    /// People living on the tile. A tile below 1 person carries no culture.
    pub population: Vec<f32>,
    /// Culture per tile, [`NO_CULTURE`] where nobody lives.
    pub culture_id: Vec<u16>,
    /// Flat per-tile culture vectors, `culture::DIMS` per tile.
    pub culture_vec: Vec<f32>,
    /// Every people that exists, indexed by id.
    pub(crate) cultures: Vec<Culture>,
    /// Split hysteresis: consecutive checks each culture had a qualifying
    /// candidate. A people that keeps producing candidates splits.
    pub(crate) split_watch: Vec<u8>,
    /// Tile humanity started on (design, Eden).
    pub(crate) eden: Option<u32>,
    /// Demography's own RNG stream, derived from the world seed (design:
    /// every system gets its own seeded RNG).
    pub(crate) rng: Pcg64,
}

impl World {
    /// Generates a new world from `seed`, then plants Eden.
    pub fn new(seed: u64) -> Self {
        let GeneratedWorld {
            elevation,
            moisture,
            temperature,
            biome,
            carrying_capacity,
        } = worldgen::generate(seed);

        let tiles = GRID_WIDTH * GRID_HEIGHT;
        let mut world = Self {
            seed,
            tick: 0,
            revision: 1,
            elevation,
            moisture,
            temperature,
            biome,
            carrying_capacity,
            population: vec![0.0; tiles],
            culture_id: vec![NO_CULTURE; tiles],
            culture_vec: vec![0.0; tiles * culture::DIMS],
            cultures: Vec::new(),
            split_watch: Vec::new(),
            eden: None,
            rng: Pcg64::seed_from_u64(seed ^ 0xD3A0_6D3D_0000_0001),
        };
        demography::plant_eden(&mut world);
        world
    }

    /// The world seed this history came from.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Current sim-year.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Revision counter; the UI repaints the map only when it changes (R6).
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Every people that exists, indexed by id.
    pub fn cultures(&self) -> &[Culture] {
        &self.cultures
    }

    /// Culture id of the people humanity started as.
    pub fn eden_culture_id(&self) -> u16 {
        match self.eden {
            Some(tile) => self.culture_id[tile as usize],
            None => NO_CULTURE,
        }
    }

    /// The tile Eden was planted on.
    pub fn eden_tile(&self) -> Option<usize> {
        self.eden.map(|tile| tile as usize)
    }

    /// Copy of one tile's culture vector.
    pub fn culture_vec_of(&self, tile: usize) -> [f32; culture::DIMS] {
        let mut out = [0.0; culture::DIMS];
        let start = tile * culture::DIMS;
        out.copy_from_slice(&self.culture_vec[start..start + culture::DIMS]);
        out
    }

    /// RGBA bytes for the map image, one 4-byte pixel per tile.
    pub fn paint_overlay(&self, mode: Overlay) -> Vec<u8> {
        match mode {
            Overlay::Biome => worldgen::overlay_biome(&self.biome),
            // Settled tiles take their culture's color, everything else stays a
            // desaturated biome shade (design, Map states).
            Overlay::Culture => {
                let mut bytes = Vec::with_capacity(self.biome.len() * 4);
                for i in 0..self.biome.len() {
                    let [r, g, b] = if self.culture_id[i] == NO_CULTURE {
                        culture::unsettled_color(self.biome[i])
                    } else {
                        culture::color(self.culture_id[i])
                    };
                    bytes.extend_from_slice(&[r, g, b, 255]);
                }
                bytes
            }
            // Traits, belief, polities and languages arrive in Milestones 4-5;
            // until then every mode paints the biome overlay.
            Overlay::Traits | Overlay::Belief | Overlay::Polity | Overlay::Language => {
                worldgen::overlay_biome(&self.biome)
            }
        }
    }

    /// Advances the simulation by `years` whole sim-years.
    ///
    /// Never returns mid-tick: player actions apply at boundaries only (D9), so
    /// replaying the same seed plus the same action log gives the same history.
    pub fn step(&mut self, years: u32) {
        for _ in 0..years {
            self.tick += 1;
            self.tick_systems();
            self.revision += 1;
        }
    }

    /// Runs every system once, in the fixed order.
    ///
    /// Systems arrive milestone by milestone (design, Milestones); the order is
    /// fixed from day one so replays stay comparable across milestones.
    fn tick_systems(&mut self) {
        // Climate (rare) -> Population -> Migration -> Drift/Blend -> Traits
        //   -> Belief -> Polity -> Tech -> War
        demography::tick_population(self);
        demography::tick_migration(self);
        demography::tick_drift(self);
        demography::sync_membership(self);

        // Aggregates refresh every 10 ticks; splits and schisms check every 25
        // (design, Simulation contracts).
        if self.tick.is_multiple_of(10) {
            culture::refresh_aggregates(self);
        }
        if self.tick.is_multiple_of(25) {
            culture::tick_splits(self);
            culture::refresh_aggregates(self);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_advances_one_sim_year_per_tick() {
        let mut world = World::new(42);
        assert_eq!(world.tick(), 0);

        let before = world.revision();
        world.step(10);

        assert_eq!(world.tick(), 10, "one tick is one sim-year (D24)");
        assert_eq!(
            world.revision(),
            before + 10,
            "revision bumps once per tick (R6)"
        );
    }

    #[test]
    fn step_zero_years_changes_nothing() {
        let mut world = World::new(7);
        world.step(0);
        assert_eq!(world.tick(), 0);
        assert_eq!(world.revision(), 1);
    }

    #[test]
    fn same_seed_starts_same_history() {
        assert_eq!(World::new(99).seed(), World::new(99).seed());
    }

    #[test]
    fn a_world_has_one_entry_per_tile_per_field() {
        let world = World::new(3);
        let tiles = GRID_WIDTH * GRID_HEIGHT;
        assert_eq!(world.elevation.len(), tiles);
        assert_eq!(world.moisture.len(), tiles);
        assert_eq!(world.temperature.len(), tiles);
        assert_eq!(world.biome.len(), tiles);
        assert_eq!(world.carrying_capacity.len(), tiles);
    }

    #[test]
    fn paint_overlay_is_rgba_for_every_tile() {
        let world = World::new(3);
        for mode in [
            Overlay::Biome,
            Overlay::Culture,
            Overlay::Traits,
            Overlay::Belief,
            Overlay::Polity,
            Overlay::Language,
        ] {
            assert_eq!(
                world.paint_overlay(mode).len(),
                GRID_WIDTH * GRID_HEIGHT * 4
            );
        }
    }
}
