//! Walking bands (design, Out of the Garden step 2; eng review D7, D8, D10,
//! D16, D20, D21, D22).
//!
//! Outside claimed land people move only as bands. A crowded settlement buds a
//! band; the band plans its route once over the land's real crossing cost, so
//! it follows coasts and grassland, skirts deserts and ranges and crosses
//! narrow straits but never open ocean, and walks it one step a year. A band of [`SETTLE_MIN`] or more founds a tier-0 camp on
//! unclaimed land, and from then on its people spread by tile migration inside
//! that camp's catchment. A band carries its own headcount (D8): it counts
//! toward its people without holding tile population.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::belief;
use crate::culture::{self, Favor, Line, NO_CULTURE};
use crate::hex;
use crate::settlements;
use crate::traits;
use crate::world::{GRID_WIDTH, World};
use crate::worldgen::{self, Biome};

/// People in a bud from a crowded settlement (D20).
pub const BUD_SIZE: f32 = 25.0;

/// A band over this size splits in two (D20).
pub const SPLIT_ABOVE: f32 = 50.0;

/// Least people a band needs to found a camp (D7).
pub const SETTLE_MIN: f32 = settlements::CAMP_THRESHOLD;

/// Most bands walking at once; crowded settlements wait their turn (D10).
pub const MAX_WALKING: usize = 64;

/// Years from a band's forming until it gives up on land and heads to its kin.
/// The clock never resets, whether the band re-plans or parts (D10, D16).
pub const STRANDED_YEARS: u64 = 100;

/// Water hexes that part a band from its people: a strait. Tuned in the sweep.
const WADE_TO_PART: u8 = 3;

/// Mountain hexes that part a band from its people: a range, not a foothill.
/// Tuned in the sweep.
const CLIMB_TO_PART: u8 = 4;

/// Crossing one hex of coastal water, an ocean hex that touches land: dearer
/// than a mountain, so a band takes a strait only when the land way is long
/// (D22). Open ocean stays impassable.
const COASTAL_WATER: f32 = 5.0;

/// Years between a band's quarrel rolls (D20).
const QUARREL_EVERY: u64 = 25;

/// Chance that a band of [`SPLIT_ABOVE`] with no communal bond quarrels at a
/// roll; smaller and closer bands quarrel less (D20). Tuned in the sweep.
const QUARREL_CHANCE: f32 = 0.3;

/// Share of its catchment's capacity a settlement fills before it buds,
/// indexed by `Line`. Dirt sends bands out readily; the snake's line holds on
/// to its walls and buds fewer (D13). Tuned in the sweep.
const CROWDED: [f32; culture::LINES] = [0.8, 0.95];

/// How far a band looks for land, in crossing cost (one grassland step is 1).
const SEARCH_COST: f32 = 30.0;

/// How much the snake's line favors high ground when it picks land: an
/// elevation of 1 triples a site's worth to it (D18, "heads for high ground").
const HIGH_GROUND: f32 = 2.0;

/// Founder effect: a band that leaves is a slightly mutated copy of its
/// people (design, Milestone 1 founder mutation, carried by the walkers now).
const FOUNDER_MUTATION: f32 = 0.05;

/// Walkers grow at a quarter of the settled rate: a bud of 25 takes about a
/// generation on the road to reach the split size. Tuned in the sweep.
const WALKING_GROWTH: f32 = crate::demography::GROWTH_RATE / 4.0;

/// Room beyond this many times its own size does not draw a band further.
const ROOM_SEEN: f32 = 4.0;

/// Fixed-point scale for crossing costs, so the route search orders exactly
/// and breaks ties on the lower tile index (D10).
const COST_SCALE: f32 = 1_000.0;

/// Where a band is headed.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Goal {
    /// Unclaimed land to found a camp on.
    Land(u32),
    /// A settlement of its own people or line to join (D16).
    Kin(u32),
    /// No reachable land; searched again at the next 25-year pass.
    Stranded,
}

/// A people on the move: a headcount, a place and a planned route.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Band {
    /// Monotonic, never reused, so a queued act cannot land on the wrong band.
    pub id: u32,
    pub culture_id: u16,
    pub tile: u32,
    pub headcount: f32,
    pub formed_tick: u64,
    pub goal: Goal,
    /// Hexes of water and of mountain the band has crossed since it formed:
    /// enough of either parts it from its people (D24).
    pub waded: u8,
    pub climbed: u8,
    /// Steps still to walk, the last first, so the next step pops off the end.
    route: Vec<u32>,
    /// The walkers' own culture, belief and bodies, stamped onto their camp.
    pub culture_vec: [f32; culture::DIMS],
    pub belief: [f32; belief::AXES],
    pub traits: [f32; traits::TRAITS],
}

