//! Settlements and their tiers (design, Buildings and settlement advancement;
//! Milestone 6a).
//!
//! A settlement is a claimed catchment: a band founds it on one tile, the
//! radius-2 neighborhood around her becomes her territory, and the people
//! living on those tiles are her population (Out of the Garden step 2, D7). Tiers are percentiles of the claimed
//! catchments, recomputed every 25-tick check, so the ladder keeps its shape
//! as the world grows (eng review, D3).

use crate::buildings::{BuildJob, Building};
use crate::hex;
use crate::world::{GRID_WIDTH, World};

/// A settlement's tier, 0–3 in Milestone 6a (tiers 4–5 wait for later eras).
#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
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
    /// Building slots (count from the tier table; filled with buildings).
    pub slots: Vec<Option<Building>>,
    /// Accumulated labor, spent on construction.
    pub labor: f32,
    /// Subsistence share: 1.0 = all labor goes to food, 0.0 = all free.
    pub subsistence: f32,
    /// What is currently being built, if anything.
    pub current_build: Option<BuildJob>,
    /// Unpaid upkeep years, for decay.
    pub unpaid_years: u32,
    /// Consecutive checks with less than one person; two release the land
    /// (D16).
    pub empty_checks: u8,
    /// Until this tick no land is reachable, so the settlement does not bud
    /// (outside voice: a stranded search is cached to the next pass).
    pub no_land_until: u64,
}

/// Number of building slots by tier (design, Buildings table).
pub fn slots_for_tier(tier: u8) -> usize {
    match tier {
        0 => 1,
        1 => 3,
        2 => 6,
        _ => 10,
    }
}

/// Least people a band needs to found a camp: about one well-populated
/// tile's worth (D7).
pub const CAMP_THRESHOLD: f32 = 8.0;

/// Radius of a settlement's catchment, in hex steps.
pub const CATCHMENT_RADIUS: u32 = 2;

/// Claims a tier-0 camp for `culture_id` on `tile`: the founding tile and
/// every unclaimed tile within [`CATCHMENT_RADIUS`] that no other people
/// lives on (D7). Returns the new settlement's index.
pub(crate) fn claim(world: &mut World, tile: usize, culture_id: u16) -> usize {
    let (cx, cy) = (tile % GRID_WIDTH, tile / GRID_WIDTH);
    let catchment: Vec<usize> = hex::disc(cx, cy, CATCHMENT_RADIUS)
        .into_iter()
        .map(|(x, y)| y * GRID_WIDTH + x)
        .filter(|&t| world.settlement_of[t] == u32::MAX)
        .filter(|&t| {
            world.culture_id[t] == crate::culture::NO_CULTURE || world.culture_id[t] == culture_id
        })
        .collect();
    let id = world.settlements.len() as u32;
    for &t in &catchment {
        world.settlement_of[t] = id;
    }
    world.settlements.push(Settlement {
        id,
        culture_id,
        tile,
        catchment,
        population: 0.0,
        tier: 0,
        tier_checks_below: 0,
        slots: vec![None; slots_for_tier(0)],
        labor: 0.0,
        subsistence: 0.8,
        current_build: None,
        unpaid_years: 0,
        empty_checks: 0,
        no_land_until: 0,
    });
    id as usize
}

/// Tick settlements: recompute populations, release emptied land, hand each
/// settlement to its majority people, and re-tier. Bands found settlements
/// now; land outside a catchment is never settled by density (D7).
#[expect(
    clippy::bool_to_int_with_if,
    reason = "the tier ladder reads top-down as a chain of thresholds"
)]
pub fn tick_settlements(world: &mut World) {
    for settlement in &mut world.settlements {
        settlement.population = settlement
            .catchment
            .iter()
            .map(|&t| world.population[t])
            .sum();
    }
    release_emptied(world);
    hand_to_majority(world);

    // Percentile thresholds over living settlements, with hysteresis on
    // demotion.
    let mut pops: Vec<f32> = world
        .settlements
        .iter()
        .filter(|s| !s.catchment.is_empty())
        .map(|s| s.population)
        .collect();
    if pops.is_empty() {
        return;
    }
    pops.sort_by(|a, b| b.total_cmp(a));
    let n = pops.len();
    let city_idx = ((n as f32 * 0.01).ceil() as usize).clamp(1, n) - 1;
    let town_idx = ((n as f32 * 0.05).ceil() as usize).clamp(1, n) - 1;
    let village_idx = ((n as f32 * 0.25).ceil() as usize).clamp(1, n) - 1;
    let city_cut = pops[city_idx];
    let town_cut = pops[town_idx];
    let village_cut = pops[village_idx];

    for settlement in world
        .settlements
        .iter_mut()
        .filter(|s| !s.catchment.is_empty())
    {
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
                // Resize slots on demotion.
                let new_slots = slots_for_tier(target);
                if settlement.slots.len() > new_slots {
                    settlement.slots.truncate(new_slots);
                } else {
                    settlement.slots.resize(new_slots, None);
                }
            }
        } else {
            settlement.tier_checks_below = 0;
            if target > settlement.tier {
                settlement.tier = target;
                // Resize slots on promotion.
                let new_slots = slots_for_tier(target);
                if settlement.slots.len() > new_slots {
                    settlement.slots.truncate(new_slots);
                } else {
                    settlement.slots.resize(new_slots, None);
                }
            }
        }
    }
}

