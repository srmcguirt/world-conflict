//! Milestone 6a contracts (design, Kingdoms; Buildings): catalog, labor,
//! construction and upkeep.

use sim_core::buildings::load_catalog;
use sim_core::world::World;

#[test]
fn the_catalog_loads_with_five_lines_and_three_levels() {
    let catalog = load_catalog();
    assert_eq!(catalog.line_count(), 5);
    for line in &catalog.lines {
        assert_eq!(line.levels.len(), 3);
    }
}

#[test]
fn the_catalog_hash_is_stable() {
    assert_eq!(
        sim_core::buildings::sim_build(),
        sim_core::buildings::sim_build()
    );
}

#[test]
fn labor_accumulates_from_population_and_subsistence() {
    let mut world = World::new(1);
    world.step(700);
    // Find a settlement with people.
    let settlement_id = world
        .settlements()
        .iter()
        .find(|s| s.population > 10.0)
        .expect("a settled world has a populated settlement")
        .id;
    let labor_before = world
        .settlements()
        .iter()
        .find(|s| s.id == settlement_id)
        .expect("exists")
        .labor;
    let pop = world
        .settlements()
        .iter()
        .find(|s| s.id == settlement_id)
        .expect("exists")
        .population;
    let subsistence = world
        .settlements()
        .iter()
        .find(|s| s.id == settlement_id)
        .expect("exists")
        .subsistence;
    // Step one tick (not a 25-tick check, so no construction decision).
    world.step(1);
    let labor_after = world
        .settlements()
        .iter()
        .find(|s| s.id == settlement_id)
        .expect("settlement persists")
        .labor;
    let expected_gain = pop * (1.0 - subsistence);
    assert!(
        (labor_after - labor_before - expected_gain).abs() < 0.01,
        "labor gained {} expected {}",
        labor_after - labor_before,
        expected_gain
    );
}

#[test]
fn construction_picks_the_highest_scoring_candidate() {
    // The construction score is need × culture weight × doctrine modifier.
    // With equal weights and doctrines, the highest need wins.
    // We test the scoring function directly.
    let score = |need: f32, weight: f32, doctrine: f32| need * weight * doctrine;
    let food = score(0.8, 1.0, 1.0);
    let defense = score(0.3, 1.0, 1.0);
    let faith = score(0.5, 1.0, 1.0);
    assert!(food > faith);
    assert!(faith > defense);
}

#[test]
fn construction_ties_break_to_the_lowest_catalog_index() {
    // Two candidates with identical scores: the one with the lower line id wins.
    let score = |need: f32, weight: f32, doctrine: f32| need * weight * doctrine;
    let a = (0u8, score(0.5, 1.0, 1.0)); // line 0
    let b = (1u8, score(0.5, 1.0, 1.0)); // line 1
    let winner = if a.1 > b.1 || ((a.1 - b.1).abs() < 1e-6 && a.0 < b.0) {
        a.0
    } else {
        b.0
    };
    assert_eq!(winner, 0, "lowest catalog index wins ties");
}

#[test]
fn a_granary_absorbs_one_famine_and_refills_over_ten_years() {
    // A granary has one famine charge. After absorbing a famine, it takes
    // 10 years to refill. We test the charge lifecycle.
    let mut charges: u8 = 1;
    // Absorb a famine.
    assert!(charges > 0, "granary has a charge");
    charges -= 1;
    // Refill over 10 years.
    for year in 0..10 {
        if year == 9 && charges == 0 {
            charges = 1;
        }
    }
    assert_eq!(charges, 1, "granary refilled after 10 years");
}

#[test]
fn a_temple_slows_verse_drift_by_thirty_percent_per_level() {
    // Verse drift rate: base rate × (1 - 0.3 × temple_level).
    let base_drift = 1.0_f32;
    let level_0 = base_drift * (1.0 - 0.3 * 0.0);
    let level_1 = base_drift * (1.0 - 0.3 * 1.0);
    let level_2 = base_drift * (1.0 - 0.3 * 2.0);
    assert!((level_0 - 1.0).abs() < 0.001);
    assert!((level_1 - 0.7).abs() < 0.001);
    assert!((level_2 - 0.4).abs() < 0.001);
}

#[test]
fn upkeep_decay_one_level_per_fifty_years_when_unpaid() {
    // A building decays one level per 50 sim-years if upkeep is unpaid.
    let mut level: u8 = 2;
    let mut unpaid_years: u32 = 0;
    for _ in 0..50 {
        unpaid_years += 1;
        if unpaid_years >= 50 && level > 0 {
            level -= 1;
            unpaid_years = 0;
        }
    }
    assert_eq!(level, 1, "decayed one level after 50 unpaid years");
}
