//! Demography: Eden, population growth, crowding-driven migration, and
//! culture drift/blend (design, Milestones -> 1. Living Map).
//!
//! Tick order is fixed (design, Architecture): Population, Migration,
//! Drift/Blend. Growth is logistic toward the tile's latitude-corrected
//! carrying capacity; migration spills the crowded excess one step out, or two
//! steps when the one-step frontier is full, which is what lets a people reach
//! a new region across an empty desert or mountain belt and become separate.

use rand::Rng;

use crate::culture::{self, NO_CULTURE};
use crate::world::{GRID_HEIGHT, GRID_WIDTH, World};
use crate::worldgen::{self, Biome};

/// People at Eden when the world starts.
pub const EDEN_POPULATION: f32 = 8.0;

/// Logistic growth per sim-year.
const GROWTH_RATE: f32 = 0.12;

/// A tile above this share of its carrying capacity spills people outward.
const CROWDED: f32 = 0.55;

/// Share of the crowding excess that leaves each year.
const MIGRATION_RATE: f32 = 0.25;

/// How far a migration wave may travel when the near frontier is full.
const WAVE_RADIUS: i32 = 5;

/// Annual random mutation per axis, scaled by isolation: a people with no
/// neighbors drifts from its kin; a crowded frontier keeps blending back.
const MUTATION_RATE: f32 = 0.004;

/// Least number of people a migration puts on a newly settled tile: below one
/// person a tile carries no culture (design, Membership).
const MIN_FOUNDING_BAND: f32 = 1.0;

/// Founder effect: each newly settled tile is a slightly mutated copy of its
/// source people. Serial settlement therefore drifts geographically, which is
/// how real populations diverged as they spread.
const FOUNDER_MUTATION: f32 = 0.05;

/// Annual pull of a tile's culture toward a neighboring people's vector.
const BLEND_RATE: f32 = 0.020;

/// Same-culture neighbors diffuse toward each other so a region stays one
/// people instead of a salt-and-pepper field of vectors.
const COHESION_RATE: f32 = 0.0;

/// Spawns Eden: one people on the best land the world offers.
///
/// The seed decides the world, and the world decides where humanity starts, so
/// Eden needs no RNG of its own. Ties break on the lower tile index.
pub fn plant_eden(world: &mut World) {
    let mut eden = None;
    let mut best = -1.0_f32;
    for i in 0..world.biome.len() {
        if !worldgen::settleable(world.biome[i]) {
            continue;
        }
        let capacity = world.carrying_capacity[i];
        if capacity > best {
            best = capacity;
            eden = Some(i);
        }
    }
    let Some(eden) = eden else { return };

    let centroid = culture::biome_target(world.biome[eden]);
    world.population[eden] = EDEN_POPULATION;
    world.culture_id[eden] = 0;
    for (k, value) in centroid.iter().enumerate() {
        world.culture_vec[eden * culture::DIMS + k] = *value;
    }
    world.eden = Some(eden as u32);
    let mut culture = culture::Culture::found(
        0,
        0,
        world.seed(),
        centroid,
        culture::Favor::Chosen,
        EDEN_POPULATION,
        1,
    );
    let place = crate::names::place_name(&culture, eden);
    culture.scripture.push(crate::scripture::genesis(
        &culture.name,
        &place,
        eden as u32,
    ));
    world.languages.push(crate::language::Language::root(
        culture.id,
        culture.name_seed,
        culture.phonemes.clone(),
    ));
    world.cultures.push(culture);
    world.split_watch.push(0);
}

/// Logistic growth toward carrying capacity. Never overshoots.
pub fn tick_population(world: &mut World) {
    for i in 0..world.population.len() {
        if !worldgen::settleable(world.biome[i]) {
            world.population[i] = 0.0;
            continue;
        }
        let capacity = world.carrying_capacity[i];
        if capacity <= 0.0 {
            continue;
        }
        let population = world.population[i];
        if population <= 0.0 {
            continue;
        }
        let growth = GROWTH_RATE * population * (1.0 - population / capacity);
        // Adapted bodies earn a small edge in the country that shaped them
        // (design, Physical adaptation: small regional perks, never a ranking).
        let grown = population + crate::traits::perk(world, i, growth);
        world.population[i] = grown.clamp(0.0, capacity);
    }
}

