//! Demography and culture contracts for Milestone 1 (design, Milestones ->
//! Living Map): the Garden, growth toward carrying capacity, crowding-driven
//! migration, culture drift/blending, and the split rule.
//!
//! These tests are the Milestone 1 "done when" gate in executable form: a
//! 2,000-year run must show three or more distinct peoples separated by
//! terrain.
#![expect(
    clippy::float_cmp,
    reason = "these contracts compare exact values: untouched land, infinite cost, symmetric tables"
)]

use sim_core::culture::{self, Favor, NO_CULTURE};
use sim_core::world::{GRID_HEIGHT, GRID_WIDTH, Overlay, World};
use sim_core::worldgen::{Biome, latitude_deg};

fn tiles() -> usize {
    GRID_WIDTH * GRID_HEIGHT
}

fn settled_tiles(world: &World) -> Vec<usize> {
    (0..tiles())
        .filter(|&i| world.population[i] >= 1.0)
        .collect()
}

#[test]
fn the_garden_plants_one_tile_per_line_on_habitable_land() {
    let world = World::new(1);
    let settled = settled_tiles(&world);
    assert_eq!(
        settled.len(),
        2,
        "humanity starts at a single origin, as two lines"
    );

    for (id, &tile) in settled.iter().enumerate() {
        assert!(
            world.carrying_capacity[tile] > 0.0,
            "each line must sit on land that can carry people"
        );
        assert_ne!(world.culture_id[tile], NO_CULTURE);
        assert_eq!(
            world.line_home(world.cultures()[world.culture_id[tile] as usize].line),
            Some(tile),
            "line {id}"
        );
    }
    assert_eq!(world.cultures().len(), 2);
    assert_eq!(world.cultures()[0].favor, Favor::Chosen);
}

#[test]
fn population_grows_toward_capacity_and_never_passes_it() {
    let mut world = World::new(1);
    let home = settled_tiles(&world)[0];
    let start = world.population[home];
    let capacity = world.carrying_capacity[home];

    world.step(300);
    let later = world.population[home];
    assert!(later > start, "population grows toward carrying capacity");
    assert!(
        later <= capacity + 1e-3,
        "logistic growth never overshoots capacity ({later} > {capacity})"
    );

    world.step(2_000);
    for &i in &settled_tiles(&world) {
        assert!(
            world.population[i] <= world.carrying_capacity[i] + 1e-3,
            "tile {i} exceeded its carrying capacity"
        );
    }
}

#[test]
fn ocean_and_ice_never_settle() {
    let mut world = World::new(1);
    world.step(1_000);
    for &i in &settled_tiles(&world) {
        assert!(
            !matches!(world.biome[i], Biome::Ocean | Biome::Ice),
            "tile {i} is {:?} but carries population",
            world.biome[i]
        );
    }
}

#[test]
fn crowding_drives_migration_onto_new_land() {
    let mut world = World::new(1);
    world.step(400);
    assert!(
        settled_tiles(&world).len() > 2,
        "crowded homes spill onto neighboring tiles beyond the two lines' first"
    );
}

#[test]
fn a_newly_settled_tile_inherits_its_source_culture() {
    let mut world = World::new(1);
    world.step(1_000);
    let settled = settled_tiles(&world);
    assert!(settled.len() > 2);

    let mut homelands = 0;
    for &i in &settled {
        let id = world.culture_id[i];
        assert_ne!(id, NO_CULTURE, "a populated tile always carries a culture");
        // Every people descends from one of the two founding lines (ids 0
        // and 1, Out of the Garden), so a culture is a line's root or has a
        // parent.
        let culture = &world.cultures()[id as usize];
        assert!(
            usize::from(culture.id) < culture::LINES || culture.parent_id.is_some(),
            "tile {i} carries an orphan culture"
        );
        if usize::from(id) < culture::LINES {
            homelands += 1;
        }
    }
    assert!(homelands > 0, "the homelands keep their cultures");
}

#[test]
fn culture_vectors_drift_away_from_the_garden_over_time() {
    let mut world = World::new(1);
    let origin = world.culture_vec_of(settled_tiles(&world)[0]);

    world.step(600);
    let settled = settled_tiles(&world);
    let drifted = settled
        .iter()
        .map(|&i| culture::distance(&world.culture_vec_of(i), &origin))
        .fold(0.0_f32, f32::max);
    assert!(
        drifted > 0.05,
        "peoples adapt to their regions (max drift {drifted})"
    );
}

#[test]
fn step_cost_makes_mountains_and_deserts_harder_than_grassland() {
    let lat = 0.0;
    let grass = culture::step_cost(Biome::Grassland, lat, true);
    let forest = culture::step_cost(Biome::Forest, lat, true);
    let desert = culture::step_cost(Biome::Desert, lat, true);
    let mountain = culture::step_cost(Biome::Mountain, lat, true);

    assert!(forest > grass && desert > forest && mountain > desert);
    assert_eq!(culture::step_cost(Biome::Ice, lat, true), f32::INFINITY);
    assert_eq!(culture::step_cost(Biome::Ocean, lat, true), f32::INFINITY);
}

