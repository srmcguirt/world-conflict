//! Milestone 4 contracts (design, Physical adaptation): trait drift toward
//! environment, cosmetic differences, small regional perks, and the individual
//! traits a portrait is drawn from.
//!
//! Adaptation is descriptive, never a ranking: no trait is better in the
//! abstract, only better suited to where a people lives.

use sim_core::traits::{self, TRAITS};
use sim_core::world::World;

/// Forces one trait axis on every populated tile of a people.
///
/// Stature and skin are cosmetic; cold and altitude carry the regional perks.
fn set_culture_trait(world: &mut World, culture_id: u16, axis: usize, value: f32) {
    for tile in 0..world.population.len() {
        if world.culture_id[tile] == culture_id && world.population[tile] >= 1.0 {
            let start = tile * TRAITS;
            world.traits[start + axis] = value;
        }
    }
}

/// A tile's traits, copied out.
fn of(world: &World, tile: usize) -> [f32; TRAITS] {
    let mut out = [0.0; TRAITS];
    out.copy_from_slice(&world.traits[tile * TRAITS..(tile + 1) * TRAITS]);
    out
}

/// The skin a tile's people average, across a whole people.
fn mean_skin(world: &World, culture_id: u16) -> f32 {
    let culture = &world.cultures()[culture_id as usize];
    culture.mean_skin
}

#[test]
fn the_sun_writes_skin() {
    let mut world = World::new(1);
    world.step(2_000);
    assert!(world.cultures().len() >= 3, "several peoples spread");

    // The people whose home sits nearest the equator vs nearest a pole.
    let mut latitudes = Vec::new();
    for culture in world.cultures() {
        latitudes.push((home_latitude(&world, culture.id).abs(), culture.id));
    }
    latitudes.sort_by(|a, b| a.0.total_cmp(&b.0));
    let equator = latitudes.first().unwrap().1;
    let pole = latitudes.last().unwrap().1;

    assert!(
        mean_skin(&world, equator) > mean_skin(&world, pole) + 0.1,
        "skin follows the sun (equator {:.2} vs pole {:.2})",
        mean_skin(&world, equator),
        mean_skin(&world, pole)
    );
}

#[test]
fn highlanders_grow_lungs() {
    let mut world = World::new(2);
    world.step(100);
    let culture = world.cultures()[0].id;
    // Whatever land they hold now, force every trait axis to its floor and let
    // the environment pull it back up.
    for axis in 0..TRAITS {
        set_culture_trait(&mut world, culture, axis, 0.0);
    }
    world.step(300);

    // Somewhere in the people's range there is high ground: its people should
    // have moved clearly off the floor. Low plains move skin and stature only.
    let mut best_altitude = 0.0_f32;
    let mut coldest_temperature = 1.0_f32;
    for tile in 0..world.population.len() {
        if world.culture_id[tile] != culture || world.population[tile] < 1.0 {
            continue;
        }
        best_altitude = best_altitude.max(of(&world, tile)[traits::ALTITUDE]);
        coldest_temperature = coldest_temperature.min(world.temperature[tile]);
    }
    assert!(
        best_altitude > 0.2,
        "generations on high ground grow lungs (best {best_altitude})"
    );
    assert!(
        coldest_temperature < 0.6,
        "the test range reaches cool country (coldest {coldest_temperature})"
    );
}

#[test]
fn adaptation_takes_generations() {
    let mut world = World::new(2);
    world.step(100);
    let culture = world.cultures()[0].id;
    set_culture_trait(&mut world, culture, traits::ALTITUDE, 0.0);
    world.step(15); // a decade and a half is not a lineage

    let mut best_altitude = 0.0_f32;
    for tile in 0..world.population.len() {
        if world.culture_id[tile] == culture && world.population[tile] >= 1.0 {
            best_altitude = best_altitude.max(of(&world, tile)[traits::ALTITUDE]);
        }
    }
    assert!(
        best_altitude < 0.1,
        "nobody adapts in fifteen years (best {best_altitude})"
    );
}

