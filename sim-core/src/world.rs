//! World state and the fixed tick pipeline.
//!
//! One tick is one sim-year in every era (D24). Systems run in a fixed order:
//! Climate, Population, Migration, Drift/Blend, Traits, Belief, Polity, Tech,
//! War (design, Architecture). Each system is a single `tick(world, rng)`
//! function so it can be tested on its own.

use rand::SeedableRng;
use rand_pcg::Pcg64;

use crate::belief::{self, Belief};
use crate::culture::{self, Culture, NO_CULTURE};
use crate::demography;
use crate::faith::{self, Faith};
pub use crate::hex;
use crate::language;
use crate::language::Language;
use crate::names;
use crate::notables::{self, Budget, Legend, Notable, Role};
use crate::powers::{self, Action, ActionKind, PendingOutcome, Power};
use crate::scripture::Verse;
use crate::settlements;
use crate::settlements::Settlement;
use crate::traits;
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
/// one `Vec` per tile attribute, row-major: index = y * `GRID_WIDTH` + x
/// (design, Architecture -> `WorldState`).
#[derive(Clone)]
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
    /// Untouched carrying capacity, before blessings multiply it.
    pub base_capacity: Vec<f32>,
    /// Tick a blessing on this tile expires; 0 means unblessed.
    pub blessed_until: Vec<u64>,
    /// Population ten ticks ago, for the witness rule's expected trend.
    pub(crate) population_10_ago: Vec<f32>,
    /// Belief per tile, six axes each (design, Belief model).
    pub belief: Vec<f32>,
    /// People traits per tile: skin, stature, cold, altitude
    /// (design, Physical adaptation).
    pub traits: Vec<f32>,
    /// Witness windows still open.
    pub(crate) pending: Vec<PendingOutcome>,
    /// Queued player actions, in arrival order (design, Player actions).
    pub(crate) queue: Vec<Action>,
    /// The action log: every action that ever applied, for replay.
    actions: Vec<Action>,
    /// Living individuals.
    pub(crate) notables: Vec<Notable>,
    /// Records of individuals who are gone but still cited.
    pub(crate) legends: Vec<Legend>,
    /// Next individual id.
    pub(crate) next_notable_id: u32,
    /// Beasts wandering the map.
    pub(crate) beasts: Vec<Beast>,
    /// Every religion, indexed by faith id.
    pub(crate) faiths: Vec<Faith>,
    /// Every tongue, indexed by language id.
    pub(crate) languages: Vec<Language>,
    /// Every settlement, indexed by settlement id.
    pub(crate) settlements: Vec<Settlement>,
    /// Which settlement claims each tile, or `u32::MAX` if none.
    pub(crate) settlement_of: Vec<u32>,
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
        let base_capacity = carrying_capacity.clone();

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
            base_capacity,
            blessed_until: vec![0; tiles],
            population_10_ago: vec![0.0; tiles],
            belief: tiles_flat_belief(tiles),
            traits: vec![traits::START; tiles * traits::TRAITS],
            pending: Vec::new(),
            queue: Vec::new(),
            actions: Vec::new(),
            notables: Vec::new(),
            legends: Vec::new(),
            next_notable_id: 1,
            beasts: Vec::new(),
            faiths: Vec::new(),
            languages: Vec::new(),
            settlements: Vec::new(),
            settlement_of: vec![u32::MAX; GRID_WIDTH * GRID_HEIGHT],
        };
        demography::plant_eden(&mut world);
        world.faiths.push(Faith {
            id: 0,
            founded_tick: 0,
            founding_belief: Belief::START,
        });
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
    #[expect(
        clippy::match_same_arms,
        reason = "Polity and Language keep their own arm until their milestones paint them"
    )]
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
            // What each tile's people believe about the god.
            Overlay::Belief => {
                let mut bytes = Vec::with_capacity(self.biome.len() * 4);
                for tile in 0..self.biome.len() {
                    let [r, g, b] = self.belief_of(tile).color();
                    bytes.extend_from_slice(&[r, g, b, 255]);
                }
                bytes
            }
            // What each tile's people look like: skin tone, cooled toward
            // slate with altitude (thin air, hard country).
            Overlay::Traits => {
                let mut bytes = Vec::with_capacity(self.biome.len() * 4);
                for tile in 0..self.biome.len() {
                    let start = tile * traits::TRAITS;
                    let mut rgb = traits::skin_color(self.traits[start + traits::SKIN]);
                    if self.population[tile] < 1.0 {
                        rgb = culture::unsettled_color(self.biome[tile]);
                    } else {
                        let altitude = self.traits[start + traits::ALTITUDE];
                        for (mix, slate) in rgb.iter_mut().zip([104_u8, 116, 128]) {
                            *mix = (f32::from(*mix) * (1.0 - altitude * 0.35)
                                + f32::from(slate) * altitude * 0.35)
                                as u8;
                        }
                    }
                    bytes.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
                }
                bytes
            }
            // Peoples who share a tongue paint as one hue, offset from the
            // culture palette so the family tree reads on the map (design,
            // Tongues).
            Overlay::Language => {
                let mut bytes = Vec::with_capacity(self.biome.len() * 4);
                for i in 0..self.biome.len() {
                    let tongue = if self.culture_id[i] == NO_CULTURE {
                        u16::MAX
                    } else {
                        self.cultures[self.culture_id[i] as usize].language_id
                    };
                    let [r, g, b] = if tongue == u16::MAX {
                        culture::unsettled_color(self.biome[i])
                    } else {
                        culture::color(tongue.wrapping_add(32))
                    };
                    bytes.extend_from_slice(&[r, g, b, 255]);
                }
                bytes
            }
            // Polity assembles in a later milestone; until then it paints the
            // biome overlay.
            Overlay::Polity => worldgen::overlay_biome(&self.biome),
        }
    }

    /// Queues a power at `(x, y)`; it applies at the start of the next tick
    /// (design, Player actions are queued).
    pub fn queue_power(&mut self, power: Power, x: usize, y: usize) {
        self.queue.push(Action {
            tick: self.tick + 1,
            kind: ActionKind::Power {
                power,
                x: x % GRID_WIDTH,
                y: y.min(GRID_HEIGHT - 1),
            },
        });
    }

    /// Queues an anointing of another people (design D11).
    pub fn queue_anoint(&mut self, culture_id: u16) {
        self.queue.push(Action {
            tick: self.tick + 1,
            kind: ActionKind::Anoint { culture_id },
        });
    }

    /// Every religion that exists, indexed by faith id.
    pub fn faiths(&self) -> &[Faith] {
        &self.faiths
    }

    /// Every tongue, indexed by language id.
    pub fn languages(&self) -> &[Language] {
        &self.languages
    }

    /// Every settlement, indexed by settlement id.
    pub fn settlements(&self) -> &[Settlement] {
        &self.settlements
    }

    /// Every action that has applied, in order: the replay log.
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    /// Living individuals.
    pub fn notables(&self) -> &[Notable] {
        &self.notables
    }

    /// Living witnesses.
    pub fn witnesses(&self) -> Vec<Notable> {
        self.notables
            .iter()
            .filter(|notable| notable.role == Role::Witness)
            .cloned()
            .collect()
    }

    /// Records of the dead who are still cited.
    pub fn legends(&self) -> &[Legend] {
        &self.legends
    }

    /// Whether a people is the god's own.
    pub fn is_chosen(&self, culture_id: u16) -> bool {
        self.cultures
            .get(culture_id as usize)
            .is_some_and(|culture| culture.favor == culture::Favor::Chosen)
    }

    /// This tile's people's belief.
    pub fn belief_of(&self, tile: usize) -> Belief {
        let start = tile * belief::AXES;
        Belief::from_axes(&self.belief[start..start + belief::AXES])
    }

    /// Writes a tile's belief back.
    pub fn set_belief(&mut self, tile: usize, value: Belief) {
        let start = tile * belief::AXES;
        self.belief[start..start + belief::AXES].copy_from_slice(&value.to_axes());
    }

    /// Writes one trait axis on a tile.
    pub fn set_trait(&mut self, tile: usize, axis: usize, value: f32) {
        self.traits[tile * traits::TRAITS + axis] = value.clamp(0.0, 1.0);
    }

    /// The Δpopulation% this tile's own 10-tick trend predicts for a 5-tick
    /// window (design, Witness rule).
    pub fn expected_growth(&self, tile: usize) -> f32 {
        let then = self.population_10_ago[tile];
        if then <= 0.0 {
            return 0.0;
        }
        (self.population[tile] / then - 1.0) * 0.5
    }

    /// Promotes the person on `tile` to witness, returning their id.
    ///
    /// Enforces the witness and culture budgets: the lowest-salience member of
    /// a full budget becomes a legend (design, Individual budgets).
    pub fn promote_witness(&mut self, tile: usize) -> Option<u32> {
        let culture_id = self.culture_id[tile];
        let culture = self.cultures.get(culture_id as usize)?;
        let name = names::person_name(culture, self.next_notable_id);
        let role = Role::Witness;

        let budget = if culture.favor == culture::Favor::Chosen {
            Budget::Chosen
        } else {
            Budget::Other
        };
        self.evict_for(budget, culture_id);

        let id = self.next_notable_id;
        self.next_notable_id += 1;
        let start = tile * traits::TRAITS;
        let (skin, stature) = (
            self.traits[start + traits::SKIN],
            self.traits[start + traits::STATURE],
        );
        self.notables.push(Notable {
            id,
            culture_id,
            name,
            tile: tile as u32,
            born_tick: self.tick,
            role,
            salience: 1.0,
            skin,
            stature,
        });
        Some(id)
    }

    /// Writes a witness's verse into their people's scripture.
    pub fn write_verse(&mut self, power: Power, witness_id: u32) {
        let Some((culture_id, verse)) = powers::verse_for(self, power, witness_id) else {
            return;
        };
        if let Some(culture) = self.cultures.get_mut(culture_id as usize) {
            culture.scripture.push(verse);
        }
    }

    /// Writes a verse of some other kind for one witness.
    pub fn write_verse_kind(&mut self, kind: crate::scripture::VerseKind, witness_id: u32) {
        let Some(notable) = self.notables.iter().find(|n| n.id == witness_id) else {
            return;
        };
        let culture_id = notable.culture_id;
        let Some(culture) = self.cultures.get(culture_id as usize) else {
            return;
        };
        let verse = Verse {
            tick: self.tick(),
            kind,
            polarity: crate::scripture::Polarity::Silence,
            witness: notable.name.clone(),
            place: names::place_name(culture, notable.tile as usize),
            place_tile: notable.tile,
            people: culture.name.clone(),
            other: String::new(),
            witness_id: Some(witness_id),
            mutation: 0,
        };
        self.cultures[culture_id as usize].scripture.push(verse);
    }

    /// A people's scripture, newest first, already rendered (design: `GDScript`
    /// only displays text Rust hands it).
    pub fn scripture_text(&self, culture_id: u16, from: usize, count: usize) -> Vec<String> {
        let Some(culture) = self.cultures.get(culture_id as usize) else {
            return Vec::new();
        };
        culture
            .scripture
            .iter()
            .rev()
            .skip(from)
            .take(count)
            .map(|verse| {
                format!(
                    "{}\n{}",
                    language::native_line(self, culture_id, verse),
                    verse.render()
                )
            })
            .collect()
    }

    /// Lifts or sinks land, then reclassifies it.
    pub fn shift_elevation(&mut self, tile: usize, delta: f32) {
        self.elevation[tile] = (self.elevation[tile] + delta).clamp(0.0, 1.0);
        self.recompute_tile(tile);
    }

    /// Replaces a tile's biome and its carrying capacity.
    pub fn set_biome(&mut self, tile: usize, biome: Biome) {
        let lat = worldgen::latitude_deg(tile / GRID_WIDTH);
        self.biome[tile] = biome;
        self.base_capacity[tile] = worldgen::capacity_for(biome, lat);
        self.refresh_capacity(tile);
    }

    /// Blesses a tile: +25% carrying capacity for ten years.
    pub fn bless(&mut self, tile: usize) {
        self.blessed_until[tile] = self.tick + 10;
        self.refresh_capacity(tile);
    }

    /// A meteor: a crater, most of the people dead, the land poorer.
    pub fn meteor(&mut self, tile: usize) {
        self.population[tile] *= 0.15;
        self.elevation[tile] = (self.elevation[tile] - 0.10).max(0.0);
        self.recompute_tile(tile);
        self.base_capacity[tile] *= 0.70;
        self.refresh_capacity(tile);
    }

    /// Sets beasts loose on a tile for twenty years.
    pub fn spawn_beasts(&mut self, tile: usize) {
        self.beasts.push(Beast {
            tile,
            until: self.tick + 20,
        });
    }

    /// Reclassifies a tile from its elevation, moisture and temperature.
    pub(crate) fn recompute_tile(&mut self, tile: usize) {
        let lat = worldgen::latitude_deg(tile / GRID_WIDTH);
        let biome = if lat.abs() > worldgen::ICE_LATITUDE_DEG {
            Biome::Ice
        } else {
            worldgen::classify(
                self.elevation[tile],
                self.moisture[tile],
                self.temperature[tile],
            )
        };
        self.biome[tile] = biome;
        self.base_capacity[tile] = worldgen::capacity_for(biome, lat);
        self.refresh_capacity(tile);
    }

    /// Recomputes effective capacity from the base and any blessing.
    fn refresh_capacity(&mut self, tile: usize) {
        let multiplier = if self.blessed_until[tile] > self.tick {
            1.25
        } else {
            1.0
        };
        self.carrying_capacity[tile] = self.base_capacity[tile] * multiplier;
        if self.population[tile] > self.carrying_capacity[tile] {
            self.population[tile] = self.carrying_capacity[tile];
        }
    }

    /// Applies every action queued for this tick, in arrival order.
    fn apply_queued_actions(&mut self) {
        let due: Vec<Action> = self
            .queue
            .iter()
            .copied()
            .filter(|action| action.tick <= self.tick)
            .collect();
        self.queue.retain(|action| action.tick > self.tick);
        for action in due {
            match action.kind {
                ActionKind::Power { power, x, y } => powers::apply(self, power, x, y),
                ActionKind::Anoint { culture_id } => faith::anoint(self, culture_id),
            }
            self.actions.push(action);
        }
    }

    /// Beasts thin the local population until they move on.
    fn tick_beasts(&mut self) {
        for beast in &self.beasts {
            if beast.until > self.tick {
                self.population[beast.tile] *= 0.82;
            }
        }
        let tick = self.tick;
        self.beasts.retain(|beast| beast.until > tick);
    }

    /// Ends blessings whose ten years are up.
    fn tick_blessings(&mut self) {
        for tile in 0..self.blessed_until.len() {
            if self.blessed_until[tile] != 0 && self.tick >= self.blessed_until[tile] {
                self.blessed_until[tile] = 0;
                self.carrying_capacity[tile] = self.base_capacity[tile];
            }
        }
    }

    /// Demotes witnesses nobody elevated within their tenure (design).
    fn demote_old_witnesses(&mut self) {
        let tick = self.tick;
        let mut kept = Vec::with_capacity(self.notables.len());
        for notable in self.notables.drain(..) {
            if notable.role == Role::Witness && tick >= notable.born_tick + notables::WITNESS_TENURE
            {
                self.legends.push(Legend {
                    id: notable.id,
                    culture_id: notable.culture_id,
                    name: notable.name,
                    born_tick: notable.born_tick,
                    died_tick: tick,
                    role: notable.role,
                });
            } else {
                kept.push(notable);
            }
        }
        self.notables = kept;
    }

    /// Evicts the lowest-salience member of a full budget to legend.
    fn evict_for(&mut self, budget: Budget, culture_id: u16) {
        let mut members: Vec<usize> = self
            .notables
            .iter()
            .enumerate()
            .filter(|(_, notable)| match budget {
                Budget::Witness => true,
                Budget::Chosen | Budget::Other => notable.culture_id == culture_id,
            })
            .map(|(index, _)| index)
            .collect();
        let witnesses = self.witnesses().len();
        let full = match budget {
            Budget::Witness => witnesses >= notables::WITNESS_BUDGET,
            Budget::Chosen => members.len() >= notables::CHOSEN_BUDGET,
            Budget::Other => members.len() >= notables::OTHER_BUDGET,
        };
        if !full {
            return;
        }
        members.sort_by(|&a, &b| {
            self.notables[a]
                .salience
                .total_cmp(&self.notables[b].salience)
                .then(a.cmp(&b))
        });
        if let Some(&index) = members.first() {
            let evicted = self.notables.remove(index);
            self.legends.push(Legend {
                id: evicted.id,
                culture_id: evicted.culture_id,
                name: evicted.name,
                born_tick: evicted.born_tick,
                died_tick: self.tick,
                role: evicted.role,
            });
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
        // Player actions first, in arrival order, before Climate (design).
        self.apply_queued_actions();
        self.tick_beasts();
        self.tick_blessings();

        // Climate (rare) -> Population -> Migration -> Drift/Blend -> Traits
        //   -> Belief -> Polity -> Tech -> War
        demography::tick_population(self);
        demography::tick_migration(self);
        demography::tick_drift(self);
        demography::sync_membership(self);
        powers::close_windows(self);
        self.demote_old_witnesses();

        // Aggregates refresh every 10 ticks; splits and schisms check every 25
        // (design, Simulation contracts).
        if self.tick.is_multiple_of(10) {
            self.population_10_ago.copy_from_slice(&self.population);
            traits::tick_adaptation(self);
            faith::tick_testimony(self);
            culture::refresh_aggregates(self);
            faith::tick_doctrine_effects(self);
        }
        if self.tick.is_multiple_of(25) {
            culture::tick_splits(self);
            culture::refresh_aggregates(self);
            language::tick_languages(self);
            settlements::tick_settlements(self);
            faith::tick_doctrines(self);
            faith::tick_schisms(self);
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

/// A beast that hunts the people of one tile for a while.
#[derive(Copy, Clone, Debug)]
pub struct Beast {
    pub tile: usize,
    pub until: u64,
}

/// Belief field initialization: every tile starts at [`Belief::START`].
fn tiles_flat_belief(tiles: usize) -> Vec<f32> {
    let start = Belief::START.to_axes();
    let mut field = Vec::with_capacity(tiles * belief::AXES);
    for _ in 0..tiles {
        field.extend_from_slice(&start);
    }
    field
}