#[test]
fn east_west_steps_get_cheaper_toward_the_poles_and_north_south_does_not() {
    let equator_east_west = culture::step_cost(Biome::Grassland, 0.0, true);
    let polar_east_west = culture::step_cost(Biome::Grassland, 80.0, true);
    assert!(
        polar_east_west < equator_east_west,
        "R2-10: east-west cost is multiplied by cos(lat), so polar steps cost less"
    );

    let equator_north_south = culture::step_cost(Biome::Grassland, 0.0, false);
    let polar_north_south = culture::step_cost(Biome::Grassland, 80.0, false);
    assert_eq!(equator_north_south, polar_north_south);
}

#[test]
fn same_seed_gives_the_same_history() {
    let mut a = World::new(7);
    let mut b = World::new(7);
    a.step(400);
    b.step(400);
    assert_eq!(a.population, b.population);
    assert_eq!(a.culture_id, b.culture_id);
    assert_eq!(a.culture_vec, b.culture_vec);
    assert_eq!(a.cultures().len(), b.cultures().len());
}

#[test]
fn exactly_one_culture_is_chosen_at_any_time() {
    let mut world = World::new(1);
    for _ in 0..10 {
        world.step(200);
        let chosen = world
            .cultures()
            .iter()
            .filter(|c| c.favor == Favor::Chosen)
            .count();
        assert_eq!(chosen, 1, "exactly one Chosen culture at any time");
    }
}

#[test]
fn a_split_creates_a_daughter_with_parent_id_and_a_distinct_vector() {
    // Measured when each daughter is founded, against her mother's centroid
    // the year before: a daughter founded beside her mother's camps can
    // assimilate later, and that rate is the Stage 1 sweep's to report (D21;
    // author's decision 2026-10-28). A daughter whose walkers crossed a strait
    // or a range is distinct by that separation and diverges in isolation
    // (D24; M1's "separated by terrain").
    let mut world = World::new(1);
    let (mut distinct, mut parted) = (0, 0);
    for _ in 0..2_000 {
        let known = world.cultures().len();
        let centroids: Vec<_> = world.cultures().iter().map(|c| c.centroid).collect();
        let walking: Vec<_> = world.bands().to_vec();
        world.step(1);
        for daughter in &world.cultures()[known..] {
            let parent = daughter.parent_id.expect("every new people has a parent");
            let distance = culture::distance(&daughter.centroid, &centroids[parent as usize]);
            let crossed = walking.iter().any(|band| {
                band.culture_id == parent
                    && band.crossed()
                    && world.bands().iter().all(|now| now.id != band.id)
            });
            if distance >= 0.2 {
                distinct += 1;
            } else {
                assert!(
                    crossed,
                    "{} is founded neither different ({distance:.3}) nor across terrain",
                    daughter.name
                );
                parted += 1;
            }
        }
    }
    assert!(
        distinct + parted > 0,
        "peoples split (cultures: {})",
        world.cultures().len()
    );
}

#[test]
fn two_thousand_years_yields_three_or_more_peoples() {
    let mut world = World::new(1);
    world.step(2_000);

    let peoples = world.cultures().len();
    assert!(
        peoples >= 3,
        "Milestone 1 done-when: 3+ distinct peoples, got {peoples} (settled {} tiles)",
        settled_tiles(&world).len()
    );
}

#[test]
fn split_components_are_measured_over_settled_tiles_only() {
    let world = World::new(1);
    let components = culture::components(&world, world.cultures()[0].id);
    assert_eq!(
        components.len(),
        1,
        "Dirt's first home is one connected settlement"
    );
    assert_eq!(components[0].len(), 1);
}

#[test]
fn culture_overlay_lights_settled_tiles_and_desaturates_the_rest() {
    let world = World::new(1);
    let overlay = world.paint_overlay(Overlay::Culture);
    assert_eq!(overlay.len(), tiles() * 4);

    let home = settled_tiles(&world)[0];
    let unsettled_land = (0..tiles())
        .find(|&i| {
            world.biome[i] != Biome::Ocean
                && world.biome[i] != Biome::Ice
                && world.population[i] < 1.0
        })
        .expect("the map has unsettled land");

    let settled_px = &overlay[home * 4..home * 4 + 3];
    let unsettled_px = &overlay[unsettled_land * 4..unsettled_land * 4 + 3];
    assert_ne!(
        settled_px, unsettled_px,
        "the first home is lit, the frontier is not"
    );

    let spread = |px: &[u8]| {
        let max = i32::from(*px.iter().max().unwrap());
        let min = i32::from(*px.iter().min().unwrap());
        max - min
    };
    assert!(
        spread(unsettled_px) < spread(settled_px),
        "unsettled land is desaturated (design, Map states)"
    );
}

#[test]
fn latitude_rows_keep_their_latitude() {
    assert!(latitude_deg(0) > 88.0);
    assert!(latitude_deg(GRID_HEIGHT - 1) < -88.0);
}
