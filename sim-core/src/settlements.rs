//! Settlements and their tiers (design, Buildings and settlement advancement;
//! Milestone 6a).
//!
//! A settlement is a claimed catchment: one tile founds it, the radius-2
//! neighborhood around her becomes her territory, and the people living on
//! those tiles are her population. Tiers are percentiles of the claimed
//! catchments, recomputed every 25-tick check, so the ladder keeps its shape
//! as the world grows (eng review, D3).

use crate::hex;
use crate::world::{GRID_WIDTH, World};

/// A settlement's tier, 0–3 in Milestone 6a (tiers 4–5 wait for later eras).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Tier {
    Camp = 0,
    Village = 1,
    Town = 2,
    City = 3,
}

impl Tier {
    pub fn from_index(index: u8) -> Self {
        match index {
            0 => Tier::Camp,
            1 => Tier::Village,
            2 => Tier::Town,
            _ => Tier::City,
        }
    }
}

/// A people's settlement: a founding tile, her claimed catchment, and her tier.
#[derive(Clone, Debug)]
pub struct Settlement {
    pub id: u32,
    pub culture_id: u16,
    /// The tile that founded this settlement.
    pub tile: usize,
    /// The tiles this settlement claims (founding tile + radius-2 neighborhood).
    pub catchment: Vec<usize>,
    /// People living on the claimed tiles.
    pub population: f32,
    pub tier: u8,
    /// Consecutive checks below the tier's threshold; resets on recovery.
    pub tier_checks_below: u8,
}

/// A tile becomes a settlement when its catchment holds at least this many
/// people (about one well-populated tile's worth).
const CAMP_THRESHOLD: f32 = 8.0;

/// Radius of a settlement's catchment, in hex steps.
const CATCHMENT_RADIUS: u32 = 2;

/// Tick settlements: found new ones, recompute populations and tiers.
#[expect(
    clippy::bool_to_int_with_if,
    reason = "four-way tier ladder from percentile cuts, not a bool conversion"
)]
pub fn tick_settlements(world: &mut World) {
    // 1. Found new settlements: the densest unclaimed tile, greedily.
    loop {
        let best = (0..world.population.len())
            .filter(|&i| world.population[i] >= 1.0 && world.settlement_of[i] == u32::MAX)
            .max_by(|&a, &b| world.population[a].total_cmp(&world.population[b]));
        let Some(tile) = best else { break };

        let (cx, cy) = (tile % GRID_WIDTH, tile / GRID_WIDTH);
        let catchment_tiles = hex::disc(cx, cy, CATCHMENT_RADIUS);
        // Only unclaimed tiles join this catchment.
        let catchment: Vec<usize> = catchment_tiles
            .iter()
            .map(|&(x, y)| y * GRID_WIDTH + x)
            .filter(|&t| world.settlement_of[t] == u32::MAX)
            .collect();
        let catchment_pop: f32 = catchment.iter().map(|&t| world.population[t]).sum();

        if catchment_pop < CAMP_THRESHOLD {
            break;
        }

        let id = world.settlements.len() as u32;
        let culture_id = world.culture_id[tile];
        world.settlements.push(Settlement {
            id,
            culture_id,
            tile,
            catchment: catchment.clone(),
            population: catchment_pop,
            tier: 0,
            tier_checks_below: 0,
        });
        for t in &catchment {
            world.settlement_of[*t] = id;
        }
    }

    // 2. Recompute catchment populations.
    for settlement in &mut world.settlements {
        settlement.population = settlement
            .catchment
            .iter()
            .map(|&t| world.population[t])
            .sum();
    }

    // 3. Percentile thresholds, with hysteresis on demotion.
    if world.settlements.is_empty() {
        return;
    }
    let mut pops: Vec<f32> = world.settlements.iter().map(|s| s.population).collect();
    pops.sort_by(|a, b| b.total_cmp(a));
    let n = pops.len();
    let city_idx = ((n as f32 * 0.01).ceil() as usize).clamp(1, n) - 1;
    let town_idx = ((n as f32 * 0.05).ceil() as usize).clamp(1, n) - 1;
    let village_idx = ((n as f32 * 0.25).ceil() as usize).clamp(1, n) - 1;
    let city_cut = pops[city_idx];
    let town_cut = pops[town_idx];
    let village_cut = pops[village_idx];

    for settlement in &mut world.settlements {
        let base_target = if settlement.population >= city_cut {
            3u8
        } else if settlement.population >= town_cut {
            2
        } else if settlement.population >= village_cut {
            1
        } else {
            0
        };
        // A settlement with no people is not a city: force the bottom tier so
        // hysteresis can demote it honestly.
        let target = if settlement.population < 1.0 {
            0
        } else {
            base_target
        };
        if target < settlement.tier {
            settlement.tier_checks_below = settlement.tier_checks_below.saturating_add(1);
            if settlement.tier_checks_below >= 2 {
                settlement.tier = target;
                settlement.tier_checks_below = 0;
            }
        } else {
            settlement.tier_checks_below = 0;
            if target > settlement.tier {
                settlement.tier = target;
            }
        }
    }
}