#[test]
fn a_cold_adapted_people_hold_the_north() {
    // Two worlds, same seed and same place: one settles a cold tile with
    // cold-adapted bodies, one without. Adaptation is a small perk where it
    // matters and nothing everywhere else.
    let find_cold_tile = |world: &World| -> Option<usize> {
        // A cold tile with living people and real room to grow: once a tile
        // sits at capacity the perk has nowhere to show.
        (0..world.population.len())
            .filter(|&tile| {
                world.population[tile] >= 1.0
                    && world.temperature[tile] < traits::COLD_LIMIT
                    && sim_core::worldgen::settleable(world.biome[tile])
                    && world.carrying_capacity[tile] - world.population[tile]
                        > 0.35 * world.carrying_capacity[tile]
            })
            .max_by_key(|&tile| (world.carrying_capacity[tile] - world.population[tile]).to_bits())
    };
    // Walk seeds until a people reaches cold country; the contract is about
    // what adaptation does once they are there, not about worldgen luck.
    let mut adapted = None;
    let mut tile = None;
    for seed in 1..20_u64 {
        let mut world = World::new(seed);
        for _ in 0..8 {
            world.step(50);
            if let Some(hit) = find_cold_tile(&world) {
                adapted = Some(world);
                tile = Some(hit);
                break;
            }
        }
        if adapted.is_some() {
            break;
        }
    }
    let mut adapted = adapted.expect("some seed reaches cold country");
    let tile = tile.unwrap();

    // One tile differs, so the worlds stay identical everywhere else and the
    // comparison stays surgical.
    let start = tile * traits::TRAITS;
    let mut plain = adapted.clone();
    adapted.traits[start + traits::COLD] = 1.0;
    plain.traits[start + traits::COLD] = 0.0;

    // A short window: the same drift that gives the perk would, over many
    // generations, re-adapt both peoples and close the gap again. Migration
    // reshuffles people but never removes them, so the world totals differ
    // only by what the adaptation allowed the land to carry.
    adapted.step(10);
    plain.step(10);
    let total = |world: &World| -> f32 { world.population.iter().sum() };
    assert!(
        total(&adapted) > total(&plain),
        "cold-country bodies let the land carry more people",
    );
    // Warm tiles feel nothing.
    let control = |world: &World| -> Vec<f32> {
        (0..world.population.len())
            .filter(|&tile| world.temperature[tile] > 0.7 && world.population[tile] >= 1.0)
            .map(|tile| world.population[tile])
            .collect()
    };
    assert_eq!(control(&adapted), control(&plain));
}

#[test]
fn looks_cost_nothing() {
    let mut world = World::new(4);
    world.step(50);
    let culture = world.cultures()[0].id;

    set_culture_trait(&mut world, culture, traits::SKIN, 1.0);
    set_culture_trait(&mut world, culture, traits::STATURE, 1.0);
    let mut plain = world.clone();
    set_culture_trait(&mut plain, culture, traits::SKIN, 0.0);
    set_culture_trait(&mut plain, culture, traits::STATURE, 0.0);

    world.step(50);
    plain.step(50);
    assert_eq!(
        world.population, plain.population,
        "skin and stature are purely how people look"
    );
}

#[test]
fn a_witness_takes_the_traits_of_their_people() {
    let mut world = World::new(5);
    world.step(50);
    let eden = world.eden_tile().expect("Eden exists");
    world.queue_power(sim_core::powers::Power::Meteor, eden % 160, eden / 160);
    world.step(5);

    let witness = world
        .witnesses()
        .into_iter()
        .next()
        .expect("a meteor makes a witness");
    let tile_traits = of(&world, witness.tile as usize);
    assert!(
        (witness.skin - tile_traits[traits::SKIN]).abs() < 0.01,
        "portraits come from the tile the person is born on"
    );
}

#[test]
fn the_portrait_is_stable_and_personal() {
    let mut world = World::new(5);
    world.step(50);
    let eden = world.eden_tile().expect("Eden exists");
    world.queue_power(sim_core::powers::Power::Meteor, eden % 160, eden / 160);
    world.step(5);
    let witness = world.witnesses().into_iter().next().unwrap();

    let again = traits::portrait(&witness, traits::dress_of(&world, witness.culture_id));
    let same = traits::portrait(&witness, traits::dress_of(&world, witness.culture_id));
    assert_eq!(again, same, "same person, same face");
    assert_eq!(again.len(), traits::PORTRAIT_W * traits::PORTRAIT_H * 4);
}

#[test]
fn the_traits_overlay_paints_skin() {
    let mut world = World::new(6);
    world.step(50);
    let culture = world.cultures()[0].id;
    let tile = (0..world.population.len())
        .find(|&tile| world.culture_id[tile] == culture && world.population[tile] >= 1.0)
        .expect("a settled tile");

    world.set_trait(tile, traits::SKIN, 0.0);
    let pale = world.paint_overlay(sim_core::world::Overlay::Traits);
    world.set_trait(tile, traits::SKIN, 1.0);
    let dark = world.paint_overlay(sim_core::world::Overlay::Traits);

    let pixel = |bytes: &[u8]| (bytes[tile * 4], bytes[tile * 4 + 1], bytes[tile * 4 + 2]);
    assert_ne!(pale, dark, "skin shows on the map");
    let (r, _, b) = pixel(&dark);
    assert!(r > b, "dark skin reads warm");
}

/// The people's population-weighted home latitude, in degrees.
fn home_latitude(world: &World, culture_id: u16) -> f32 {
    let mut weighted = 0.0_f32;
    let mut people = 0.0_f32;
    for tile in 0..world.population.len() {
        if world.culture_id[tile] != culture_id || world.population[tile] < 1.0 {
            continue;
        }
        let y = (tile / sim_core::world::GRID_WIDTH) as f32;
        weighted += y * world.population[tile];
        people += world.population[tile];
    }
    let row = weighted / people;
    90.0 - (row + 0.5) * (180.0 / sim_core::world::GRID_HEIGHT as f32)
}
