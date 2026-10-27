//! Milestone 6a contracts (design, Kingdoms; Polities): founding, territory
//! and cohesion.

use sim_core::world::World;

#[test]
fn a_polity_is_founded_by_a_tier_one_settlement() {
    let mut world = World::new(1);
    world.step(2_000);
    assert!(!world.polities().is_empty(), "a settled world has polities");
    // Every polity has a capital settlement that exists.
    for polity in world.polities() {
        let capital = world.settlements().iter().find(|s| s.id == polity.capital);
        assert!(capital.is_some(), "polity {} has a capital", polity.id);
    }
}

#[test]
fn settlements_within_reach_join_their_polity() {
    let mut world = World::new(1);
    world.step(2_000);
    // Every settlement with a polity nearby belongs to one.
    for settlement in world.settlements() {
        if world.polity_of_settlement(settlement.id).is_some() {
            // The settlement's polity exists.
            assert!(
                world
                    .polities()
                    .iter()
                    .any(|p| p.id == world.polity_of_settlement(settlement.id).unwrap())
            );
        }
    }
    // At least one polity has more than one settlement.
    assert!(
        world.polities().iter().any(|p| p.settlements.len() > 1),
        "polities group settlements"
    );
}

#[test]
fn territory_follows_the_nearest_settlement() {
    let mut world = World::new(1);
    world.step(2_000);
    // Every tile assigned to a polity is settled.
    for i in 0..world.population.len() {
        if world.tile_polity(i) != u16::MAX {
            assert!(world.population[i] >= 1.0, "polity tile {i} is settled");
        }
    }
    // At least one polity has territory.
    assert!(
        world.polities().iter().any(|p| {
            (0..world.population.len())
                .filter(|&i| world.tile_polity(i) == p.id as u16)
                .count()
                > 0
        }),
        "polities hold territory"
    );
}

#[test]
fn cohesion_falls_with_belief_spread() {
    let mut world = World::new(1);
    world.step(2_000);
    for polity in world.polities() {
        assert!(
            polity.cohesion >= 0.0 && polity.cohesion <= 1.0,
            "cohesion is in [0, 1]"
        );
    }
}

#[test]
fn the_same_seed_founds_the_same_polities() {
    let mut a = World::new(5);
    let mut b = World::new(5);
    for world in [&mut a, &mut b] {
        world.step(700);
    }
    assert_eq!(a.polities().len(), b.polities().len());
    let ids_a: Vec<(u32, u16)> = a.polities().iter().map(|p| (p.id, p.culture_id)).collect();
    let ids_b: Vec<(u32, u16)> = b.polities().iter().map(|p| (p.id, p.culture_id)).collect();
    assert_eq!(ids_a, ids_b, "same seed, same polities");
}
