//! Walking bands (design, Out of the Garden step 2; D7, D8, D10, D16, D20,
//! D21, D22).
//!
//! Outside claimed land people move only as bands: a crowded settlement buds
//! a band of 25, the band walks a planned route one step a year, and a band of
//! 8 or more founds a tier-0 camp on unclaimed land. A band that crossed water
//! or a mountain range, or whose culture has drifted past the split distance,
//! founds a daughter people (D21, D24). At most 64 walk
//! at once, and a band still walking at 100 years heads home to its kin.

use sim_core::bands::{self, Goal};
use sim_core::culture::{self, NO_CULTURE};
use sim_core::world::{GRID_WIDTH, World};
use sim_core::{hex, save};

const SEEDS: [u64; 4] = [1, 2, 3, 5];

fn hexes(a: usize, b: usize) -> u32 {
    hex::distance(
        a % GRID_WIDTH,
        a / GRID_WIDTH,
        b % GRID_WIDTH,
        b / GRID_WIDTH,
    )
}

#[test]
fn crowded_settlements_bud_bands_of_twenty_five() {
    let mut world = World::new(1);
    let mut buds = 0;
    for _ in 0..600 {
        world.step(1);
        // A bud stands on its settlement's own tile in its first year, 25
        // strong; bands that split or part on the walk can form anywhere.
        for band in world.bands() {
            let home = world
                .settlements()
                .iter()
                .any(|s| s.tile == band.tile as usize);
            if band.formed_tick == world.tick() && home {
                if (band.headcount - bands::BUD_SIZE).abs() < 0.01 {
                    buds += 1;
                }
                assert!(
                    band.headcount >= bands::SETTLE_MIN,
                    "every new band can found a camp, got {}",
                    band.headcount
                );
            }
        }
    }
    assert!(
        buds > 0,
        "600 years of growth crowd the first camps into budding"
    );
    assert!(
        world.settlements().len() > 2,
        "bands found camps beyond the two lines' first"
    );
}

#[test]
fn people_live_only_on_claimed_land() {
    for seed in SEEDS {
        let mut world = World::new(seed);
        for _ in 0..800 {
            world.step(1);
            for i in 0..world.population.len() {
                if world.population[i] >= 1.0 {
                    assert_ne!(
                        world.settlement_of(i),
                        u32::MAX,
                        "seed {seed} year {}: tile {i} holds people outside every catchment",
                        world.tick()
                    );
                }
            }
        }
    }
}

#[test]
fn tile_migration_never_crosses_into_another_peoples_catchment() {
    let mut world = World::new(2);
    for _ in 0..1_000 {
        let before: Vec<u16> = world.culture_id.clone();
        let camps = world.settlements().len();
        let tick = world.tick() + 1;
        world.step(1);
        // Splits and the 25-year pass relabel tiles on purpose; between them a
        // newly lit tile is lit by its own settlement's people.
        if tick.is_multiple_of(25) {
            continue;
        }
        for i in 0..world.population.len() {
            if before[i] != NO_CULTURE || world.culture_id[i] == NO_CULTURE {
                continue;
            }
            let owner = world.settlement_of(i);
            let settlement = &world.settlements()[owner as usize];
            let lit_by_kin = settlement.culture_id == world.culture_id[i]
                || settlement
                    .catchment
                    .iter()
                    .any(|&t| t != i && before[t] == world.culture_id[i]);
            let new_camp = owner as usize >= camps;
            assert!(
                lit_by_kin || new_camp,
                "year {tick}: tile {i} took a people that does not live in its catchment"
            );
        }
    }
}

#[test]
fn a_band_founds_a_tier_zero_camp_only_on_unclaimed_land() {
    let mut founded = 0;
    for seed in SEEDS {
        let mut world = World::new(seed);
        for _ in 0..600 {
            let claimed: Vec<u32> = (0..world.population.len())
                .map(|i| world.settlement_of(i))
                .collect();
            let count = world.settlements().len();
            world.step(1);
            for settlement in &world.settlements()[count..] {
                founded += 1;
                assert_eq!(settlement.tier, 0, "a new camp starts at tier 0");
                // A tile the sea or an emptied settlement released this year
                // is open again (D16, D27); one still held by another is not.
                for &tile in &settlement.catchment {
                    let held = claimed[tile] != u32::MAX
                        && world.settlements()[claimed[tile] as usize]
                            .catchment
                            .contains(&tile);
                    assert!(
                        !held,
                        "seed {seed}: a camp claimed tile {tile}, which another still holds"
                    );
                }
                assert!(
                    settlement.population >= bands::SETTLE_MIN - 0.01,
                    "a camp starts with its band's people, got {}",
                    settlement.population
                );
            }
        }
    }
    assert!(founded > 0, "bands found camps");
}