impl Band {
    /// Whether the band has crossed a strait or a range: terrain parts it from
    /// its people, and its camp founds a daughter people (D24).
    pub fn crossed(&self) -> bool {
        self.waded >= WADE_TO_PART || self.climbed >= CLIMB_TO_PART
    }
}

/// Walks, grows, splits and buds bands for one sim-year.
pub fn tick_bands(world: &mut World) {
    let mut index = 0;
    while index < world.bands.len() {
        if walk(world, index) {
            world.bands.remove(index);
        } else {
            index += 1;
        }
    }
    split_large(world);
    if world.tick().is_multiple_of(QUARREL_EVERY) {
        quarrel(world);
        for index in 0..world.bands.len() {
            if world.bands[index].goal == Goal::Stranded {
                plan_land(world, index);
            }
        }
    }
    bud(world);
}

/// One year of one band's walk. True when the band has ended: settled,
/// joined its kin or been taken in.
fn walk(world: &mut World, index: usize) -> bool {
    grow(world, index);
    let band = &world.bands[index];
    let age = world.tick() - band.formed_tick;
    if age >= STRANDED_YEARS && !matches!(band.goal, Goal::Kin(_)) && head_home(world, index) {
        return true;
    }
    let band = &mut world.bands[index];
    if let Some(next) = band.route.pop() {
        band.tile = next;
        match world.biome[next as usize] {
            Biome::Ocean => band.waded = band.waded.saturating_add(1),
            Biome::Mountain => band.climbed = band.climbed.saturating_add(1),
            _ => {}
        }
    }
    if !band.route.is_empty() {
        return false;
    }
    arrive(world, index)
}

/// What a band does at the end of its route.
fn arrive(world: &mut World, index: usize) -> bool {
    let band = &world.bands[index];
    match band.goal {
        Goal::Land(goal) => {
            let here = band.tile as usize;
            if band.tile == goal && fits(world, here, band.headcount) {
                settle(world, index);
                return true;
            }
            // Someone claimed it, or the land around it, first: look again, on
            // the same clock.
            plan_land(world, index);
            false
        }
        Goal::Kin(settlement) => {
            if world.settlements[settlement as usize].catchment.is_empty() {
                return head_home(world, index);
            }
            join(world, index, settlement as usize);
            true
        }
        Goal::Stranded => false,
    }
}

/// Walkers grow toward what their tile and its neighbors can feed, and never
/// starve on the road: the road is how a bud of 25 becomes a band that splits.
/// While every walking slot is taken a band cannot split, so it stops at the
/// split size rather than outgrow every site it could settle.
fn grow(world: &mut World, index: usize) {
    let tile = world.bands[index].tile as usize;
    let (x, y) = (tile % GRID_WIDTH, tile / GRID_WIDTH);
    let mut forage = world.carrying_capacity[tile]
        + hex::neighbors(x, y)
            .map(|(nx, ny)| world.carrying_capacity[ny * GRID_WIDTH + nx])
            .sum::<f32>();
    if world.bands.len() >= MAX_WALKING {
        forage = forage.min(SPLIT_ABOVE);
    }
    let band = &mut world.bands[index];
    if band.headcount < forage {
        band.headcount += WALKING_GROWTH * band.headcount * (1.0 - band.headcount / forage);
    }
}

/// A band past its years heads to the nearest settlement of its own people,
/// else of its line; if none is reachable it is taken in by the people whose
/// land it stands on (D16). True when it ended here and now.
fn head_home(world: &mut World, index: usize) -> bool {
    let from = world.bands[index].tile as usize;
    let reach = search(world, from, u32::MAX);
    if let Some(settlement) = nearest_kin(world, &reach, world.bands[index].culture_id) {
        let route = reach.route_to(world.settlements[settlement].tile);
        if route.is_empty() {
            join(world, index, settlement);
            return true;
        }
        let band = &mut world.bands[index];
        band.goal = Goal::Kin(settlement as u32);
        band.route = route;
        return false;
    }
    let owner = world.settlement_of[from];
    if owner != u32::MAX {
        join(world, index, owner as usize);
    } else if fits(world, from, world.bands[index].headcount) {
        settle(world, index);
    }
    // Otherwise nobody can reach it and nothing will grow where it stands:
    // the band scatters.
    true
}

