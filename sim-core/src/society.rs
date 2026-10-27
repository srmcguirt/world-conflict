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
    // --- meters (0–1, refreshed each 25-tick check) ---
    pub prosperity: f32,
    pub inequality: f32,
    pub legitimacy: f32,
    pub piety: f32,
    pub liberty: f32,
    pub unrest: f32,
    // --- strata shares (of population) ---
    pub commoner_share: f32,
    pub artisan_share: f32,
    pub priest_share: f32,
    pub ruler_share: f32,
    // --- strata wealth shares ---
    pub commoner_wealth: f32,
    pub artisan_wealth: f32,
    pub priest_wealth: f32,
    pub ruler_wealth: f32,
    // --- government evolution ---
    pub government_checks: u8,
}

/// How far a settlement can be from a polity's capital and still join it.
const REACH: u32 = 10;

/// Tick polities: found, join, assign territory, compute cohesion.
pub fn tick_polities(world: &mut World) {
    found_polities(world);
    join_polities(world);
    assign_territory(world);
    compute_cohesion(world);
    tick_meters(world);
    tick_governments(world);
    tick_unrest(world);
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
            prosperity: 0.5,
            inequality: 0.3,
            legitimacy: 0.5,
            piety: 0.5,
            liberty: 0.3,
            unrest: 0.0,
            commoner_share: 0.7,
            artisan_share: 0.15,
            priest_share: 0.10,
            ruler_share: 0.05,
            commoner_wealth: 0.4,
            artisan_wealth: 0.2,
            priest_wealth: 0.2,
            ruler_wealth: 0.2,
            government_checks: 0,
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

/// Meters: refreshed each 25-tick check with hysteresis.
pub fn tick_meters(world: &mut World) {
    for polity in &mut world.polities {
        let tiles: Vec<usize> = world
            .tile_polity
            .iter()
            .enumerate()
            .filter(|&(_, &p)| p == polity.id as u16)
            .map(|(i, _)| i)
            .collect();
        if tiles.is_empty() {
            continue;
        }
        // Prosperity: food + goods per person, simplified to population density.
        let pop: f32 = tiles.iter().map(|&t| world.population[t]).sum();
        polity.prosperity = (pop / (tiles.len() as f32 * 12.0)).clamp(0.0, 1.0);

        // Piety: culture-level belief magnitude.
        let culture = world.cultures.get(polity.culture_id as usize);
        polity.piety = culture.map_or(0.5, |c| {
            let axes = c.belief.to_axes();
            let sum: f32 = axes.iter().map(|x| x.abs()).sum();
            (sum / crate::belief::AXES as f32).clamp(0.0, 1.0)
        });

        // Legitimacy: source depends on government.
        polity.legitimacy = match polity.government {
            Government::Theocracy => polity.piety,
            Government::Monarchy | Government::Band => 0.5,
            Government::Chiefdom => polity.prosperity,
        };

        // Unrest: rises with inequality, low prosperity, low legitimacy, low cohesion.
        polity.unrest = (polity.inequality * 0.3
            + (1.0 - polity.prosperity) * 0.3
            + (1.0 - polity.legitimacy) * 0.2
            + (1.0 - polity.cohesion) * 0.2)
            .clamp(0.0, 1.0);
    }
}

/// Government evolution: threshold rules, 2 checks.
pub fn tick_governments(world: &mut World) {
    for polity in &mut world.polities {
        let target = match polity.government {
            Government::Band => {
                // Band -> Chiefdom when a tier-1+ settlement exists.
                let has_village = polity
                    .settlements
                    .iter()
                    .any(|&sid| world.settlements.iter().any(|s| s.id == sid && s.tier >= 1));
                if has_village {
                    Some(Government::Chiefdom)
                } else {
                    None
                }
            }
            Government::Chiefdom => {
                // Chiefdom -> Theocracy: piety > 0.7 and priest wealth >= 30%.
                if polity.piety > 0.7 && polity.priest_wealth >= 0.3 {
                    Some(Government::Theocracy)
                } else if polity.legitimacy > 0.5 {
                    Some(Government::Monarchy)
                } else {
                    None
                }
            }
            _ => None, // Monarchy and Theocracy are terminal in M6a.
        };

        if let Some(new_gov) = target {
            polity.government_checks = polity.government_checks.saturating_add(1);
            if polity.government_checks >= 2 {
                polity.government = new_gov;
                polity.government_checks = 0;
            }
        } else {
            polity.government_checks = 0;
        }
    }
}

/// Unrest outcomes: protest and riot (M6a). Rebellion/revolution/exodus
/// wait for M8.
pub fn tick_unrest(world: &mut World) {
    // Collect polity ids and their unrest levels.
    let unrest_levels: Vec<(u32, f32, u16)> = world
        .polities
        .iter()
        .map(|p| (p.id, p.unrest, p.culture_id))
        .collect();

    for (polity_id, unrest, culture_id) in unrest_levels {
        if unrest < 0.5 {
            continue;
        }
        // Protest: write a verse for the culture's scripture.
        let ci = culture_id as usize;
        let capital = world
            .polities
            .iter()
            .find(|p| p.id == polity_id)
            .map(|p| p.capital);
        let tile = capital.map_or(0, |id| crate::events::capital_tile(world, id));
        let kingdom_event = |kind| crate::events::Event {
            tick: world.tick(),
            kind,
            culture_id,
            tile,
            polity_id: Some(polity_id),
        };
        let protest = kingdom_event(crate::events::EventKind::Protest);
        let riot = kingdom_event(crate::events::EventKind::Riot);
        world.events.push(protest);
        if ci < world.cultures.len() {
            let place = if let Some(settlement) = world.settlements.iter().find(|s| {
                world
                    .polities
                    .iter()
                    .any(|p| p.id == polity_id && p.capital == s.id)
            }) {
                crate::names::place_name(&world.cultures[ci], settlement.tile)
            } else {
                String::new()
            };
            let verse = crate::scripture::Verse {
                tick: world.tick(),
                kind: crate::scripture::VerseKind::Protest,
                polarity: crate::scripture::Polarity::Silence,
                witness: "the elders".to_string(),
                place,
                place_tile: 0,
                people: world.cultures[ci].name.clone(),
                other: String::new(),
                witness_id: None,
                mutation: 0,
            };
            world.cultures[ci].scripture.push(verse);
        }

        if unrest >= 0.65 {
            world.events.push(riot);
            riot_in(world, polity_id);
        }
    }
}

/// Riot: -2% population in the polity's largest settlement, one building
/// damaged.
fn riot_in(world: &mut World, polity_id: u32) {
    let largest_settlement = world
        .polities
        .iter()
        .find(|p| p.id == polity_id)
        .and_then(|p| {
            p.settlements
                .iter()
                .filter_map(|&sid| world.settlements.iter().find(|s| s.id == sid))
                .max_by(|a, b| a.population.total_cmp(&b.population))
                .map(|s| s.id)
        });
    if let Some(sid) = largest_settlement
        && let Some(settlement) = world.settlements.iter_mut().find(|s| s.id == sid)
    {
        // Remove 2% of the settlement's population from its catchment.
        let loss = settlement.population * 0.02;
        // Distribute the loss across catchment tiles.
        let tiles = settlement.catchment.clone();
        let per_tile = loss / tiles.len().max(1) as f32;
        for t in &tiles {
            world.population[*t] = (world.population[*t] - per_tile).max(0.0);
        }
        settlement.population = settlement
            .catchment
            .iter()
            .map(|&t| world.population[t])
            .sum();
        // Damage one building.
        for slot in &mut settlement.slots {
            if let Some(b) = slot
                && b.level > 0
            {
                b.level = b.level.saturating_sub(1);
                break;
            }
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
