//! Geology during play (design, Out of the Garden; D27).
//!
//! The plates that broke the supercontinent keep drifting through the 5,000
//! years: rifts widen into seas, land slams into land and folds into ranges
//! or closes a sea into a land bridge, and hotspots keep raising islands.
//! Every [`GEOLOGY_EVERY`] years the land is regenerated for the new year from
//! the seed and the change is applied tile by tile. Land that sinks or is
//! pushed up past living on drives its people out, into the rest of their
//! land or away as refugee bands; new land is open to bands. Peoples parted by a new sea drift apart through the
//! existing split rules.

use crate::bands;
use crate::culture::NO_CULTURE;
use crate::world::{GRID_HEIGHT, GRID_WIDTH, World};
use crate::worldgen::{self, Biome};

/// Years between geology passes.
pub const GEOLOGY_EVERY: u64 = 50;

/// Least change in a tile's ground that is applied; smaller drift waits, so
/// the map is not rebuilt for motion nobody could see.
const GROUND_STEP: f32 = 0.01;

/// Rows of the next pass's crust computed each year in between, so no one
/// year carries the whole regeneration (budget: 8 ms a tick).
const ROWS_PER_YEAR: usize = GRID_HEIGHT.div_ceil(GEOLOGY_EVERY as usize - 1);

/// The next pass's crust, built a band of rows each year. Not saved: it is a
/// pure function of the seed and the year, so a loaded world rebuilds what it
/// lacks at the pass and reaches the same land.
#[derive(Clone, Default)]
pub(crate) struct Pending {
    year: u64,
    rows_done: usize,
    raw: Vec<f32>,
}

/// Builds the coming pass's crust a few rows a year, and every
/// [`GEOLOGY_EVERY`] years moves the land on to the year.
pub fn tick_geology(world: &mut World) {
    let tick = world.tick();
    let seed = world.seed();
    let pass = tick.next_multiple_of(GEOLOGY_EVERY);
    let pending = &mut world.pending_ground;
    if pending.year != pass || pending.raw.len() != GRID_WIDTH * GRID_HEIGHT {
        *pending = Pending {
            year: pass,
            rows_done: 0,
            raw: vec![0.0; GRID_WIDTH * GRID_HEIGHT],
        };
    }
    let upto = if tick == pass {
        GRID_HEIGHT
    } else {
        (pending.rows_done + ROWS_PER_YEAR).min(GRID_HEIGHT)
    };
    worldgen::crust_rows(seed, pass, pending.rows_done..upto, &mut pending.raw);
    pending.rows_done = upto;
    if tick != pass {
        return;
    }
    let raw = std::mem::take(&mut world.pending_ground.raw);
    let target = worldgen::finish_ground(seed, pass, raw, world.history());
    let mut lost = Vec::new();
    let mut moved = false;
    for (tile, &ground) in target.iter().enumerate() {
        let delta = ground - world.ground[tile];
        if delta.abs() < GROUND_STEP {
            continue;
        }
        let was_home = worldgen::settleable(world.biome[tile]);
        world.ground[tile] = ground;
        // As a shift, so a power's raise or lower on this tile stays.
        world.shift_elevation(tile, delta);
        moved = true;
        if was_home && !worldgen::settleable(world.biome[tile]) {
            lost.push(tile);
        }
    }
    if !moved {
        return;
    }
    world.ground_revision += 1;
    for tile in lost {
        displace(world, tile);
    }
    waves(world, pass);
    bands::replan_after_geology(world);
}

/// Share of the people on a coast a volcano's wave takes.
const WAVE_TOLL: f32 = 0.4;

/// A volcano's collapse sends a wave: every lived-on coast within its reach
/// loses [`WAVE_TOLL`] of its people (D28, the island that blew apart).
fn waves(world: &mut World, year: u64) {
    let due: Vec<_> = world
        .history()
        .waves
        .iter()
        .filter(|w| w.year == year)
        .copied()
        .collect();
    for wave in due {
        let (cx, cy) = (
            wave.center[0].round() as usize % GRID_WIDTH,
            wave.center[1].round().clamp(0.0, GRID_HEIGHT as f32 - 1.0) as usize,
        );
        for (x, y) in crate::hex::disc(cx, cy, wave.reach as u32) {
            let tile = y * GRID_WIDTH + x;
            let shore = crate::hex::neighbors(x, y)
                .any(|(nx, ny)| world.biome[ny * GRID_WIDTH + nx] == Biome::Ocean);
            if shore && world.biome[tile] != Biome::Ocean {
                world.population[tile] *= 1.0 - WAVE_TOLL;
            }
        }
    }
}

