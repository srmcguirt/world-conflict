//! The generated founding (design, Out of the Garden step 1; D18, D19, D4).
//!
//! The seed decides the world, and the world decides the founding, so the
//! Garden needs no RNG of its own: it sits on the best land, and nobody stays.
//! Of its neighbors, Dirt takes the most arable and offers its harvest; the
//! snake's line takes the high ground, offers the work of its hands, and
//! starts turned from the god and toward force. Ties break on the lower tile
//! index.

use crate::culture::{self, Culture, Favor, LINES, Line};
use crate::hex;
use crate::language::Language;
use crate::scripture::{self, Harvest, Offering, VerseKind};
use crate::settlements;
use crate::world::{GRID_WIDTH, World};
use crate::worldgen::{self, Biome};

/// People in each line when the world starts: enough to found a camp (D7).
pub const LINE_POPULATION: f32 = 8.0;

/// How far the snake's line starts from Dirt: toward an absent god, and from
/// devotion toward force. The author's 50/50 lean (design, Garden Open
/// Question 10); the Stage 1 sweep tunes it so the line hardens on about
/// half of seeds (R10).
pub const SNAKE_LEAN: f32 = 0.2;

/// Salt for the snake's line's name seed, so it gets its own name and sound.
const SNAKE_SEED_SALT: u64 = 0x5_4E4B_0E00;

/// What the Garden is called in both lines' founding verses.
const GARDEN_PLACE: &str = "the Garden";

/// Plants the two lines beside the Garden and writes both scriptures' opening.
pub fn plant(world: &mut World) {
    let Some(garden) = best_land(world) else {
        return;
    };
    world.garden = Some(garden as u32);
    let Some(homes) = homes(world, garden) else {
        return;
    };
    for line in [Line::Dirt, Line::Snake] {
        found_line(world, line, homes[line as usize]);
    }
    // Each line's first camp: its band of eight founds it where it stands
    // (D7), so its people spread only inside that land.
    for line in [Line::Dirt, Line::Snake] {
        let tile = homes[line as usize];
        let camp = settlements::claim(world, tile, world.culture_id[tile]);
        world.settlements[camp].population = LINE_POPULATION;
    }
    write_founding(world, garden, homes);
}

/// The settleable tile with the most carrying capacity.
fn best_land(world: &World) -> Option<usize> {
    let mut best = None;
    let mut most = -1.0_f32;
    for i in 0..world.biome.len() {
        if worldgen::settleable(world.biome[i]) && world.carrying_capacity[i] > most {
            most = world.carrying_capacity[i];
            best = Some(i);
        }
    }
    best
}

/// Where each line settles: Dirt on the most arable neighbor, the snake's line
/// on the highest of the rest, or the next-best land when none rises above
/// the Garden (D18). Looks one ring out, wider only when the Garden has fewer
/// than two settleable neighbors.
fn homes(world: &World, garden: usize) -> Option<[usize; LINES]> {
    let (gx, gy) = (garden % GRID_WIDTH, garden / GRID_WIDTH);
    let candidates = (1..=3).find_map(|radius| {
        let mut tiles: Vec<usize> = hex::disc(gx, gy, radius)
            .into_iter()
            .map(|(x, y)| y * GRID_WIDTH + x)
            .filter(|&i| i != garden && worldgen::settleable(world.biome[i]))
            .collect();
        tiles.sort_unstable();
        (tiles.len() >= 2).then_some(tiles)
    })?;
    let dirt = arg_max(&candidates, |i| world.carrying_capacity[i])?;
    let rest: Vec<usize> = candidates.into_iter().filter(|&i| i != dirt).collect();
    let rises = rest
        .iter()
        .any(|&i| world.elevation[i] > world.elevation[garden]);
    let snake = if rises {
        arg_max(&rest, |i| world.elevation[i])
    } else {
        arg_max(&rest, |i| world.carrying_capacity[i])
    }?;
    Some([dirt, snake])
}

/// The tile with the largest `key`, the lower index on ties (`tiles` sorted).
fn arg_max(tiles: &[usize], key: impl Fn(usize) -> f32) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for &tile in tiles {
        let value = key(tile);
        if best.is_none_or(|(_, top)| value > top) {
            best = Some((tile, value));
        }
    }
    best.map(|(tile, _)| tile)
}

/// Founds one line on its home tile, with its own name, tongue and lean.
fn found_line(world: &mut World, line: Line, tile: usize) {
    let id = world.cultures.len() as u16;
    let mut centroid = culture::biome_target(world.biome[tile]);
    let mut belief = world.belief_of(tile);
    let (name_seed, favor) = match line {
        Line::Dirt => (world.seed(), Favor::Chosen),
        Line::Snake => {
            centroid[culture::MARTIAL] = (centroid[culture::MARTIAL] + SNAKE_LEAN).min(1.0);
            centroid[culture::DEVOUT] = (centroid[culture::DEVOUT] - SNAKE_LEAN).max(0.0);
            belief.absent = (belief.absent + SNAKE_LEAN).min(1.0);
            (world.seed() ^ SNAKE_SEED_SALT, Favor::Other)
        }
    };
    world.population[tile] = LINE_POPULATION;
    world.culture_id[tile] = id;
    world.culture_vec[tile * culture::DIMS..(tile + 1) * culture::DIMS].copy_from_slice(&centroid);
    world.set_belief(tile, belief);

    let mut people = Culture::found(id, line, name_seed, centroid, favor, LINE_POPULATION, 1);
    people.belief = belief;
    let mut tongue = Language::root(id, name_seed, people.phonemes.clone());
    tongue.id = world.languages.len() as u16;
    people.language_id = tongue.id;
    world.languages.push(tongue);
    world.cultures.push(people);
    world.split_watch.push(0);
    world.line_homes[line as usize] = Some(tile as u32);
}

/// Opens both scriptures: the beginning at the Garden, the line's own
/// offering at its home, and the parting as that line tells it (D18, D19).
fn write_founding(world: &mut World, garden: usize, homes: [usize; LINES]) {
    let names = [
        world.cultures[0].name.clone(),
        world.cultures[1].name.clone(),
    ];
    for line in [Line::Dirt, Line::Snake] {
        let me = line as usize;
        let home = homes[me];
        let offering = match line {
            Line::Dirt => Offering::Harvest(harvest(world.biome[home])),
            Line::Snake => Offering::Handiwork,
        };
        let place = crate::names::place_name(&world.cultures[me], home);
        let verses = [
            scripture::founding_verse(
                VerseKind::Genesis,
                &names[me],
                "",
                GARDEN_PLACE,
                garden as u32,
            ),
            scripture::founding_verse(
                VerseKind::Offering(offering),
                &names[me],
                "",
                &place,
                home as u32,
            ),
            scripture::founding_verse(
                VerseKind::Parting {
                    high_ground: line == Line::Snake,
                },
                &names[me],
                &names[1 - me],
                GARDEN_PLACE,
                garden as u32,
            ),
        ];
        world.cultures[me].scripture.extend(verses);
    }
}

/// The first of the harvest, by the land Dirt worked.
fn harvest(biome: Biome) -> Harvest {
    match biome {
        Biome::Forest => Harvest::Fruit,
        Biome::Beach => Harvest::Fish,
        _ => Harvest::Grain,
    }
}
