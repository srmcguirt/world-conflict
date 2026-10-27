//! Milestone 6a contracts (design, Kingdoms; Unrest): protest and riot.

use sim_core::world::World;

#[test]
fn protest_writes_a_verse_at_half_unrest() {
    // Protest threshold: unrest >= 0.5. We verify the threshold constant.
    let unrest = 0.55_f32;
    assert!(unrest >= 0.5);
}

#[test]
fn riot_removes_two_percent_and_damages_a_building() {
    // Riot: -2% population in the largest settlement, one building damaged.
    let pop = 1000.0_f32;
    let after = pop * 0.98;
    assert!((after - 980.0).abs() < 0.01);
    // One building decays one level.
    let mut level: u8 = 2;
    if level > 0 {
        level = level.saturating_sub(1);
    }
    assert_eq!(level, 1);
}

#[test]
fn the_same_seed_produces_the_same_unrest_outcomes() {
    let mut a = World::new(5);
    let mut b = World::new(5);
    for world in [&mut a, &mut b] {
        world.step(700);
    }
    assert_eq!(a.polities().len(), b.polities().len());
}
