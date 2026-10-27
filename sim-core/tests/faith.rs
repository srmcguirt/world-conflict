//! Milestone 3 contracts (design, Belief model; Simulation contracts ->
//! Schism, Chosen vs. rival): culture-level belief, doctrines, prophets,
//! schisms, and re-anointing.
//!
//! The milestone's gate: rival religions about the same god, formed by what the
//! people saw the god do.

use sim_core::belief::Belief;
use sim_core::culture::Favor;
use sim_core::faith::Doctrine;
use sim_core::world::World;

/// Sets every tile of `culture_id` to `belief`.
fn set_culture_belief(world: &mut World, culture_id: u16, belief: Belief) {
    for tile in 0..world.population.len() {
        if world.culture_id[tile] == culture_id {
            world.set_belief(tile, belief);
        }
    }
}

fn angry() -> Belief {
    Belief {
        wrathful: 1.0,
        benevolent: 0.0,
        capricious: 0.0,
        protective: 0.0,
        jealous: 0.0,
        absent: 0.0,
    }
}

fn culture_ids(world: &World) -> Vec<u16> {
    world.cultures().iter().map(|c| c.id).collect()
}

#[test]
fn a_cultures_belief_is_the_population_weighted_mean_of_its_tiles() {
    let mut world = World::new(1);
    world.step(50);
    let culture = world.cultures()[0].id;

    let mut holy = Belief::START;
    holy.wrathful = 1.0;
    let mut mild = Belief::START;
    mild.wrathful = 0.0;

    let tiles: Vec<usize> = (0..world.population.len())
        .filter(|&tile| world.culture_id[tile] == culture && world.population[tile] >= 1.0)
        .collect();
    assert!(tiles.len() >= 2, "the culture holds at least two tiles");
    world.set_belief(tiles[0], holy);
    for &tile in &tiles[1..] {
        world.set_belief(tile, mild);
    }

    world.step(10); // aggregates refresh
    let belief = world.cultures()[culture as usize].belief;
    assert!(
        belief.wrathful > 0.0 && belief.wrathful < 1.0,
        "the culture reads the mean of its tiles, not one tile (got {})",
        belief.wrathful
    );
}

#[test]
fn a_doctrine_crystallizes_when_an_axis_passes_the_threshold() {
    let mut world = World::new(1);
    world.step(50);
    let culture = world.cultures()[0].id;
    assert_eq!(world.cultures()[0].doctrine, None);

    set_culture_belief(&mut world, culture, angry());
    world.step(60);

    assert_eq!(
        world.cultures()[culture as usize].doctrine,
        Some(Doctrine::Wrathful),
        "a people who saw only fire believe in a wrathful god"
    );
}

#[test]
fn doctrine_needs_two_consecutive_checks_before_it_crystallizes() {
    let mut world = World::new(1);
    world.step(50);
    let culture = world.cultures()[0].id;

    set_culture_belief(&mut world, culture, angry());
    world.step(25); // one check at 25-tick cadence
    assert_eq!(
        world.cultures()[culture as usize].doctrine,
        None,
        "one check is not enough (hysteresis)"
    );

    world.step(25);
    assert_eq!(
        world.cultures()[culture as usize].doctrine,
        Some(Doctrine::Wrathful)
    );
}

#[test]
fn a_new_doctrine_elevates_a_prophet_who_speaks_for_the_god() {
    let mut world = World::new(1);
    world.step(50);
    let culture = world.cultures()[0].id;

    // Smite them once so somebody is a witness to elevate.
    let eden = world.eden_tile().unwrap();
    world.queue_power(sim_core::powers::Power::Meteor, eden % 160, eden / 160);
    world.step(10);
    assert!(!world.witnesses().is_empty(), "the meteor made witnesses");

    set_culture_belief(&mut world, culture, angry());
    world.step(60);

    let prophets: Vec<_> = world
        .notables()
        .iter()
        .filter(|notable| notable.role == sim_core::notables::Role::Prophet)
        .collect();
    assert_eq!(prophets.len(), 1, "one prophet speaks for the god");
    assert_eq!(prophets[0].culture_id, culture);

    let verses = world.scripture_text(culture, 0, 4);
    assert!(
        verses.iter().any(|verse| verse.contains(&prophets[0].name)),
        "the prophet is named in scripture"
    );
}

#[test]
fn a_prophet_is_not_demoted_like_a_witness() {
    let mut world = World::new(1);
    world.step(50);
    let culture = world.cultures()[0].id;
    let eden = world.eden_tile().unwrap();
    world.queue_power(sim_core::powers::Power::Meteor, eden % 160, eden / 160);
    world.step(10);

    set_culture_belief(&mut world, culture, angry());
    world.step(60);
    let prophets = world
        .notables()
        .iter()
        .filter(|notable| notable.role == sim_core::notables::Role::Prophet)
        .count();
    assert_eq!(prophets, 1);

    world.step(60);
    let prophets = world
        .notables()
        .iter()
        .filter(|notable| notable.role == sim_core::notables::Role::Prophet)
        .count();
    assert_eq!(prophets, 1, "prophets keep their office");
}

