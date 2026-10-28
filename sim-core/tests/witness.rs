//! Milestone 2 contracts (design, Simulation contracts -> Witness rule, and
//! Player actions are queued): the six powers, witness promotion, generated
//! names, and scripture written by the people who watched.
//!
//! The milestone's own gate is here too: smite something, then read what
//! someone believed you meant by it.
#![expect(
    clippy::float_cmp,
    reason = "these contracts compare exact values: untouched land, infinite cost, symmetric tables"
)]

use sim_core::powers::Power;
use sim_core::world::{GRID_WIDTH, World};
use sim_core::worldgen::Biome;

fn home(world: &World) -> (usize, usize) {
    let tile = world
        .line_home(sim_core::culture::Line::Dirt)
        .expect("Dirt is planted");
    (tile % GRID_WIDTH, tile / GRID_WIDTH)
}

fn populated_tiles(world: &World) -> Vec<usize> {
    (0..world.population.len())
        .filter(|&i| world.population[i] >= 1.0)
        .collect()
}

#[test]
fn a_queued_power_applies_at_the_start_of_the_next_tick() {
    let mut world = World::new(1);
    let (x, y) = home(&world);
    let before = world.population[home(&world).1 * GRID_WIDTH + home(&world).0];

    world.queue_power(Power::Meteor, x, y);
    assert_eq!(
        world.population[y * GRID_WIDTH + x],
        before,
        "a queued power does not apply until the next tick"
    );

    world.step(1);
    assert!(
        world.population[y * GRID_WIDTH + x] < before,
        "the meteor lands on the next tick"
    );
}

#[test]
fn the_action_log_records_the_tick_the_power_and_the_tile() {
    let mut world = World::new(1);
    let (x, y) = home(&world);
    world.queue_power(Power::Meteor, x, y);
    world.step(1);

    let action = world.actions().last().expect("one action");
    assert_eq!(action.tick, 1);
    assert_eq!(
        action.kind,
        sim_core::powers::ActionKind::Power {
            power: Power::Meteor,
            x,
            y
        }
    );
}

#[test]
fn a_meteor_kills_most_people_under_it() {
    let mut world = World::new(1);
    world.step(200);
    let (x, y) = home(&world);
    let tile = y * GRID_WIDTH + x;
    let before = world.population[tile];

    world.queue_power(Power::Meteor, x, y);
    world.step(1);
    assert!(
        world.population[tile] < before * 0.5,
        "a meteor wipes out most of the tile it falls on ({} -> {})",
        before,
        world.population[tile]
    );
}

#[test]
fn a_meteor_promotes_two_witnesses_and_writes_a_verse() {
    let mut world = World::new(1);
    world.step(300);
    let settled = populated_tiles(&world);
    assert!(settled.len() > 3, "the world has people to witness");

    let (x, y) = home(&world);
    world.queue_power(Power::Meteor, x, y);
    world.step(1);

    let witnesses = world.witnesses();
    assert_eq!(witnesses.len(), 2, "a meteor promotes two witnesses");
    for witness in &witnesses {
        assert!(!witness.name.is_empty(), "a witness has a generated name");
        assert!(
            (witness.culture_id as usize) < world.cultures().len(),
            "the witness belongs to a real people"
        );
    }

    let cultures_with_verses = world
        .cultures()
        .iter()
        .filter(|culture| !world.scripture_text(culture.id, 0, 16).is_empty())
        .count();
    assert!(
        cultures_with_verses >= 1,
        "the witness's people write it down"
    );
}

#[test]
fn no_people_means_no_witness_and_no_verse() {
    let mut world = World::new(1);
    let ocean = (0..world.biome.len())
        .find(|&i| world.biome[i] == Biome::Ocean)
        .expect("an ocean tile");
    let (x, y) = (ocean % GRID_WIDTH, ocean / GRID_WIDTH);

    world.step(300);
    let peoples = world.cultures().len();
    let before: usize = verse_counts(&world).iter().sum();

    world.queue_power(Power::Meteor, x, y);
    world.step(10);

    assert!(world.witnesses().is_empty(), "nobody saw it");
    // A people founded in these ten years inherits its mother's verses; only
    // the peoples that saw the sky could have written one.
    let after: usize = verse_counts(&world).iter().take(peoples).sum();
    assert_eq!(after, before, "nobody wrote a new verse");
}