/// The reachable live settlement nearest by crossing cost: the band's own
/// people first, then its line; the lower id on ties.
fn nearest_kin(world: &World, reach: &Reach, people: u16) -> Option<usize> {
    let line = world.cultures[people as usize].line;
    let nearest = |kin: &dyn Fn(u16) -> bool| {
        world
            .settlements
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.catchment.is_empty() && reach.reached(s.tile) && kin(s.culture_id))
            .min_by_key(|(id, s)| (reach.cost[s.tile], *id))
            .map(|(id, _)| id)
    };
    nearest(&|owner| owner == people).or_else(|| {
        nearest(&|owner| owner != NO_CULTURE && world.cultures[owner as usize].line == line)
    })
}

/// Picks the band's land and plans its route there, or marks it stranded.
fn plan_land(world: &mut World, index: usize) {
    let band = &world.bands[index];
    let reach = search(world, band.tile as usize, (SEARCH_COST * COST_SCALE) as u32);
    let line = world.cultures[band.culture_id as usize].line;
    let found = best_land(world, &reach, line, band.headcount);
    let band = &mut world.bands[index];
    if let Some(goal) = found {
        band.goal = Goal::Land(goal as u32);
        band.route = reach.route_to(goal);
    } else {
        band.goal = Goal::Stranded;
        band.route.clear();
    }
}

/// Whether a band of `headcount` can found a camp on `tile`: open land, enough
/// people, and room around it for all of them (D7).
fn fits(world: &World, tile: usize, headcount: f32) -> bool {
    open(world, tile) && headcount >= SETTLE_MIN && room_around(world, tile) >= headcount
}

/// Unclaimed, settleable land.
fn open(world: &World, tile: usize) -> bool {
    worldgen::settleable(world.biome[tile]) && world.settlement_of[tile] == u32::MAX
}

/// The reachable open tile whose would-be catchment can hold `headcount`
/// and suits the line best, discounted by the walk: Dirt looks for arable
/// land, the snake's line for high ground (design, step 2; D18). The walk
/// weighs heavily, so a band takes the near land that fits before the far
/// land that is richer, and goes far only as near land runs out (D20).
fn best_land(world: &World, reach: &Reach, line: Line, headcount: f32) -> Option<usize> {
    let mut best: Option<(f32, usize)> = None;
    for tile in 0..world.biome.len() {
        if !reach.reached(tile) || !fits(world, tile, headcount) {
            continue;
        }
        let room = room_around(world, tile);
        let lean = match line {
            Line::Dirt => 1.0,
            Line::Snake => 1.0 + HIGH_GROUND * world.elevation[tile],
        };
        let walk = reach.cost[tile] as f32 / COST_SCALE;
        // More room than a few bands' worth adds nothing a band can see.
        let score = room.min(ROOM_SEEN * headcount) * lean / (1.0 + walk);
        if best.is_none_or(|(top, _)| score > top) {
            best = Some((score, tile));
        }
    }
    best.map(|(_, tile)| tile)
}

/// What a camp founded on `tile` could feed: the open land it would claim.
fn room_around(world: &World, tile: usize) -> f32 {
    let (x, y) = (tile % GRID_WIDTH, tile / GRID_WIDTH);
    hex::disc(x, y, settlements::CATCHMENT_RADIUS)
        .into_iter()
        .map(|(cx, cy)| cy * GRID_WIDTH + cx)
        .filter(|&t| open(world, t))
        .map(|t| world.carrying_capacity[t])
        .sum()
}

/// Founds a tier-0 camp where the band stands (D7). A camp founds a daughter
/// people when its walkers crossed water or a mountain range to reach it
/// (D24), or drifted past the split distance from their people (D21);
/// distance alone does not.
fn settle(world: &mut World, index: usize) {
    let band = world.bands[index].clone();
    let tile = band.tile as usize;
    let people = camp_people(world, &band);
    let settlement = settlements::claim(world, tile, people);
    let placed = place(world, settlement, &band, people);
    world.settlements[settlement].population = placed;
}

