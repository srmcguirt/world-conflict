//! Kingdom events for the UI (design, Architecture → typed events; R4; D11).
//!
//! The sim records what happened in its kingdoms — a settlement growing or
//! shrinking a tier, a building finished, a kingdom founded, a government
//! changed, a protest, a riot — and the UI drains them into the event log and
//! the polity panel. Nothing durable depends on them: scripture lives in the
//! cultures, so a dropped event loses no history (R4).

use std::collections::VecDeque;

use crate::buildings;
use crate::names;
use crate::society::Government;
use crate::world::World;

/// The UI queue holds at most this many undrained events; the oldest drop
/// first and are counted (R4).
pub const QUEUE_CAP: usize = 4_096;

/// What happened.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    TierUp { tier: u8 },
    TierDown { tier: u8 },
    BuildingFinished { line: u8, level: u8 },
    KingdomFounded,
    GovernmentChanged { from: Government, to: Government },
    Protest,
    Riot,
}

/// One event: what, when, whose and where.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub tick: u64,
    pub kind: EventKind,
    pub culture_id: u16,
    /// The tile the camera should fly to.
    pub tile: usize,
    /// The polity it concerns, if any.
    pub polity_id: Option<u32>,
}

/// The bounded queue between the sim and the UI (R4).
#[derive(Clone, Debug, Default)]
pub struct EventQueue {
    events: VecDeque<Event>,
    dropped: u64,
}

impl EventQueue {
    pub fn push(&mut self, event: Event) {
        if self.events.len() == QUEUE_CAP {
            self.events.pop_front();
            self.dropped += 1;
        }
        self.events.push_back(event);
    }