fn verse_counts(world: &World) -> Vec<usize> {
    world
        .cultures()
        .iter()
        .map(|culture| world.scripture_text(culture.id, 0, 64).len())
        .collect()
}

#[test]
fn a_witnessed_meteor_turns_the_witness_wrathful() {
    let mut world = World::new(1);
    world.step(300);
    let (x, y) = home(&world);
    let tile = y * GRID_WIDTH + x;
    let before = world.belief_of(tile);

    world.queue_power(Power::Meteor, x, y);
    world.step(10); // the 5-tick outcome window closes inside this

    let after = world.belief_of(tile);
    assert!(
        after.wrathful > before.wrathful,
        "a meteor reads as wrath ({} -> {})",
        before.wrathful,
        after.wrathful
    );
    assert!(after.capricious > before.capricious);
}

#[test]
fn a_blessing_raises_capacity_and_expires_after_ten_years() {
    let mut world = World::new(1);
    world.step(100);
    let (x, y) = home(&world);
    let tile = y * GRID_WIDTH + x;
    let before = world.carrying_capacity[tile];

    world.queue_power(Power::Bless, x, y);
    world.step(1);
    let blessed = world.carrying_capacity[tile];
    assert!(
        blessed > before,
        "blessing raises fertility ({before} -> {blessed})"
    );

    world.step(12);
    assert!(
        (world.carrying_capacity[tile] - before).abs() < 1e-3,
        "the blessing expires after ten years"
    );
}

#[test]
fn raising_land_makes_a_tile_hold_more_people() {
    let mut world = World::new(1);
    let (x, y) = home(&world);
    let tile = y * GRID_WIDTH + x;
    let before = world.carrying_capacity[tile];
    let elevation = world.elevation[tile];

    world.queue_power(Power::Raise, x, y);
    world.step(1);

    assert!(world.elevation[tile] > elevation);
    assert!(world.carrying_capacity[tile] >= before);
}

#[test]
fn painting_a_biome_changes_what_the_tile_can_carry() {
    let mut world = World::new(1);
    let (x, y) = home(&world);
    let tile = y * GRID_WIDTH + x;

    world.queue_power(Power::Paint(Biome::Desert), x, y);
    world.step(1);
    assert_eq!(world.biome[tile], Biome::Desert);

    let capacity = world.carrying_capacity[tile];
    world.queue_power(Power::Paint(Biome::Grassland), x, y);
    world.step(1);
    assert_eq!(world.biome[tile], Biome::Grassland);
    assert!(world.carrying_capacity[tile] > capacity);
}

#[test]
fn a_tornado_kills_people_without_reshaping_the_land() {
    let mut world = World::new(1);
    world.step(300);
    let (x, y) = home(&world);
    let tile = y * GRID_WIDTH + x;
    let before = world.population[tile];
    let elevation = world.elevation[tile];
    let biome = world.biome[tile];

    world.queue_power(Power::Tornado, x, y);
    world.step(1);

    assert!(world.population[tile] < before, "a tornado kills");
    assert_eq!(world.elevation[tile], elevation);
    assert_eq!(world.biome[tile], biome);
}

#[test]
fn spawned_beasts_hunt_the_local_people() {
    let mut world = World::new(1);
    world.step(300);
    let (x, y) = home(&world);
    let tile = y * GRID_WIDTH + x;
    let before = world.population[tile];

    world.queue_power(Power::Spawn, x, y);
    world.step(20);
    assert!(
        world.population[tile] < before,
        "beasts thin the herd ({} -> {})",
        before,
        world.population[tile]
    );
}

#[test]
fn witnesses_come_from_the_most_populous_affected_tile() {
    let mut world = World::new(1);
    world.step(200);
    let (x, y) = home(&world);
    let footprint = sim_core::powers::tiles_around(x, y, 1);
    // Read the population before the tick: the witness is chosen from the
    // world as it was when the power landed.
    let best = footprint
        .iter()
        .map(|&tile| world.population[tile])
        .fold(0.0_f32, f32::max);
    world.queue_power(Power::Bless, x, y);
    world.step(1);

    let witnesses = world.witnesses();
    assert_eq!(witnesses.len(), 1);
    let witness = &witnesses[0];
    assert!(
        world.population[witness.tile as usize] >= best - 1e-3,
        "a witness is drawn from the most populous tile that saw it"
    );
}