/// Whose camp this is: the band's people, or a daughter of it.
fn camp_people(world: &mut World, band: &Band) -> u16 {
    let parent = &world.cultures[band.culture_id as usize];
    let drifted =
        culture::distance(&band.culture_vec, &parent.centroid) > culture::SPLIT_MIN_DISTANCE;
    if !band.crossed() && !drifted {
        return band.culture_id;
    }
    culture::push_daughter(
        world,
        band.culture_id,
        band.culture_vec,
        Favor::Other,
        band.headcount,
        1,
    )
    .unwrap_or(band.culture_id)
}

/// Joins a band to a settlement's people (D16).
fn join(world: &mut World, index: usize, settlement: usize) {
    let band = world.bands[index].clone();
    let people = world.settlements[settlement].culture_id;
    let placed = place(world, settlement, &band, people);
    world.settlements[settlement].population += placed;
}

/// Puts a band's people onto a settlement's land, founding tile first, never
/// past any tile's capacity: those a full catchment cannot feed are lost.
/// Returns how many found room.
fn place(world: &mut World, settlement: usize, band: &Band, people: u16) -> f32 {
    let founding = world.settlements[settlement].tile;
    let mut tiles = world.settlements[settlement].catchment.clone();
    tiles.sort_by_key(|&t| t != founding);
    let mut left = band.headcount;
    for tile in tiles {
        let room = (world.carrying_capacity[tile] - world.population[tile]).max(0.0);
        let put = left.min(room);
        if put <= 0.0 {
            continue;
        }
        if world.culture_id[tile] == NO_CULTURE {
            stamp(world, tile, people, band);
        }
        world.population[tile] += put;
        left -= put;
    }
    band.headcount - left
}

/// An empty tile takes the walkers' people, culture, belief and bodies.
fn stamp(world: &mut World, tile: usize, people: u16, band: &Band) {
    world.culture_id[tile] = people;
    world.culture_vec[tile * culture::DIMS..(tile + 1) * culture::DIMS]
        .copy_from_slice(&band.culture_vec);
    world.belief[tile * belief::AXES..(tile + 1) * belief::AXES].copy_from_slice(&band.belief);
    world.traits[tile * traits::TRAITS..(tile + 1) * traits::TRAITS].copy_from_slice(&band.traits);
}

/// People driven off sinking land leave as a band from where they stood, if
/// a walking slot is free; otherwise the sea takes them (D27).
pub(crate) fn refugees(world: &mut World, tile: usize, culture_id: u16, headcount: f32) {
    if world.bands.len() >= MAX_WALKING {
        return;
    }
    let mut belief = [0.0; belief::AXES];
    belief.copy_from_slice(&world.belief[tile * belief::AXES..(tile + 1) * belief::AXES]);
    let mut traits = [0.0; traits::TRAITS];
    traits.copy_from_slice(&world.traits[tile * traits::TRAITS..(tile + 1) * traits::TRAITS]);
    let band = Band {
        id: next_id(world),
        culture_id,
        tile: tile as u32,
        headcount,
        formed_tick: world.tick(),
        goal: Goal::Stranded,
        waded: 0,
        climbed: 0,
        route: Vec::new(),
        culture_vec: world.culture_vec_of(tile),
        belief,
        traits,
    };
    world.bands.push(band);
    let index = world.bands.len() - 1;
    plan_land(world, index);
}

/// The land moved under the walkers: every band still looking for land
/// plans again from where it stands, on the same clock.
pub(crate) fn replan_after_geology(world: &mut World) {
    for index in 0..world.bands.len() {
        if matches!(world.bands[index].goal, Goal::Land(_)) {
            plan_land(world, index);
        }
    }
}

/// Bands over [`SPLIT_ABOVE`] split in two while there are walking slots.
fn split_large(world: &mut World) {
    for index in 0..world.bands.len() {
        if world.bands[index].headcount > SPLIT_ABOVE && world.bands.len() < MAX_WALKING {
            part(world, index);
        }
    }
}

/// Every [`QUARREL_EVERY`] years a band may quarrel and part, likelier the
/// bigger it is and the weaker its communal bond (D20).
fn quarrel(world: &mut World) {
    for index in 0..world.bands.len() {
        let band = &world.bands[index];
        let size = (band.headcount / SPLIT_ABOVE).min(1.0);
        let chance = QUARREL_CHANCE * size * (1.0 - band.culture_vec[culture::COMMUNAL]);
        let roll = world.band_rng.random::<f32>();
        // Both halves must still be able to found a camp.
        let big_enough = world.bands[index].headcount >= 2.0 * SETTLE_MIN;
        if roll < chance && big_enough && world.bands.len() < MAX_WALKING {
            part(world, index);
        }
    }
}