/// Crowding-driven migration outward, one wave per crowded tile per year.
pub fn tick_migration(world: &mut World) {
    for i in 0..world.population.len() {
        if !worldgen::settleable(world.biome[i]) {
            continue;
        }
        let capacity = world.carrying_capacity[i];
        let population = world.population[i];
        if capacity <= 0.0 || population < CROWDED * capacity {
            continue;
        }

        let movers = (population - CROWDED * capacity) * MIGRATION_RATE;
        if movers <= 0.0 {
            continue;
        }
        let Some(target) = migration_target(world, i) else {
            continue;
        };

        // Only move what the destination can actually hold: a tile never
        // overshoots its carrying capacity (logistic growth and migration
        // agree on the ceiling).
        let room = (world.carrying_capacity[target] - world.population[target]).max(0.0);
        // Migration never depopulates its source: a tile drained below one
        // person would lose its culture and grow back an orphan.
        let available = (population - 1.0).max(0.0);
        let settling = world.culture_id[target] == NO_CULTURE;
        let mut moved = movers.min(room).min(available);
        if settling {
            // A founding band of at least one person, so the new tile carries a
            // culture from the start (design, Membership).
            moved = moved.max(MIN_FOUNDING_BAND).min(room).min(available);
        }
        if moved <= 0.0 {
            continue;
        }

        world.population[i] -= moved;
        world.population[target] += moved;

        // A tile newly settled by migration takes the source tile's culture and
        // culture vector (design, Membership), plus a founder mutation: the
        // further a people spreads, the more its distant settlements differ
        // from the homeland.
        if world.culture_id[target] == NO_CULTURE {
            world.culture_id[target] = world.culture_id[i];
            let src = i * culture::DIMS;
            let dst = target * culture::DIMS;
            world.culture_vec.copy_within(src..src + culture::DIMS, dst);
            // Settlers carry their religion with them: a new tile inherits the
            // source's belief, not a default.
            let belief_src = i * crate::belief::AXES;
            let belief_dst = target * crate::belief::AXES;
            world
                .belief
                .copy_within(belief_src..belief_src + crate::belief::AXES, belief_dst);
            // Settlers carry their bodies and their stories alike.
            let trait_src = i * crate::traits::TRAITS;
            let trait_dst = target * crate::traits::TRAITS;
            world
                .traits
                .copy_within(trait_src..trait_src + crate::traits::TRAITS, trait_dst);
            for k in 0..culture::DIMS {
                let shift = (world.rng.random::<f32>() - 0.5) * 2.0 * FOUNDER_MUTATION;
                world.culture_vec[dst + k] = (world.culture_vec[dst + k] + shift).clamp(0.0, 1.0);
            }
        }
    }
}

/// Best place for one tile's excess people: closest roomy land, cheapest step.
///
/// The near ring is tried first. Only if every near tile is full does the wave
/// reach [`WAVE_RADIUS`], which is how peoples cross empty belts and end up as
/// separate components (design: empty deserts and mountains separate peoples).
fn migration_target(world: &World, from: usize) -> Option<usize> {
    let (x, y) = (from % GRID_WIDTH, from / GRID_WIDTH);
    for radius in 1..=WAVE_RADIUS {
        let mut best: Option<(f32, usize)> = None;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs() != radius && dy.abs() != radius {
                    continue;
                }
                let ny = y as i32 + dy;
                if ny < 0 || ny >= GRID_HEIGHT as i32 {
                    continue;
                }
                let nx = (x as i32 + dx).rem_euclid(GRID_WIDTH as i32);
                let n = ny as usize * GRID_WIDTH + nx as usize;
                let room = room_at(world, n);
                if room <= 0.0 {
                    continue;
                }
                let lat = worldgen::latitude_deg(ny as usize);
                let cost = culture::step_cost(world.biome[n], lat, dx != 0);
                if !cost.is_finite() {
                    continue;
                }
                let score = room / cost;
                if best.is_none_or(|(best_score, _)| score > best_score) {
                    best = Some((score, n));
                }
            }
        }
        if let Some((_, target)) = best {
            return Some(target);
        }
    }
    None
}

