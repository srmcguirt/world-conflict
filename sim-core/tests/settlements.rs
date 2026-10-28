//! Milestone 6a contracts (design, Kingdoms; Settlements): claimed catchments,
//! percentile tiers and their hysteresis.

use sim_core::world::World;

#[test]
fn settlements_claim_tiles_and_do_not_overlap() {
    let mut world = World::new(1);
    world.step(700);
    assert!(
        !world.settlements().is_empty(),
        "the densest ground has raised a camp"
    );

    // Every claimed tile belongs to exactly one settlement.
    let mut seen = vec![0u32; world.population.len()];
    for settlement in world.settlements() {
        for tile in &settlement.catchment {
            seen[*tile] += 1;
        }
    }
    assert!(
        seen.iter().all(|&count| count <= 1),
        "no tile is claimed twice"
    );
    assert!(seen.contains(&1), "at least one tile is claimed");
}

#[test]
#[expect(
    clippy::bool_to_int_with_if,
    reason = "four-way tier ladder from percentile cuts"
)]
fn tiers_are_percentiles_of_the_claimed_catchments() {
    let mut world = World::new(1);
    world.step(2_000);
    let settlements = world.settlements();
    assert!(
        settlements.len() >= 10,
        "a settled world has many settlements"
    );

    // Sort catchment populations descending; the top 1% are cities, the next
    // 4% towns, the next 20% villages, the rest camps.
    // A settlement whose land was released claims nothing and is not ranked
    // (D16).
    let living: Vec<_> = settlements
        .iter()
        .filter(|s| !s.catchment.is_empty())
        .collect();
    let mut pops: Vec<f32> = living.iter().map(|s| s.population).collect();
    pops.sort_by(|a, b| b.total_cmp(a));
    let n = pops.len();
    let city_cut = pops[((n as f32 * 0.01).ceil() as usize).clamp(1, n) - 1];
    let town_cut = pops[((n as f32 * 0.05).ceil() as usize).clamp(1, n) - 1];
    let village_cut = pops[((n as f32 * 0.25).ceil() as usize).clamp(1, n) - 1];

    let cities = living.iter().filter(|s| s.tier == 3).count();
    let towns = living.iter().filter(|s| s.tier == 2).count();
    let villages = living.iter().filter(|s| s.tier == 1).count();
    let camps = living.iter().filter(|s| s.tier == 0).count();

    // Cities are rare: roughly 1% of settlements.
    assert!(
        cities <= (n / 50).max(1) + 2,
        "cities are rare, got {cities} of {n}"
    );
    assert!(camps > villages, "camps outnumber villages");
    assert!(camps > 0, "there are camps");
    // Every settlement's tier matches its population against the cuts.
    for s in &living {
        let expected: u8 = if s.population >= city_cut {
            3
        } else if s.population >= town_cut {
            2
        } else if s.population >= village_cut {
            1
        } else {
            0
        };
        // A settlement keeps a higher tier through one check below it before
        // it drops (hysteresis, design: two checks); with geology moving
        // people off sinking land, some are always mid-decline (D27).
        let waiting_to_drop = s.tier > expected && s.tier_checks_below == 1;
        assert!(
            s.tier == expected || waiting_to_drop,
            "settlement {} population {} tier {} expected {}",
            s.id,
            s.population,
            s.tier,
            expected
        );
    }
    let _ = (towns, villages);
}

#[test]
fn the_same_seed_founds_the_same_settlements() {
    let mut a = World::new(5);
    let mut b = World::new(5);
    for world in [&mut a, &mut b] {
        world.step(700);
    }
    assert_eq!(a.settlements().len(), b.settlements().len());
    let ids_a: Vec<(u32, usize, u8)> = a
        .settlements()
        .iter()
        .map(|s| (s.id, s.tile, s.tier))
        .collect();
    let ids_b: Vec<(u32, usize, u8)> = b
        .settlements()
        .iter()
        .map(|s| (s.id, s.tile, s.tier))
        .collect();
    assert_eq!(ids_a, ids_b, "same seed, same settlements");
}

#[test]
fn a_settlement_demotes_after_two_checks_below_its_tier() {
    let mut world = World::new(1);
    world.step(2_000);
    let before: Vec<(u32, u8)> = world.settlements().iter().map(|s| (s.id, s.tier)).collect();
    assert!(!before.is_empty());

    // Starve the world: zero population on every tile for two checks.
    world.population.fill(0.0);
    world.step(25);
    world.step(25);

    let after: Vec<(u32, u8)> = world.settlements().iter().map(|s| (s.id, s.tier)).collect();
    assert!(
        after.iter().any(|(_, tier)| *tier < 3),
        "at least one tier has dropped after two starved checks"
    );
}

#[test]
fn settlements_only_form_on_settled_ground() {
    let mut world = World::new(1);
    world.step(700);
    let settlements = world.settlements();
    assert!(!settlements.is_empty());
    // A settlement whose land the sea took keeps its record after it
    // releases the land (D16, D27); only living ones are checked.
    for s in settlements.iter().filter(|s| !s.catchment.is_empty()) {
        assert!(
            world.population[s.tile] >= 1.0,
            "settlement {} sits on a settled tile",
            s.id
        );
    }
}