/// Halves a band; the half that leaves is a new band on its own clock and
/// finds its own land.
fn part(world: &mut World, index: usize) {
    let tick = world.tick();
    let id = next_id(world);
    let band = &mut world.bands[index];
    band.headcount /= 2.0;
    let mut other = band.clone();
    other.id = id;
    other.formed_tick = tick;
    world.bands.push(other);
    let new = world.bands.len() - 1;
    plan_land(world, new);
}

fn next_id(world: &mut World) -> u32 {
    let id = world.next_band_id;
    world.next_band_id += 1;
    id
}

/// Crowded settlements bud a band each, round-robin from the last one served
/// so old cities cannot starve new ones, while walking slots last (D10). A
/// settlement with no reachable land waits for the next 25-year pass.
fn bud(world: &mut World) {
    let count = world.settlements.len();
    if count == 0 {
        return;
    }
    let tick = world.tick();
    let start = (world.bud_cursor as usize + 1) % count;
    for offset in 0..count {
        if world.bands.len() >= MAX_WALKING {
            return;
        }
        let settlement = (start + offset) % count;
        if world.settlements[settlement].no_land_until > tick || !crowded(world, settlement) {
            continue;
        }
        let from = world.settlements[settlement].tile;
        let reach = search(world, from, (SEARCH_COST * COST_SCALE) as u32);
        let line = world.cultures[world.settlements[settlement].culture_id as usize].line;
        let Some(goal) = best_land(world, &reach, line, BUD_SIZE) else {
            world.settlements[settlement].no_land_until = (tick / 25 + 1) * 25;
            continue;
        };
        let mut band = leave(world, settlement);
        band.goal = Goal::Land(goal as u32);
        band.route = reach.route_to(goal);
        world.bands.push(band);
        world.bud_cursor = settlement as u32;
    }
}

/// Whether a settlement has filled its land past its line's crowding share,
/// with enough people that a bud leaves a living camp behind.
fn crowded(world: &World, settlement: usize) -> bool {
    let settlement = &world.settlements[settlement];
    if settlement.catchment.is_empty() || settlement.culture_id == NO_CULTURE {
        return false;
    }
    let line = world.cultures[settlement.culture_id as usize].line;
    let (people, capacity) = settlement.catchment.iter().fold((0.0, 0.0), |(p, c), &t| {
        (p + world.population[t], c + world.carrying_capacity[t])
    });
    people >= 2.0 * BUD_SIZE && people >= CROWDED[line as usize] * capacity
}

/// Takes a bud of [`BUD_SIZE`] out of a settlement's land, evenly across its
/// tiles. The walkers are its most populous tile's people, with a founder's
/// twist on their culture.
fn leave(world: &mut World, settlement: usize) -> Band {
    let catchment = world.settlements[settlement].catchment.clone();
    let people: f32 = catchment.iter().map(|&t| world.population[t]).sum();
    let source = catchment
        .iter()
        .copied()
        .max_by(|&a, &b| {
            world.population[a]
                .total_cmp(&world.population[b])
                .then(b.cmp(&a))
        })
        .expect("a crowded settlement has land");
    let keep = 1.0 - BUD_SIZE / people;
    for &tile in &catchment {
        world.population[tile] *= keep;
    }
    let mut culture_vec = world.culture_vec_of(source);
    for value in &mut culture_vec {
        let shift = (world.band_rng.random::<f32>() - 0.5) * 2.0 * FOUNDER_MUTATION;
        *value = (*value + shift).clamp(0.0, 1.0);
    }
    let mut belief = [0.0; belief::AXES];
    belief.copy_from_slice(&world.belief[source * belief::AXES..(source + 1) * belief::AXES]);
    let mut traits = [0.0; traits::TRAITS];
    traits.copy_from_slice(&world.traits[source * traits::TRAITS..(source + 1) * traits::TRAITS]);
    Band {
        id: next_id(world),
        culture_id: world.culture_id[source],
        tile: world.settlements[settlement].tile as u32,
        headcount: BUD_SIZE,
        formed_tick: world.tick(),
        goal: Goal::Stranded,
        waded: 0,
        climbed: 0,
        route: Vec::new(),
        culture_vec,
        belief,
        traits,
    }
}