#[test]
fn a_camp_founds_a_daughter_people_only_across_terrain_or_drift() {
    let (mut kin_camps, mut daughter_camps) = (0, 0);
    for seed in SEEDS {
        let mut world = World::new(seed);
        for _ in 0..1_500 {
            let walking: Vec<_> = world.bands().to_vec();
            let centroids: Vec<_> = world.cultures().iter().map(|c| c.centroid).collect();
            let count = world.settlements().len();
            world.step(1);
            for settlement in &world.settlements()[count..] {
                // The band that founded it stood beside it last year and is gone.
                let Some(band) = walking.iter().find(|b| {
                    hexes(b.tile as usize, settlement.tile) <= 1
                        && world.bands().iter().all(|now| now.id != b.id)
                }) else {
                    continue;
                };
                let drift =
                    culture::distance(&band.culture_vec, &centroids[band.culture_id as usize]);
                let people = &world.cultures()[settlement.culture_id as usize];
                if band.crossed() || drift > culture::SPLIT_MIN_DISTANCE {
                    daughter_camps += 1;
                    assert_eq!(
                        people.parent_id,
                        Some(band.culture_id),
                        "seed {seed}: walkers that crossed ({}) or drifted {drift:.2} found a daughter",
                        band.crossed()
                    );
                } else if settlement.culture_id == band.culture_id {
                    kin_camps += 1;
                } else {
                    panic!(
                        "seed {seed}: walkers that stayed on open land, only {drift:.2} from their people, founded {}",
                        people.name
                    );
                }
            }
        }
    }
    assert!(kin_camps > 0, "most camps stay their people's");
    assert!(
        daughter_camps < kin_camps,
        "daughters are the exception, {daughter_camps} of {}",
        kin_camps + daughter_camps
    );
}

#[test]
fn no_more_than_sixty_four_bands_walk_at_once() {
    for seed in SEEDS {
        let mut world = World::new(seed);
        let mut most = 0;
        for _ in 0..2_000 {
            world.step(1);
            most = most.max(world.bands().len());
            assert!(
                world.bands().len() <= bands::MAX_WALKING,
                "seed {seed} year {}: {} bands walking",
                world.tick(),
                world.bands().len()
            );
        }
        assert!(most > 0, "seed {seed}: bands walk at all");
    }
}

#[test]
fn a_band_still_walking_at_one_hundred_years_heads_to_its_kin() {
    for seed in SEEDS {
        let mut world = World::new(seed);
        for _ in 0..2_000 {
            world.step(1);
            for band in world.bands() {
                let age = world.tick() - band.formed_tick;
                if age >= bands::STRANDED_YEARS {
                    assert!(
                        matches!(band.goal, Goal::Kin(_)),
                        "seed {seed}: band {} is {age} years old and still looking for land",
                        band.id
                    );
                }
            }
        }
    }
}

#[test]
fn walkers_count_in_their_peoples_population_and_belief() {
    let mut world = World::new(1);
    let mut checked = 0;
    for _ in 0..80 {
        world.step(10);
        for culture in world.cultures() {
            let walkers: f32 = world
                .bands()
                .iter()
                .filter(|b| b.culture_id == culture.id)
                .map(|b| b.headcount)
                .sum();
            if walkers <= 0.0 {
                continue;
            }
            let settled: f32 = (0..world.population.len())
                .filter(|&i| world.culture_id[i] == culture.id && world.population[i] >= 1.0)
                .map(|i| world.population[i])
                .sum();
            checked += 1;
            assert!(
                (culture.population - (settled + walkers)).abs() < 0.01 * culture.population,
                "{} counts {} but holds {settled} on tiles and {walkers} walking",
                culture.name,
                culture.population
            );
        }
    }
    assert!(checked > 0, "some people had walkers at a refresh");
}

#[test]
fn a_walking_band_shows_its_people_on_the_culture_map() {
    let mut world = World::new(1);
    while world.bands().is_empty() {
        world.step(1);
        assert!(world.tick() < 1_000, "a band forms");
    }
    let band = &world.bands()[0];
    let tile = band.tile as usize;
    let bytes = world.paint_overlay(sim_core::world::Overlay::Culture);
    assert_eq!(
        bytes[tile * 4..tile * 4 + 3],
        culture::color(band.culture_id),
        "the band's tile wears its people's color"
    );
}

#[test]
fn bands_survive_a_save_and_load_and_replay_the_same() {
    let mut world = World::new(3);
    while world.bands().is_empty() {
        world.step(1);
    }
    let bytes = save::save(&world);
    let (_, mut loaded) = save::load(&bytes).expect("a fresh save loads");
    assert_eq!(loaded.bands(), world.bands());
    world.step(300);
    loaded.step(300);
    assert_eq!(
        save::save(&loaded),
        save::save(&world),
        "a loaded world walks its bands the same way"
    );
}

#[test]
fn the_same_seed_walks_the_same_bands() {
    let run = || {
        let mut world = World::new(5);
        world.step(700);
        save::save(&world)
    };
    assert_eq!(run(), run());
}
