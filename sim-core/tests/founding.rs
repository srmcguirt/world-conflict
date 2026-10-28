//! The generated founding (design, Out of the Garden step 1; D18, D19, D4).
//!
//! The Garden sits on the world's best land and stays empty: of its
//! neighbors, Dirt takes the most arable and the snake's line the highest,
//! each offering what it works, and both scriptures open with the parting.

use sim_core::culture::{self, Favor, Line};
use sim_core::scripture::{Offering, VerseKind};
use sim_core::world::{GRID_WIDTH, World};
use sim_core::{hex, save, worldgen};

const SEEDS: [u64; 8] = [1, 2, 3, 5, 8, 13, 21, 34];

fn home(world: &World, line: Line) -> usize {
    world.line_home(line).expect("both lines are planted")
}

fn settleable_neighbors(world: &World, tile: usize) -> Vec<usize> {
    hex::neighbors(tile % GRID_WIDTH, tile / GRID_WIDTH)
        .map(|(x, y)| y * GRID_WIDTH + x)
        .filter(|&i| worldgen::settleable(world.biome[i]))
        .collect()
}

#[test]
fn the_garden_stays_empty_and_two_lines_are_planted_beside_it() {
    for seed in SEEDS {
        let world = World::new(seed);
        let garden = world.garden_tile().expect("the Garden exists");
        let settled: Vec<usize> = (0..world.population.len())
            .filter(|&i| world.population[i] >= 1.0)
            .collect();
        assert_eq!(settled.len(), 2, "seed {seed}: one tile per line");
        assert!(
            !settled.contains(&garden),
            "seed {seed}: nobody stays in the Garden"
        );
        for line in [Line::Dirt, Line::Snake] {
            let tile = home(&world, line);
            assert!(settled.contains(&tile));
            assert!(
                hex::distance(
                    tile % GRID_WIDTH,
                    tile / GRID_WIDTH,
                    garden % GRID_WIDTH,
                    garden / GRID_WIDTH
                ) <= 2,
                "seed {seed}: {line:?} is planted beside the Garden"
            );
        }

        let cultures = world.cultures();
        assert_eq!(cultures.len(), 2);
        assert_eq!(cultures[0].line, Line::Dirt);
        assert_eq!(cultures[1].line, Line::Snake);
        assert_eq!(cultures[0].favor, Favor::Chosen, "Dirt starts Chosen");
        assert_eq!(cultures[1].favor, Favor::Other);
        assert_ne!(cultures[0].name, cultures[1].name);
        assert_ne!(cultures[0].language_id, cultures[1].language_id);
    }
}

#[test]
fn dirt_takes_the_most_arable_neighbor_and_the_snake_the_high_ground() {
    for seed in SEEDS {
        let world = World::new(seed);
        let garden = world.garden_tile().expect("the Garden exists");
        let neighbors = settleable_neighbors(&world, garden);
        if neighbors.len() < 2 {
            continue;
        }
        let dirt = home(&world, Line::Dirt);
        let snake = home(&world, Line::Snake);
        for &n in &neighbors {
            assert!(
                world.carrying_capacity[dirt] >= world.carrying_capacity[n],
                "seed {seed}: Dirt takes the most arable neighbor"
            );
        }
        let rest: Vec<usize> = neighbors.iter().copied().filter(|&n| n != dirt).collect();
        let high_ground = rest
            .iter()
            .any(|&n| world.elevation[n] > world.elevation[garden]);
        for &n in &rest {
            if high_ground {
                assert!(
                    world.elevation[snake] >= world.elevation[n],
                    "seed {seed}: the snake's line takes the highest neighbor"
                );
            } else {
                assert!(
                    world.carrying_capacity[snake] >= world.carrying_capacity[n],
                    "seed {seed}: on flat land the snake's line takes the next-best land"
                );
            }
        }
    }
}

#[test]
fn both_scriptures_open_with_the_offerings_and_the_parting() {
    let world = World::new(1);
    let cultures = world.cultures();
    for (culture, other) in [(&cultures[0], &cultures[1]), (&cultures[1], &cultures[0])] {
        let kinds: Vec<&VerseKind> = culture.scripture.iter().map(|v| &v.kind).collect();
        assert!(matches!(kinds[0], VerseKind::Genesis));
        assert!(matches!(kinds[1], VerseKind::Offering(_)));
        assert!(matches!(kinds[2], VerseKind::Parting { .. }));
        assert!(culture.scripture[..3].iter().all(|v| v.tick == 0));

        let parting = culture.scripture[2].render();
        assert!(
            parting.contains("Garden"),
            "the parting is told at the Garden: {parting}"
        );
        assert!(
            parting.contains(&other.name),
            "each parting names the other line: {parting}"
        );
    }
    assert!(matches!(
        cultures[0].scripture[1].kind,
        VerseKind::Offering(Offering::Harvest(_))
    ));
    assert_eq!(
        cultures[1].scripture[1].kind,
        VerseKind::Offering(Offering::Handiwork)
    );
    assert_ne!(
        cultures[0].scripture[1].render(),
        cultures[1].scripture[1].render()
    );
}

#[test]
#[expect(
    clippy::float_cmp,
    reason = "Dirt's vector is copied from the biome target unchanged, so equality is exact"
)]
fn the_snakes_line_starts_turned_from_god_and_toward_force() {
    for seed in SEEDS {
        let world = World::new(seed);
        let dirt = home(&world, Line::Dirt);
        let snake = home(&world, Line::Snake);
        assert!(
            world.belief_of(snake).absent > world.belief_of(dirt).absent,
            "seed {seed}: the snake's line leans toward an absent god"
        );
        let target = culture::biome_target(world.biome[snake]);
        let vec = world.culture_vec_of(snake);
        assert!(
            vec[culture::MARTIAL] - vec[culture::DEVOUT]
                > target[culture::MARTIAL] - target[culture::DEVOUT],
            "seed {seed}: the snake's line leans toward force over devotion"
        );
        let dirt_target = culture::biome_target(world.biome[dirt]);
        assert_eq!(
            world.culture_vec_of(dirt),
            dirt_target,
            "Dirt carries no lean"
        );
    }
}

#[test]
fn a_daughter_people_keeps_its_parents_line() {
    let mut world = World::new(1);
    world.step(2_000);
    let cultures = world.cultures();
    let daughters: Vec<_> = cultures.iter().filter(|c| c.parent_id.is_some()).collect();
    assert!(
        !daughters.is_empty(),
        "2,000 years produce daughter peoples"
    );
    for daughter in daughters {
        let parent = &cultures[daughter.parent_id.expect("filtered") as usize];
        assert_eq!(
            daughter.line, parent.line,
            "{} keeps its line",
            daughter.name
        );
    }
    assert_eq!(
        cultures.iter().filter(|c| c.favor == Favor::Chosen).count(),
        1,
        "exactly one people is Chosen"
    );
}

#[test]
fn the_line_survives_a_save_and_load() {
    let mut world = World::new(3);
    world.step(300);
    let (_, loaded) = save::load(&save::save(&world)).expect("a fresh save loads");
    let lines = |w: &World| w.cultures().iter().map(|c| c.line).collect::<Vec<_>>();
    assert_eq!(lines(&loaded), lines(&world));
    assert_eq!(loaded.garden_tile(), world.garden_tile());
    assert_eq!(loaded.line_home(Line::Snake), world.line_home(Line::Snake));
}