/// Every tile a search reached, with its crossing cost and the step before it.
struct Reach {
    cost: Vec<u32>,
    prev: Vec<u32>,
}

impl Reach {
    fn reached(&self, tile: usize) -> bool {
        self.cost[tile] != u32::MAX
    }

    /// The steps from the search's start to `goal`, the last first.
    fn route_to(&self, goal: usize) -> Vec<u32> {
        let mut route = Vec::new();
        let mut at = goal;
        while self.prev[at] != u32::MAX {
            route.push(at as u32);
            at = self.prev[at] as usize;
        }
        route
    }
}

/// Cheapest crossings from `from` over `culture::step_cost` and coastal water,
/// up to `limit` (D10, D22). Costs are fixed-point and the heap orders by (cost, tile), so the
/// same land always gives the same routes.
fn search(world: &World, from: usize, limit: u32) -> Reach {
    let tiles = world.biome.len();
    let mut cost = vec![u32::MAX; tiles];
    let mut prev = vec![u32::MAX; tiles];
    let mut frontier = BinaryHeap::new();
    cost[from] = 0;
    frontier.push(Reverse((0_u32, from)));
    while let Some(Reverse((spent, tile))) = frontier.pop() {
        if spent > cost[tile] {
            continue;
        }
        let (x, y) = (tile % GRID_WIDTH, tile / GRID_WIDTH);
        for (nx, ny) in hex::neighbors(x, y) {
            let next = ny * GRID_WIDTH + nx;
            let step = crossing_cost(world, next, nx != x);
            if !step.is_finite() {
                continue;
            }
            let total = spent.saturating_add((step * COST_SCALE) as u32);
            if total > limit || total >= cost[next] {
                continue;
            }
            cost[next] = total;
            prev[next] = tile as u32;
            frontier.push(Reverse((total, next)));
        }
    }
    Reach { cost, prev }
}

