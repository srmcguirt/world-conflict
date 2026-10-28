//! The real world's shaping events set symbolically on every generated world
//! (design, D28): placed around the cradle, and each during-play event
//! changes the land when it is due and not before.

use sim_core::history::{self, Plan};
use sim_core::world::{GRID_WIDTH, World};
use sim_core::worldgen::{self, SEA_LEVEL};
use sim_core::{hex, save};

const SEEDS: [u64; 4] = [1, 2, 3, 5];

/// Share of an event's tiles under the sea in `years`.
fn drowned(plan: &Plan, seed: u64, symbol: &str, years: u64) -> f32 {
    let tiles = plan.tiles_of(symbol);
    let land = worldgen::elevation_at(seed, years);
    let wet = tiles.iter().filter(|&&t| land[t] <= SEA_LEVEL).count();
    wet as f32 / tiles.len().max(1) as f32
}

#[test]
fn every_world_carries_the_core_of_the_catalog() {
    for seed in SEEDS {
        let plan = history::plan(seed);
        assert!(plan.cradle.is_some(), "seed {seed}: a cradle");
        for symbol in [
            "atlantic",
            "india_asia",
            "red_sea",
            "mediterranean",
            "east_african_rift",
            "persian_gulf",
            "black_sea",
            "thera",
        ] {
            assert!(plan.has(symbol), "seed {seed}: {symbol} is placed");
        }
        assert_eq!(
            plan,
            history::plan(seed),
            "seed {seed}: placed the same twice"
        );
    }
}

#[test]
fn the_garden_sits_by_the_cradle() {
    for seed in SEEDS {
        let mut world = World::new(seed);
        let cradle = world.history().cradle.expect("a cradle");
        let garden = world.garden_tile().expect("a Garden");
        let apart = hex::distance(
            cradle % GRID_WIDTH,
            cradle / GRID_WIDTH,
            garden % GRID_WIDTH,
            garden / GRID_WIDTH,
        );
        assert!(
            apart <= 4,
            "seed {seed}: the Garden is {apart} hexes from the cradle"
        );
    }
}

#[test]
fn the_fertile_lowland_drowns_in_its_centuries() {
    for seed in SEEDS {
        let plan = history::plan(seed);
        let before = drowned(&plan, seed, "persian_gulf", 150);
        let after = drowned(&plan, seed, "persian_gulf", 450);
        assert!(
            before < 0.3,
            "seed {seed}: the lowland is dry at year 150 ({before:.2})"
        );
        assert!(
            after > 0.7,
            "seed {seed}: the sea has it by year 450 ({after:.2})"
        );
    }
}

#[test]
fn the_dry_basin_floods_all_at_once_when_the_strait_breaches() {
    for seed in SEEDS {
        let plan = history::plan(seed);
        let before = drowned(&plan, seed, "black_sea", 1_450);
        let after = drowned(&plan, seed, "black_sea", 1_500);
        assert!(
            before < 0.3,
            "seed {seed}: the basin is dry before the breach ({before:.2})"
        );
        assert!(
            after > 0.7,
            "seed {seed}: the basin is sea after it ({after:.2})"
        );
    }
}

#[test]
fn the_volcanic_island_stands_until_it_blows_apart() {
    for seed in SEEDS {
        let plan = history::plan(seed);
        let before = drowned(&plan, seed, "thera_rises", 2_450);
        let after = drowned(&plan, seed, "thera_rises", 2_500);
        assert!(before < 0.5, "seed {seed}: the island stands ({before:.2})");
        assert!(
            after > 0.9,
            "seed {seed}: the sea fills its caldera ({after:.2})"
        );
    }
}

#[test]
fn the_rift_valley_deepens_through_play() {
    for seed in SEEDS {
        let plan = history::plan(seed);
        let tiles = plan.tiles_of("east_african_rift");
        let mean = |years: u64| {
            let land = worldgen::elevation_at(seed, years);
            tiles.iter().map(|&t| land[t]).sum::<f32>() / tiles.len() as f32
        };
        let (start, half, end) = (mean(0), mean(2_500), mean(5_000));
        assert!(
            start > half && half > end,
            "seed {seed}: the valley sinks steadily ({start:.3}, {half:.3}, {end:.3})"
        );
    }
}

#[test]
fn the_volcanos_wave_takes_coastal_people() {
    let mut world = World::new(1);
    world.step(2_499);
    let wave = world.history().waves[0];
    let (cx, cy) = (
        wave.center[0].round() as usize % GRID_WIDTH,
        wave.center[1].round() as usize,
    );
    let reach: Vec<usize> = hex::disc(cx, cy, wave.reach as u32)
        .into_iter()
        .map(|(x, y)| y * GRID_WIDTH + x)
        .collect();
    let people = |world: &World| reach.iter().map(|&t| world.population[t]).sum::<f32>();
    let before = people(&world);
    world.step(1);
    let after = people(&world);
    assert!(before > 0.0, "people live within the wave's reach");
    assert!(
        after < 0.9 * before,
        "the wave takes coastal people: {before:.0} -> {after:.0}"
    );
}

#[test]
fn a_loaded_world_places_the_same_catalog() {
    let mut world = World::new(2);
    world.step(120);
    let (_, mut loaded) = save::load(&save::save(&world)).expect("a fresh save loads");
    assert_eq!(loaded.history(), world.history());
    world.step(400);
    loaded.step(400);
    assert_eq!(save::save(&loaded), save::save(&world));
}