    /// Everything undrained, oldest first, plus how many were dropped since
    /// the last drain.
    pub fn drain(&mut self) -> (Vec<Event>, u64) {
        let dropped = std::mem::take(&mut self.dropped);
        (self.events.drain(..).collect(), dropped)
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// What the kingdoms looked like before a 25-tick check, so the check's
/// changes can be read off afterwards without threading events through
/// every system.
pub struct KingdomSnapshot {
    settlements: Vec<SettlementShape>,
    /// Polity id and government.
    polities: Vec<(u32, Government)>,
}

/// One settlement as the snapshot saw it: id, tier, and the (line, level) of
/// every finished building.
type SettlementShape = (u32, u8, Vec<(u8, u8)>);

pub fn snapshot(world: &World) -> KingdomSnapshot {
    KingdomSnapshot {
        settlements: world
            .settlements()
            .iter()
            .map(|s| {
                let built = s
                    .slots
                    .iter()
                    .flatten()
                    .map(|b| (b.line, b.level))
                    .collect();
                (s.id, s.tier, built)
            })
            .collect(),
        polities: world
            .polities()
            .iter()
            .map(|p| (p.id, p.government))
            .collect(),
    }
}

/// Records every tier change, finished building, new kingdom and change of
/// government since `before`.
pub fn record_changes(world: &mut World, before: &KingdomSnapshot) {
    let tick = world.tick();
    let mut found = Vec::new();
    for settlement in world.settlements() {
        let polity_id = crate::society::polity_of_settlement(world, settlement.id);
        let event = |kind| Event {
            tick,
            kind,
            culture_id: settlement.culture_id,
            tile: settlement.tile,
            polity_id,
        };
        let old = before
            .settlements
            .iter()
            .find(|(id, _, _)| *id == settlement.id);
        let (old_tier, old_built) =
            old.map_or((0, &[][..]), |(_, tier, built)| (*tier, &built[..]));
        if settlement.tier > old_tier {
            found.push(event(EventKind::TierUp {
                tier: settlement.tier,
            }));
        } else if settlement.tier < old_tier {
            found.push(event(EventKind::TierDown {
                tier: settlement.tier,
            }));
        }
        for building in settlement.slots.iter().flatten() {
            let key = (building.line, building.level);
            if !old_built.contains(&key) {
                found.push(event(EventKind::BuildingFinished {
                    line: building.line,
                    level: building.level,
                }));
            }
        }
    }
    for polity in world.polities() {
        let tile = capital_tile(world, polity.capital);
        let event = |kind| Event {
            tick,
            kind,
            culture_id: polity.culture_id,
            tile,
            polity_id: Some(polity.id),
        };
        match before.polities.iter().find(|(id, _)| *id == polity.id) {
            None => found.push(event(EventKind::KingdomFounded)),
            Some(&(_, from)) if from != polity.government => {
                found.push(event(EventKind::GovernmentChanged {
                    from,
                    to: polity.government,
                }));
            }
            Some(_) => {}
        }
    }
    for event in found {
        world.events.push(event);
    }
}

pub fn capital_tile(world: &World, settlement_id: u32) -> usize {
    world
        .settlements()
        .iter()
        .find(|s| s.id == settlement_id)
        .map_or(0, |s| s.tile)
}

const TIERS: [&str; 4] = ["a camp", "a village", "a town", "a city"];

/// The event in words, for the log (`GDScript` only displays what it's given).
pub fn describe(world: &World, event: &Event) -> String {
    let Some(culture) = world.cultures().get(event.culture_id as usize) else {
        return String::new();
    };
    let place = names::place_name(culture, event.tile);
    let people = &culture.name;
    match event.kind {
        EventKind::TierUp { tier } => {
            format!(
                "{place} of the {people} grew into {}",
                TIERS[usize::from(tier.min(3))]
            )
        }
        EventKind::TierDown { tier } => {
            format!(
                "{place} of the {people} shrank to {}",
                TIERS[usize::from(tier.min(3))]
            )
        }
        EventKind::BuildingFinished { line, level } => {
            let catalog = buildings::load_catalog();
            let name = catalog
                .lines
                .iter()
                .find(|l| l.id == line)
                .and_then(|l| l.levels.get(usize::from(level)))
                .map_or("a building", |l| l.name.as_str());
            // Catalog names are ids ("irrigation_works"); the log wants words.
            format!(
                "{place} of the {people} finished {}",
                name.replace('_', " ")
            )
        }
        EventKind::KingdomFounded => format!("The {people} of {place} became a kingdom"),
        EventKind::GovernmentChanged { to, .. } => {
            format!("The kingdom of {place} became a {}", to.name())
        }
        EventKind::Protest => format!("The people of {place} protested"),
        EventKind::Riot => format!("Riots in {place}: people lost, a building damaged"),
    }
}

/// The short label the log and panel group events by.
pub fn label(kind: EventKind) -> &'static str {
    match kind {
        EventKind::TierUp { .. } => "tier_up",
        EventKind::TierDown { .. } => "tier_down",
        EventKind::BuildingFinished { .. } => "building",
        EventKind::KingdomFounded => "kingdom",
        EventKind::GovernmentChanged { .. } => "government",
        EventKind::Protest => "protest",
        EventKind::Riot => "riot",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(tick: u64) -> Event {
        Event {
            tick,
            kind: EventKind::Protest,
            culture_id: 0,
            tile: 0,
            polity_id: None,
        }
    }

    #[test]
    fn the_queue_keeps_the_newest_4096_and_counts_the_rest() {
        // R4: 5,000 pushed -> 4,096 kept, 904 dropped, oldest first.
        let mut queue = EventQueue::default();
        for tick in 0..5_000 {
            queue.push(event(tick));
        }
        let (events, dropped) = queue.drain();
        assert_eq!(events.len(), QUEUE_CAP);
        assert_eq!(dropped, 904);
        assert_eq!(events[0].tick, 904, "the oldest are the ones dropped");
        assert_eq!(events[QUEUE_CAP - 1].tick, 4_999);
    }

    #[test]
    fn a_drain_empties_the_queue_and_resets_the_count() {
        let mut queue = EventQueue::default();
        for tick in 0..5_000 {
            queue.push(event(tick));
        }
        queue.drain();
        assert!(queue.is_empty());
        queue.push(event(1));
        let (events, dropped) = queue.drain();
        assert_eq!(events.len(), 1);
        assert_eq!(dropped, 0);
    }
}
