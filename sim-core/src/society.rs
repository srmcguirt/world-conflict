//! Polities, strata, meters and governments (design, Social balance;
//! Milestone 6a). Step 3 of 5: polities, territory and cohesion. Meters and
//! governments follow in step 4.

use crate::hex;
use crate::world::{GRID_WIDTH, World};

/// A polity's government. M6a ships four; the rest arrive with M8.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Government {
    Band,
    Chiefdom,
    Monarchy,
    Theocracy,
}

impl Government {
    pub fn name(self) -> &'static str {
        match self {
            Government::Band => "band",
            Government::Chiefdom => "chiefdom",
            Government::Monarchy => "monarchy",
            Government::Theocracy => "theocracy",
        }
    }
}

/// A polity: a territory with a capital, a government and meters.
#[derive(Clone, Debug)]
pub struct Polity {
    pub id: u32,
    pub culture_id: u16,
    /// The settlement that founded this polity.
    pub capital: u32,
    /// Settlements belonging to this polity.
    pub settlements: Vec<u32>,
    pub government: Government,
    /// How alike the polity's tiles are culturally and in faith, 0–1.
    pub cohesion: f32,
}

/// How far a settlement can be from a polity's capital and still join it.
const REACH: u32 = 10;

/// Tick polities: found, join, assign territory, compute cohesion.
pub fn tick_polities(world: &mut World) {
    found_polities(world);
    join_polities(world);
    assign_territory(world);
    compute_cohesion(world);
}

/// Found new polities from tier-1+ settlements with no polity within reach.
fn found_polities(world: &mut World) {
    let mut new_polities: Vec<Polity> = Vec::new();
    let existing_capitals: Vec<(u32, usize)> = world
        .polities
        .iter()
        .map(|p| {
            let cap = world
                .settlements
                .iter()
                .find(|s| s.id == p.capital)
                .map_or(0, |s| s.tile);
            (p.id, cap)
        })
        .collect();

    for settlement in &world.settlements {
        if settlement.tier < 1 {
            continue;
        }
        let within_reach = existing_capitals.iter().any(|&(_, cap_tile)| {
            let (cx, cy) = (settlement.tile % GRID_WIDTH, settlement.tile / GRID_WIDTH);
            let (px, py) = (cap_tile % GRID_WIDTH, cap_tile / GRID_WIDTH);
            hex::distance(cx, cy, px, py) <= REACH
        });
        if within_reach {
            continue;
        }
        let already_founded = new_polities.iter().any(|p| {
            let cap = world
                .settlements
                .iter()
                .find(|s| s.id == p.capital)
                .map_or(0, |s| s.tile);
            let (cx, cy) = (settlement.tile % GRID_WIDTH, settlement.tile / GRID_WIDTH);
            let (px, py) = (cap % GRID_WIDTH, cap / GRID_WIDTH);
            hex::distance(cx, cy, px, py) <= REACH
        });
        if already_founded {
            continue;
        }
        let id = (world.polities.len() + new_polities.len()) as u32;
        new_polities.push(Polity {
            id,
            culture_id: settlement.culture_id,
            capital: settlement.id,
            settlements: vec![settlement.id],
            government: Government::Band,
            cohesion: 1.0,
        });
    }
    world.polities.extend(new_polities);
}

/// Settlements within reach of a polity's capital join it.
fn join_polities(world: &mut World) {
    let mut to_join: Vec<(u32, u32)> = Vec::new();
    for settlement in &world.settlements {
        if settlement.tier < 1 {
            continue;
        }
        if world
            .polities
            .iter()
            .any(|p| p.settlements.contains(&settlement.id))
        {
            continue;
        }
        let mut best: Option<(u32, u32)> = None;
        for polity in &world.polities {
            let cap_tile = world
                .settlements
                .iter()
                .find(|s| s.id == polity.capital)
                .map_or(0, |s| s.tile);
            let (cx, cy) = (settlement.tile % GRID_WIDTH, settlement.tile / GRID_WIDTH);
            let (px, py) = (cap_tile % GRID_WIDTH, cap_tile / GRID_WIDTH);
            let d = hex::distance(cx, cy, px, py);
            if d <= REACH && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((polity.id, d));
            }
        }
        if let Some((polity_id, _)) = best {
            to_join.push((settlement.id, polity_id));
        }
    }
    for (sid, pid) in to_join {
        if let Some(polity) = world.polities.iter_mut().find(|p| p.id == pid) {
            polity.settlements.push(sid);
        }
    }
}

/// Each settled tile belongs to the polity of its nearest settlement.
fn assign_territory(world: &mut World) {
    world.tile_polity.fill(u16::MAX);
    for i in 0..world.population.len() {
        if world.population[i] < 1.0 {
            continue;
        }
        let (x, y) = (i % GRID_WIDTH, i / GRID_WIDTH);
        let mut best: Option<(u32, u32)> = None;
        for settlement in &world.settlements {
            let (sx, sy) = (settlement.tile % GRID_WIDTH, settlement.tile / GRID_WIDTH);
            let d = hex::distance(x, y, sx, sy);
            if d <= REACH && best.is_none_or(|(_, bd)| d < bd) {
                let polity_id = world
                    .polities
                    .iter()
                    .find(|p| p.settlements.contains(&settlement.id))
                    .map(|p| p.id);
                if let Some(pid) = polity_id {
                    best = Some((pid, d));
                }
            }
        }
        if let Some((pid, _)) = best {
            world.tile_polity[i] = pid as u16;
        }
    }
}

/// Cohesion: 1 - spread of belief across the polity's tiles.
fn compute_cohesion(world: &mut World) {
    let polity_ids: Vec<u32> = world.polities.iter().map(|p| p.id).collect();
    for pid in polity_ids {
        let tiles: Vec<usize> = world
            .tile_polity
            .iter()
            .enumerate()
            .filter(|&(_, &p)| p == pid as u16)
            .map(|(i, _)| i)
            .collect();
        if tiles.is_empty() {
            if let Some(polity) = world.polities.iter_mut().find(|p| p.id == pid) {
                polity.cohesion = 1.0;
            }
            continue;
        }
        let belief_dim = crate::belief::AXES;
        let mut mean = vec![0.0_f32; belief_dim];
        for &t in &tiles {
            let base = t * belief_dim;
            for (m, mean_val) in mean.iter_mut().enumerate() {
                *mean_val += world.belief[base + m];
            }
        }
        for mean_val in &mut mean {
            *mean_val /= tiles.len() as f32;
        }
        let mut var = 0.0_f32;
        for &t in &tiles {
            let base = t * belief_dim;
            for (m, _) in mean.iter().enumerate() {
                let diff = world.belief[base + m] - mean[m];
                var += diff * diff;
            }
        }
        var /= (tiles.len() * belief_dim) as f32;
        let cohesion = (1.0 - libm::sqrtf(var)).clamp(0.0, 1.0);
        if let Some(polity) = world.polities.iter_mut().find(|p| p.id == pid) {
            polity.cohesion = cohesion;
        }
    }
}

/// Which polity a settlement belongs to, if any.
pub fn polity_of_settlement(world: &World, settlement_id: u32) -> Option<u32> {
    world
        .polities
        .iter()
        .find(|p| p.settlements.contains(&settlement_id))
        .map(|p| p.id)
}