#[test]
fn a_chosen_witness_is_preferred_when_both_peoples_are_watching() {
    let mut world = World::new(1);
    world.step(200);
    let (x, y) = home(&world);
    world.queue_power(Power::Bless, x, y);
    world.step(1);

    let witness = world.witnesses()[0].clone();
    assert_eq!(
        world.cultures()[witness.culture_id as usize].favor,
        sim_core::culture::Favor::Chosen,
        "Dirt's people are the Chosen, so theirs is the witness"
    );
}

#[test]
fn names_are_deterministic_and_differ_between_peoples() {
    let mut a = World::new(1);
    a.step(400);
    let names_a: Vec<String> = a
        .cultures()
        .iter()
        .map(|culture| sim_core::names::person_name(culture, 7))
        .collect();

    let mut b = World::new(1);
    b.step(400);
    let names_b: Vec<String> = b
        .cultures()
        .iter()
        .map(|culture| sim_core::names::person_name(culture, 7))
        .collect();

    assert_eq!(names_a, names_b, "names are a pure function of the seed");
    assert!(names_a.iter().all(|name| !name.is_empty()));
    if names_a.len() > 1 {
        assert_ne!(
            names_a[0], names_a[1],
            "different peoples name their children differently"
        );
    }
}

#[test]
fn the_witness_budget_is_bounded_and_old_witnesses_become_legends() {
    let mut world = World::new(1);
    world.step(300);
    let (x, y) = home(&world);

    for _ in 0..400 {
        world.queue_power(Power::Bless, x, y);
        world.step(1);
    }

    assert!(
        world.witnesses().len() <= 300,
        "the witness budget is a hard cap ({} witnesses)",
        world.witnesses().len()
    );
    assert!(
        !world.legends().is_empty(),
        "evicted witnesses leave legend records"
    );
}

#[test]
fn scripture_reads_newest_first_and_names_its_witness() {
    let mut world = World::new(1);
    world.step(300);
    let (x, y) = home(&world);
    world.queue_power(Power::Meteor, x, y);
    world.step(10);

    let culture_id = world.witnesses()[0].culture_id;
    let verses = world.scripture_text(culture_id, 0, 8);
    assert!(!verses.is_empty(), "the people wrote it down");

    let witness = &world.witnesses()[0];
    assert!(
        verses[0].contains(&witness.name) || verses[0].contains("Watcher"),
        "a verse names its witness or its god: {:?}",
        verses[0]
    );
    assert!(!verses[0].is_empty());
}

#[test]
fn the_same_seed_and_the_same_actions_write_the_same_scripture() {
    let mut a = World::new(9);
    let mut b = World::new(9);
    a.step(200);
    b.step(200);

    let (x, y) = home(&a);
    for world in [&mut a, &mut b] {
        world.queue_power(Power::Meteor, x, y);
        world.queue_power(Power::Bless, x, y);
        world.step(20);
    }

    let verse_a = a.scripture_text(a.cultures()[0].id, 0, 4);
    let verse_b = b.scripture_text(b.cultures()[0].id, 0, 4);
    assert_eq!(verse_a, verse_b);
    assert_eq!(
        a.witnesses()
            .iter()
            .map(|w| w.name.clone())
            .collect::<Vec<_>>(),
        b.witnesses()
            .iter()
            .map(|w| w.name.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn scripture_survives_a_split_into_the_daughter_people() {
    let mut world = World::new(1);
    world.step(300);
    let (x, y) = home(&world);
    world.queue_power(Power::Meteor, x, y);
    world.step(1_800);

    assert!(world.cultures().len() > 1, "peoples split");
    let daughters = world
        .cultures()
        .iter()
        .filter(|culture| culture.parent_id.is_some())
        .count();
    assert!(daughters > 0);
    let total_verses: usize = world
        .cultures()
        .iter()
        .map(|culture| world.scripture_text(culture.id, 0, 64).len())
        .sum();
    assert!(
        total_verses >= 1,
        "the verse is still in someone's scripture"
    );
}