/// A settlement below one person for two checks releases its catchment, so
/// the land can reopen to bands (D16). Its record stays, emptied, so ids keep
/// pointing where they did.
fn release_emptied(world: &mut World) {
    for index in 0..world.settlements.len() {
        let settlement = &mut world.settlements[index];
        if settlement.catchment.is_empty() {
            continue;
        }
        if settlement.population >= 1.0 {
            settlement.empty_checks = 0;
            continue;
        }
        settlement.empty_checks += 1;
        if settlement.empty_checks < 2 {
            continue;
        }
        settlement.tier = 0;
        settlement.slots.truncate(slots_for_tier(0));
        settlement.current_build = None;
        for tile in std::mem::take(&mut settlement.catchment) {
            world.settlement_of[tile] = u32::MAX;
            world.population[tile] = 0.0;
            world.culture_id[tile] = crate::culture::NO_CULTURE;
        }
    }
}

/// Each settlement belongs to the people most of its catchment's people are,
/// so after a split the daughter owns her own camps (outside voice).
fn hand_to_majority(world: &mut World) {
    for settlement in &mut world.settlements {
        let mut counts = std::collections::BTreeMap::new();
        for &tile in &settlement.catchment {
            let people = world.culture_id[tile];
            if people != crate::culture::NO_CULTURE && world.population[tile] >= 1.0 {
                *counts.entry(people).or_insert(0.0_f32) += world.population[tile];
            }
        }
        // The larger people; the lower id on a tie.
        let majority = counts
            .into_iter()
            .max_by(|(a, x), (b, y)| x.total_cmp(y).then(b.cmp(a)))
            .map(|(people, _)| people);
        if let Some(people) = majority {
            settlement.culture_id = people;
        }
    }
}

/// Accumulate labor for one tick: population × (1 − subsistence).
pub fn tick_labor(world: &mut World) {
    for settlement in &mut world.settlements {
        settlement.labor += settlement.population * (1.0 - settlement.subsistence);
    }
}

/// Run construction and upkeep decisions on the 25-tick check.
pub fn tick_construction(world: &mut World) {
    tick_upkeep(world);
    tick_build(world);
}

/// Upkeep: each building level costs upkeep; decay if unpaid.
fn tick_upkeep(world: &mut World) {
    use crate::buildings::load_catalog;
    let catalog = load_catalog();
    for settlement in &mut world.settlements {
        let upkeep: u32 = settlement
            .slots
            .iter()
            .filter_map(|s| s.as_ref())
            .map(|b| {
                catalog
                    .lines
                    .iter()
                    .find(|l| l.id == b.line)
                    .and_then(|l| l.levels.get(b.level as usize))
                    .map_or(0, |lvl| lvl.upkeep)
            })
            .sum();
        let upkeep_labor = upkeep as f32;
        if settlement.labor >= upkeep_labor {
            settlement.labor -= upkeep_labor;
            settlement.unpaid_years = 0;
        } else {
            settlement.unpaid_years += 25;
            if settlement.unpaid_years >= 50 {
                for slot in &mut settlement.slots {
                    if let Some(b) = slot
                        && b.level > 0
                    {
                        b.level -= 1;
                        break;
                    }
                }
                settlement.unpaid_years = 0;
            }
        }
    }
}

/// Construction: pick the highest-scoring candidate, fill the build slot.
fn tick_build(world: &mut World) {
    use crate::buildings::load_catalog;
    let catalog = load_catalog();
    let era: u8 = 2; // M6a: up to Iron era.

    for settlement in &mut world.settlements {
        if settlement.current_build.is_some() {
            advance_build(settlement);
            continue;
        }
        let empty_slot = settlement.slots.iter().position(Option::is_none);
        let Some(_slot) = empty_slot else { continue };

        let mut best: Option<(u8, f32)> = None;
        for line in &catalog.lines {
            let next_level = settlement
                .slots
                .iter()
                .filter_map(|s| s.as_ref())
                .filter(|b| b.line == line.id)
                .map(|b| b.level + 1)
                .max()
                .unwrap_or(0);
            if next_level as usize >= line.levels.len() {
                continue;
            }
            let level = &line.levels[next_level as usize];
            if level.era > era {
                continue;
            }
            // Need: simplified — 0.5 baseline for M6a step 2.
            // Culture weight and doctrine modifier: refined in step 4.
            let need = 0.5_f32;
            let weight = 1.0_f32;
            let doctrine = 1.0_f32;
            let score = need * weight * doctrine;
            let is_better = match best {
                None => true,
                Some((best_line, best_score)) => {
                    score > best_score || ((score - best_score).abs() < 1e-6 && line.id < best_line)
                }
            };
            if is_better {
                best = Some((line.id, score));
            }
        }
        if let Some((line_id, _)) = best {
            let next_level = settlement
                .slots
                .iter()
                .filter_map(|s| s.as_ref())
                .filter(|b| b.line == line_id)
                .map(|b| b.level + 1)
                .max()
                .unwrap_or(0);
            let labor_needed = 10.0 * (f32::from(next_level) + 1.0);
            settlement.current_build = Some(BuildJob {
                line: line_id,
                target_level: next_level,
                progress: 0.0,
                labor_needed,
            });
        }
    }
}

/// Advance a settlement's current build by available labor.
fn advance_build(settlement: &mut Settlement) {
    if let Some(job) = &mut settlement.current_build {
        let remaining = job.labor_needed - job.progress;
        let spent = settlement.labor.min(remaining);
        job.progress += spent;
        settlement.labor -= spent;
        if job.progress >= job.labor_needed {
            let line_id = job.line;
            let target_level = job.target_level;
            let slot = settlement.slots.iter().position(Option::is_none);
            if let Some(slot_idx) = slot {
                settlement.slots[slot_idx] = Some(Building {
                    line: line_id,
                    level: target_level,
                });
            }
            settlement.current_build = None;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_camp_is_founded_on_settled_ground() {
        let mut world = crate::world::World::new(1);
        world.step(700);
        let settlements = &world.settlements;
        assert!(!settlements.is_empty());
        for s in settlements {
            assert!(world.population[s.tile] >= 1.0);
        }
    }
}