#[test]
fn a_schism_founds_a_new_faith_when_a_people_drifts_from_its_founding_belief() {
    let mut world = World::new(1);
    world.step(300);
    assert!(!world.faiths().is_empty());
    let culture = world.cultures()[0].id;
    let faith_before = world.cultures()[culture as usize].faith_id;

    set_culture_belief(&mut world, culture, angry());
    world.step(60);

    assert!(world.faiths().len() >= 2, "a new faith is founded");
    assert_ne!(
        world.cultures()[culture as usize].faith_id,
        faith_before,
        "the people left the old faith"
    );
}

#[test]
fn peoples_who_diverge_together_share_one_new_faith() {
    let mut world = World::new(1);
    world.step(1_200);
    let ids = culture_ids(&world);
    assert!(ids.len() >= 2, "two peoples exist");
    let (a, b) = (ids[0], ids[1]);
    let faith_before: Vec<u16> = world
        .cultures()
        .iter()
        .map(|culture| culture.faith_id)
        .collect();

    set_culture_belief(&mut world, a, angry());
    set_culture_belief(&mut world, b, angry());
    world.step(60);

    let faith_a = world.cultures()[a as usize].faith_id;
    let faith_b = world.cultures()[b as usize].faith_id;
    assert_eq!(
        faith_a, faith_b,
        "they see the same god, so they share a faith"
    );
    assert!(
        faith_before.iter().all(|&faith| faith != faith_a),
        "the shared faith is new"
    );
}

#[test]
fn re_anointing_moves_favor_and_makes_the_old_people_jealous() {
    let mut world = World::new(1);
    world.step(1_200);
    let chosen = world.cultures()[0].id;
    let rival = world
        .cultures()
        .iter()
        .find(|culture| culture.favor == Favor::Other)
        .expect("a rival people")
        .id;
    let jealous_before = world.cultures()[chosen as usize].belief.jealous;
    let verses_before = world.scripture_text(chosen, 0, 64).len();

    world.queue_anoint(rival);
    world.step(1);

    assert_eq!(world.cultures()[rival as usize].favor, Favor::Chosen);
    assert_eq!(world.cultures()[chosen as usize].favor, Favor::Other);
    assert!(
        world.cultures()[chosen as usize].belief.jealous > jealous_before,
        "the abandoned people grow jealous"
    );
    assert!(
        world.scripture_text(chosen, 0, 64).len() > verses_before,
        "the abandoned people write it down"
    );
    let chosen_count = world
        .cultures()
        .iter()
        .filter(|culture| culture.favor == Favor::Chosen)
        .count();
    assert_eq!(chosen_count, 1, "exactly one Chosen at any time");
}

#[test]
fn piety_is_belief_strength_minus_absence() {
    let mut world = World::new(1);
    world.step(50);
    let culture = world.cultures()[0].id;

    let mut devout = Belief::START;
    devout.wrathful = 1.0;
    devout.absent = 0.0;
    set_culture_belief(&mut world, culture, devout);
    world.step(10);
    let high = world.cultures()[culture as usize].piety;

    let mut doubting = devout;
    doubting.absent = 1.0;
    set_culture_belief(&mut world, culture, doubting);
    world.step(10);
    let low = world.cultures()[culture as usize].piety;

    assert!(high > low, "doubt lowers piety ({high} vs {low})");
}

#[test]
fn a_wrathful_doctrine_demands_sacrifices() {
    let mut wrathful = World::new(1);
    let mut control = World::new(1);
    wrathful.step(200);
    control.step(200);
    let culture = wrathful.cultures()[0].id;
    let people = |world: &World| -> f32 {
        (0..world.population.len())
            .filter(|&tile| world.culture_id[tile] == culture)
            .map(|tile| world.population[tile])
            .sum()
    };

    set_culture_belief(&mut wrathful, culture, angry());
    wrathful.step(60);
    control.step(60);
    assert_eq!(
        wrathful.cultures()[culture as usize].doctrine,
        Some(Doctrine::Wrathful)
    );

    wrathful.step(20);
    control.step(20);
    assert!(
        people(&wrathful) < people(&control),
        "a wrathful god takes a share of the people ({} with the altar vs {} without)",
        people(&wrathful),
        people(&control)
    );
}

#[test]
fn the_same_seed_and_actions_produce_the_same_faiths() {
    let mut a = World::new(5);
    let mut b = World::new(5);
    for world in [&mut a, &mut b] {
        world.step(400);
        let eden = world.eden_tile().unwrap();
        world.queue_power(sim_core::powers::Power::Meteor, eden % 160, eden / 160);
        world.step(400);
    }

    assert_eq!(a.faiths().len(), b.faiths().len());
    let faiths_a: Vec<u64> = a.faiths().iter().map(|faith| faith.founded_tick).collect();
    let faiths_b: Vec<u64> = b.faiths().iter().map(|faith| faith.founded_tick).collect();
    assert_eq!(faiths_a, faiths_b);
    assert_eq!(
        a.cultures().iter().map(|c| c.doctrine).collect::<Vec<_>>(),
        b.cultures().iter().map(|c| c.doctrine).collect::<Vec<_>>()
    );
}