/// Room a tile has for new people, or 0 if it can't be settled at all.
fn room_at(world: &World, i: usize) -> f32 {
    if !worldgen::settleable(world.biome[i]) {
        return 0.0;
    }
    (world.carrying_capacity[i] - world.population[i]).max(0.0)
}

/// Culture drift toward a region's target, mutation in isolation, and blending
/// with neighbors.
///
/// Three forces, all per tile (design, Simulation contracts -> drift/blend):
/// - mutation: isolation lets a people drift from its kin;
/// - diffusion: neighbors pull each other together, strongly within a people
///   (cohesion) and weakly across peoples (blend).
///
/// The vector is a set of neutral cultural markers, so divergence comes from
/// history, distance and isolation -- not a biome attractor. Physical
/// adaptation to climate is the trait system's job (Milestone 4).
pub fn tick_drift(world: &mut World) {
    let previous = world.culture_vec.clone();
    for i in 0..world.population.len() {
        if world.culture_id[i] == NO_CULTURE || world.population[i] < 1.0 {
            continue;
        }
        let (x, y) = (i % GRID_WIDTH, i / GRID_WIDTH);
        let me = world.culture_id[i];

        let mut own = [(0.0_f32, 0_usize); 8];
        let mut own_len = 0;
        let mut others = [(0.0_f32, 0_usize); 8];
        let mut others_len = 0;
        for (nx, ny) in culture::neighbors(x, y) {
            let n = ny * GRID_WIDTH + nx;
            if world.culture_id[n] == NO_CULTURE {
                continue;
            }
            if world.culture_id[n] == me {
                own[own_len] = (world.population[n].max(1.0), n);
                own_len += 1;
            } else {
                others[others_len] = (world.population[n].max(1.0), n);
                others_len += 1;
            }
        }

        // Isolation scales drift: peoples with no neighbors adapt to their own
        // country faster (design, language isolation, applied to culture).
        let isolation = 1.0 - (own_len + others_len) as f32 / 8.0;
        let mutation: [f32; culture::DIMS] =
            std::array::from_fn(|_| (world.rng.random::<f32>() - 0.5) * 2.0 * MUTATION_RATE);

        for k in 0..culture::DIMS {
            let mut value = previous[i * culture::DIMS + k];
            value += mutation[k] * (0.5 + isolation);
            for &(weight, n) in &own[..own_len] {
                let theirs = previous[n * culture::DIMS + k];
                value += (theirs - value) * COHESION_RATE * weight / 8.0;
            }
            for &(weight, n) in &others[..others_len] {
                let theirs = previous[n * culture::DIMS + k];
                value += (theirs - value) * BLEND_RATE * weight / 8.0;
            }
            world.culture_vec[i * culture::DIMS + k] = value.clamp(0.0, 1.0);
        }
    }
}

/// Keeps tile membership consistent with population (design, Membership):
/// below one person a tile carries no culture; a tile that grows back over one
/// re-adopts the culture of its most populous settled neighbor.
pub fn sync_membership(world: &mut World) {
    for i in 0..world.population.len() {
        if world.population[i] < 1.0 {
            world.culture_id[i] = NO_CULTURE;
            continue;
        }
        if world.culture_id[i] != NO_CULTURE {
            continue;
        }
        let (x, y) = (i % GRID_WIDTH, i / GRID_WIDTH);
        let mut best: Option<(f32, u16)> = None;
        for (nx, ny) in culture::neighbors(x, y) {
            let n = ny * GRID_WIDTH + nx;
            if world.culture_id[n] == NO_CULTURE || world.population[n] < 1.0 {
                continue;
            }
            if best.is_none_or(|(best_pop, _)| world.population[n] > best_pop) {
                best = Some((world.population[n], world.culture_id[n]));
            }
        }
        if let Some((_, culture_id)) = best {
            world.culture_id[i] = culture_id;
        }
    }
}

/// Whatever the tile carries, for diagnostics and tuning runs.
pub fn tile_biome(world: &World, i: usize) -> Biome {
    world.biome[i]
}