/// What one step onto `tile` costs a band: the land's crossing cost, or
/// [`COASTAL_WATER`] for ocean within a hex of land (D22).
fn crossing_cost(world: &World, tile: usize, east_west: bool) -> f32 {
    let (x, y) = (tile % GRID_WIDTH, tile / GRID_WIDTH);
    let land = culture::step_cost(world.biome[tile], worldgen::latitude_deg(y), east_west);
    if land.is_finite() || world.biome[tile] != Biome::Ocean {
        return land;
    }
    let ashore = hex::neighbors(x, y)
        .any(|(nx, ny)| !matches!(world.biome[ny * GRID_WIDTH + nx], Biome::Ocean | Biome::Ice));
    if ashore { COASTAL_WATER } else { f32::INFINITY }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRID_HEIGHT_MID: usize = crate::world::GRID_HEIGHT / 2;

    /// A band that has walked 100 years without settling heads to the nearest
    /// settlement of its own people and joins it there (D16).
    #[test]
    fn a_band_past_its_years_walks_home_and_joins_its_people() {
        let mut world = World::new(1);
        world.step(STRANDED_YEARS as u32 + 1);
        let home = world.settlements[0].tile;
        let people = world.settlements[0].culture_id;
        let start = hex::ring(home % GRID_WIDTH, home / GRID_WIDTH, 3)
            .map(|(x, y)| y * GRID_WIDTH + x)
            .find(|&t| worldgen::settleable(world.biome[t]))
            .expect("land three hexes from the first camp");
        world.bands.clear();
        world.bands.push(Band {
            id: 999,
            culture_id: people,
            tile: start as u32,
            headcount: 20.0,
            formed_tick: world.tick() - STRANDED_YEARS,
            goal: Goal::Land(start as u32),
            waded: 0,
            climbed: 0,
            route: vec![start as u32],
            culture_vec: world.culture_vec_of(home),
            belief: [0.0; belief::AXES],
            traits: [0.5; traits::TRAITS],
        });
        assert!(!walk(&mut world, 0), "the band sets out for home");
        let Goal::Kin(kin) = world.bands[0].goal else {
            panic!(
                "a band past its years heads to its kin, not {:?}",
                world.bands[0].goal
            );
        };
        let kin = kin as usize;
        assert_eq!(
            world.settlements[kin].culture_id, people,
            "its own people first"
        );
        // Make room, so every walker can be counted on arrival.
        for &tile in &world.settlements[kin].catchment.clone() {
            world.population[tile] *= 0.5;
        }
        let before: f32 = world.settlements[kin]
            .catchment
            .iter()
            .map(|&t| world.population[t])
            .sum();
        let walkers = world.bands[0].headcount;
        let mut years = 0;
        while !walk(&mut world, 0) {
            years += 1;
            assert!(years < 20, "the walk home ends");
        }
        let after: f32 = world.settlements[kin]
            .catchment
            .iter()
            .map(|&t| world.population[t])
            .sum();
        // Walkers grow on the road and never starve, so every one of the
        // 20 arrives, and a few more.
        let joined = after - before;
        assert!(
            joined >= walkers - 0.01 && joined < 2.0 * walkers,
            "the walkers join their people: {before} + {walkers} -> {after}"
        );
    }

    /// A route goes around a mountain belt rather than over it when going
    /// around costs less (D10).
    #[test]
    fn a_route_skirts_a_mountain_belt() {
        let mut world = World::new(1);
        let y = GRID_HEIGHT_MID;
        for tile in 0..world.biome.len() {
            world.biome[tile] = Biome::Grassland;
        }
        for row in y - 3..=y + 3 {
            for x in 50..55 {
                world.biome[row * GRID_WIDTH + x] = Biome::Mountain;
            }
        }
        let from = y * GRID_WIDTH + 45;
        let goal = y * GRID_WIDTH + 58;
        let reach = search(&world, from, u32::MAX);
        let route = reach.route_to(goal);
        assert_eq!(
            route.first(),
            Some(&(goal as u32)),
            "the route ends at the goal"
        );
        assert!(
            route
                .iter()
                .all(|&t| world.biome[t as usize] != Biome::Mountain),
            "the band walks around the range"
        );
        let straight: u32 = (46..=58)
            .map(|x| {
                let biome = world.biome[y * GRID_WIDTH + x];
                (culture::step_cost(biome, worldgen::latitude_deg(y), true) * COST_SCALE) as u32
            })
            .sum();
        assert!(
            reach.cost[goal] < straight,
            "going around ({}) is cheaper than over ({straight})",
            reach.cost[goal]
        );
    }

    /// A band crosses a strait two hexes wide, where every water hex touches
    /// land, but not open ocean six hexes wide (D22).
    #[test]
    fn a_band_crosses_a_narrow_strait_but_not_open_ocean() {
        let mut world = World::new(1);
        let y = GRID_HEIGHT_MID;
        for tile in 0..world.biome.len() {
            world.biome[tile] = Biome::Ocean;
        }
        let row = |world: &mut World, xs: std::ops::Range<usize>| {
            for r in y - 2..=y + 2 {
                for x in xs.clone() {
                    world.biome[r * GRID_WIDTH + x] = Biome::Grassland;
                }
            }
        };
        row(&mut world, 40..50);
        row(&mut world, 52..60);
        row(&mut world, 66..76);
        let from = y * GRID_WIDTH + 45;
        let reach = search(&world, from, u32::MAX);
        assert!(
            reach.reached(y * GRID_WIDTH + 55),
            "the land past the strait"
        );
        assert!(
            !reach.reached(y * GRID_WIDTH + 70),
            "the land past open ocean stays out of reach"
        );
        let route = reach.route_to(y * GRID_WIDTH + 55);
        let wet = route
            .iter()
            .filter(|&&t| world.biome[t as usize] == Biome::Ocean)
            .count();
        assert_eq!(wet, 2, "the band wades the two-hex strait and no more");
    }

    /// Two settlements equally crowded: Dirt's buds a band, the snake's line's
    /// holds on to its people (D13).
    #[test]
    fn the_snakes_line_buds_later_than_dirt() {
        let mut world = World::new(1);
        world.step(1);
        let shares = [0.85_f32, 0.85];
        for (settlement, share) in shares.iter().enumerate() {
            for &tile in &world.settlements[settlement].catchment.clone() {
                world.population[tile] = world.carrying_capacity[tile] * share;
            }
        }
        let lines: Vec<Line> = (0..2)
            .map(|s| world.cultures[world.settlements[s].culture_id as usize].line)
            .collect();
        let crowding: Vec<bool> = (0..2).map(|s| crowded(&world, s)).collect();
        for (line, buds) in lines.iter().zip(&crowding) {
            match line {
                Line::Dirt => assert!(buds, "Dirt buds at 85% of its land"),
                Line::Snake => assert!(!buds, "the snake's line holds on at 85%"),
            }
        }
    }
}
