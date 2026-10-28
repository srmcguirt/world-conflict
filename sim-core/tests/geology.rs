//! Geology during play (design, D27): the plates keep drifting through the
//! 5,000 years, so rifts widen, land meets land, and the sea takes coasts.

use sim_core::World;
use sim_core::geology::GEOLOGY_EVERY;
use sim_core::worldgen::{self, Biome, SEA_LEVEL};

const SEEDS: [u64; 4] = [1, 2, 3, 5];

/// Tiles that changed between land and sea from year 0 to `years`, as
/// (sea that became land, land that became sea).
fn flips(seed: u64, years: u64) -> (usize, usize) {
    let start = worldgen::elevation_at(seed, 0);
    let later = worldgen::elevation_at(seed, years);
    let lat = |tile: usize| worldgen::latitude_deg(tile / sim_core::world::GRID_WIDTH);
    let mut out = (0, 0);
    for tile in 0..start.len() {
        if lat(tile).abs() > worldgen::ICE_LATITUDE_DEG {
            continue;
        }
        match (start[tile] > SEA_LEVEL, later[tile] > SEA_LEVEL) {
            (false, true) => out.0 += 1,
            (true, false) => out.1 += 1,
            _ => {}
        }
    }
    out
}

#[test]
fn the_land_moves_gradually_through_play() {
    for seed in SEEDS {
        let (rose_1k, sank_1k) = flips(seed, 1_000);
        let (rose_5k, sank_5k) = flips(seed, 5_000);
        assert!(
            rose_5k > 0 && sank_5k > 0,
            "seed {seed}: land rises and sinks"
        );
        assert!(
            rose_5k + sank_5k > rose_1k + sank_1k,
            "seed {seed}: more has moved by year 5,000 ({}) than by 1,000 ({})",
            rose_5k + sank_5k,
            rose_1k + sank_1k
        );
        // A young sea, not a new world: most of the map stays as it was.
        let tiles = (sim_core::world::GRID_WIDTH * sim_core::world::GRID_HEIGHT) as f32;
        assert!(
            ((rose_5k + sank_5k) as f32) < 0.15 * tiles,
            "seed {seed}: {} tiles changed between land and sea",
            rose_5k + sank_5k
        );
    }
}

#[test]
fn year_zero_is_the_generated_world() {
    for seed in SEEDS {
        let world = World::new(seed);
        assert_eq!(world.elevation, worldgen::elevation_at(seed, 0));
    }
}

#[test]
fn the_sea_never_holds_people_and_the_map_hears_the_land_move() {
    for seed in SEEDS {
        let mut world = World::new(seed);
        for _ in 0..(3_000 / GEOLOGY_EVERY) {
            world.step(GEOLOGY_EVERY as u32);
            for tile in 0..world.population.len() {
                if world.biome[tile] == Biome::Ocean {
                    assert!(
                        world.population[tile] < 1.0,
                        "seed {seed} year {}: people on sea tile {tile}",
                        world.tick()
                    );
                }
            }
        }
        assert!(
            world.ground_revision() > 0,
            "seed {seed}: the ground moved in 3,000 years"
        );
    }
}