/// A tile can no longer be lived on: its people move into the rest of their
/// settlement's land as far as there is room, and the rest leave as a refugee
/// band if they can found a camp and a walking slot is free. A tile the sea
/// took leaves its catchment; a seat that cannot be lived on moves to the
/// settlement's most populous land left.
pub(crate) fn displace(world: &mut World, tile: usize) {
    let people = world.population[tile];
    let culture = world.culture_id[tile];
    world.population[tile] = 0.0;
    world.culture_id[tile] = NO_CULTURE;
    let owner = world.settlement_of[tile];
    if owner == u32::MAX {
        return;
    }
    let settlement = &mut world.settlements[owner as usize];
    if world.biome[tile] == Biome::Ocean {
        world.settlement_of[tile] = u32::MAX;
        settlement.catchment.retain(|&t| t != tile);
    }
    if settlement.tile == tile
        && let Some(&seat) = settlement
            .catchment
            .iter()
            .filter(|&&t| worldgen::settleable(world.biome[t]))
            .max_by(|&&a, &&b| {
                world.population[a]
                    .total_cmp(&world.population[b])
                    .then(b.cmp(&a))
            })
    {
        settlement.tile = seat;
    }
    if people < 1.0 || culture == NO_CULTURE {
        return;
    }
    let mut left = people;
    for t in settlement.catchment.clone() {
        if world.culture_id[t] != culture {
            continue;
        }
        let room = (world.carrying_capacity[t] - world.population[t]).max(0.0);
        let put = left.min(room);
        world.population[t] += put;
        left -= put;
    }
    if left >= bands::SETTLE_MIN {
        bands::refugees(world, tile, culture, left);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::powers::Power;
    use crate::world::GRID_WIDTH;

    /// A power's raise stays on its tile while the land moves under it.
    #[test]
    fn a_raised_tile_keeps_its_raise_as_the_land_moves() {
        let mut world = World::new(3);
        let tile = (0..world.biome.len())
            .find(|&t| world.biome[t] == Biome::Grassland)
            .expect("grassland");
        world.queue_power(Power::Raise, tile % GRID_WIDTH, tile / GRID_WIDTH);
        world.step(1);
        let raised = world.elevation[tile] - world.ground[tile];
        world.step(2_000);
        let kept = world.elevation[tile] - world.ground[tile];
        assert!(raised > 0.04, "the raise landed ({raised})");
        assert!(
            (kept - raised).abs() < 1e-4
                || world.elevation[tile] >= 1.0
                || world.elevation[tile] <= 0.0,
            "the raise survives 2,000 years of drift: {raised} -> {kept}"
        );
    }

    /// People on a tile the sea takes go to the rest of their land, or walk.
    #[test]
    fn people_driven_off_sinking_land_are_not_lost_while_there_is_room() {
        let mut world = World::new(1);
        world.step(300);
        let (tile, owner) = (0..world.population.len())
            .filter(|&t| world.population[t] >= 5.0 && world.settlement_of[t] != u32::MAX)
            .map(|t| (t, world.settlement_of[t] as usize))
            .next()
            .expect("a settled tile");
        let land = world.settlements[owner].catchment.clone();
        for &t in &land {
            if t != tile {
                world.population[t] *= 0.5;
            }
        }
        let people = world.population[tile];
        let before: f32 = land.iter().map(|&t| world.population[t]).sum();
        let walking: f32 = world.bands.iter().map(|b| b.headcount).sum();
        displace(&mut world, tile);
        let after: f32 = world.settlements[owner]
            .catchment
            .iter()
            .map(|&t| world.population[t])
            .sum();
        let walking_now: f32 = world.bands.iter().map(|b| b.headcount).sum();
        // The test drowns the tile, as the sea would have.
        assert_eq!(
            world.settlements[owner].catchment.contains(&tile),
            world.biome[tile] != Biome::Ocean
        );
        assert!(
            (after + walking_now - walking - before).abs() < 0.01,
            "{people} people leave the tile and none are lost: {before} -> {after} + {} walking",
            walking_now - walking
        );
    }
}
