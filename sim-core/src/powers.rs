//! The six powers (design, Powers): their footprints, who witnesses them, what
//! they do to the land and the people, and the 5-tick outcome window.
//!
//! A power never applies immediately: the player's call enqueues an action, and
//! the next tick applies it before Climate (design, Player actions are queued).

use crate::culture::NO_CULTURE;
use crate::scripture::{Polarity, Verse};
use crate::world::{GRID_HEIGHT, GRID_WIDTH, World};
use crate::worldgen::Biome;

/// The first playable's six powers (design, Powers).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Power {
    /// Lift land out of the sea.
    Raise,
    /// Sink land into it.
    Lower,
    /// Repaint the biome under the brush.
    Paint(Biome),
    /// Fertility and plenty for ten years.
    Bless,
    /// A rock from the sky.
    Meteor,
    /// A wind that kills.
    Tornado,
    /// Beasts that hunt the local people.
    Spawn,
}

impl Power {
    /// Brush radius in tiles. Raise/Lower and Paint are drag brushes whose size
    /// the UI grows with - and =; everything else is a single 3x3 impact.
    pub fn footprint_radius(self) -> i32 {
        1
    }

    /// Tiles around the footprint that see the act (design, Witness rule).
    pub fn visibility_radius(self) -> i32 {
        match self {
            Power::Meteor => 6,
            Power::Tornado => 3,
            Power::Raise | Power::Lower => 2,
            Power::Spawn => 2,
            Power::Bless | Power::Paint(_) => 1,
        }
    }

    /// A meteor promotes two witnesses; everything else promotes one.
    pub fn witnesses_promoted(self) -> usize {
        match self {
            Power::Meteor => 2,
            _ => 1,
        }
    }

    /// How the people are expected to read it before the outcome lands.
    pub fn intent(self) -> Polarity {
        match self {
            Power::Meteor | Power::Tornado | Power::Lower | Power::Spawn => Polarity::Wrath,
            Power::Bless | Power::Raise => Polarity::Mercy,
            Power::Paint(biome) => match biome {
                Biome::Grassland | Biome::Forest | Biome::Beach => Polarity::Mercy,
                _ => Polarity::Wrath,
            },
        }
    }

    /// Maps a toolbar slot to a power. Slot 2's paint cycles its biome in the
    /// UI; the sim takes whichever biome the UI hands it.
    pub fn from_index(index: i64) -> Power {
        match index {
            0 => Power::Raise,
            1 => Power::Lower,
            2 => Power::Paint(Biome::Grassland),
            3 => Power::Bless,
            4 => Power::Meteor,
            5 => Power::Tornado,
            _ => Power::Spawn,
        }
    }

    /// Stable id for the UI and the action log.
    pub fn name(self) -> &'static str {
        match self {
            Power::Raise => "raise",
            Power::Lower => "lower",
            Power::Paint(_) => "paint",
            Power::Bless => "bless",
            Power::Meteor => "meteor",
            Power::Tornado => "tornado",
            Power::Spawn => "spawn",
        }
    }
}

/// One queued player action: what, where, and the tick it takes effect on.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Action {
    pub tick: u64,
    pub power: Power,
    pub x: usize,
    pub y: usize,
}

/// A witness tile waiting for its 5-tick outcome window to close.
#[derive(Copy, Clone, Debug)]
pub struct PendingOutcome {
    pub tile: usize,
    pub until: u64,
    pub pre_population: f32,
    pub pre_capacity: f32,
    /// Δpopulation% the tile's own 10-tick trend predicted for the window.
    pub expected: f32,
    pub culture_id: u16,
}

/// Applies one power at `(cx, cy)`.
///
/// Witness snapshots are taken before the effect lands, so every score is
/// measured against the world as it was.
pub fn apply(world: &mut World, power: Power, cx: usize, cy: usize) {
    let footprint = tiles_around(cx, cy, power.footprint_radius());
    let watched: Vec<usize> = {
        let footprint: Vec<usize> = footprint.clone();
        let mut all = footprint.clone();
        for tile in tiles_around(cx, cy, power.visibility_radius()) {
            if !all.contains(&tile) {
                all.push(tile);
            }
        }
        all
    };

    // Snapshot who can see it, before anything changes.
    let mut snapshots = Vec::new();
    for &tile in &watched {
        if world.population[tile] < 1.0 || world.culture_id[tile] == NO_CULTURE {
            continue;
        }
        snapshots.push((
            tile,
            world.population[tile],
            world.carrying_capacity[tile],
            world.expected_growth(tile),
            world.culture_id[tile],
        ));
    }

    let witnesses = promote(world, &footprint, power.witnesses_promoted());

    for &tile in &footprint {
        match power {
            Power::Raise => world.shift_elevation(tile, 0.05),
            Power::Lower => world.shift_elevation(tile, -0.05),
            Power::Paint(biome) => world.set_biome(tile, biome),
            Power::Bless => world.bless(tile),
            Power::Meteor => world.meteor(tile),
            Power::Tornado => world.population[tile] *= 0.60,
            Power::Spawn => world.spawn_beasts(tile),
        }
    }

    let until = world.tick() + 5;
    world.pending.extend(snapshots.into_iter().map(
        |(tile, pre_population, pre_capacity, expected, culture_id)| PendingOutcome {
            tile,
            until,
            pre_population,
            pre_capacity,
            expected,
            culture_id,
        },
    ));

    // The people write it down at once, in the words of their own language.
    for witness_id in witnesses {
        world.write_verse(power, witness_id);
    }
}

/// Promotes up to `count` witnesses from the most populous footprint tiles,
/// preferring the Chosen people (design, Promotion).
fn promote(world: &mut World, footprint: &[usize], count: usize) -> Vec<u32> {
    let mut candidates: Vec<usize> = footprint
        .iter()
        .copied()
        .filter(|&tile| world.population[tile] >= 1.0 && world.culture_id[tile] != NO_CULTURE)
        .collect();
    candidates.sort_by(|&a, &b| {
        world.population[b]
            .total_cmp(&world.population[a])
            .then(a.cmp(&b))
    });

    let chosen = |tile: &usize, world: &World| world.is_chosen(world.culture_id[*tile]);
    let mut ordered: Vec<usize> = candidates
        .iter()
        .copied()
        .filter(|t| chosen(t, world))
        .collect();
    ordered.extend(candidates.iter().copied().filter(|t| !chosen(t, world)));

    let mut promoted = Vec::new();
    for tile in ordered {
        if promoted.len() >= count {
            break;
        }
        if let Some(id) = world.promote_witness(tile) {
            promoted.push(id);
        }
    }
    promoted
}

/// Tiles within `radius` of `(cx, cy)`, east-west wrapping, poles clamped.
pub fn tiles_around(cx: usize, cy: usize, radius: i32) -> Vec<usize> {
    let mut tiles = Vec::new();
    for dy in -radius..=radius {
        let y = cy as i32 + dy;
        if y < 0 || y >= GRID_HEIGHT as i32 {
            continue;
        }
        for dx in -radius..=radius {
            let x = (cx as i32 + dx).rem_euclid(GRID_WIDTH as i32);
            tiles.push(y as usize * GRID_WIDTH + x as usize);
        }
    }
    tiles
}

/// Writes the verse for one witness of `power`, if their people still exist.
pub fn verse_for(world: &World, power: Power, witness_id: u32) -> Option<(u16, Verse)> {
    let notable = world.notables.iter().find(|n| n.id == witness_id)?;
    let culture = world.cultures.get(notable.culture_id as usize)?;
    let place = crate::names::place_name(culture, notable.tile as usize);
    Some((
        notable.culture_id,
        Verse {
            tick: world.tick(),
            power,
            polarity: power.intent(),
            witness: notable.name.clone(),
            place,
            people: culture.name.clone(),
            witness_id: Some(witness_id),
            mutation: 0,
            opening: false,
        },
    ))
}

/// Closes every pending outcome window whose five ticks have passed.
pub fn close_windows(world: &mut World) {
    let now = world.tick();
    let mut still_pending = Vec::with_capacity(world.pending.len());
    let closing: Vec<PendingOutcome> = world
        .pending
        .iter()
        .copied()
        .filter(|pending| pending.until <= now)
        .collect();
    still_pending.extend(
        world
            .pending
            .iter()
            .copied()
            .filter(|pending| pending.until > now),
    );
    world.pending = still_pending;

    for pending in closing {
        let actual = if pending.pre_population > 0.0 {
            (world.population[pending.tile] - pending.pre_population) / pending.pre_population
        } else {
            0.0
        };
        let capacity = if pending.pre_capacity > 0.0 {
            (world.carrying_capacity[pending.tile] - pending.pre_capacity) / pending.pre_capacity
        } else {
            0.0
        };
        let score = (actual - pending.expected + capacity).clamp(-1.0, 1.0);
        let current = world.belief_of(pending.tile);
        let updated = if score.abs() < 0.05 {
            current.witness_silence()
        } else {
            current.witness(score)
        };
        world.set_belief(pending.tile, updated);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visibility_radii_match_the_design_table() {
        assert_eq!(Power::Meteor.visibility_radius(), 6);
        assert_eq!(Power::Tornado.visibility_radius(), 3);
        assert_eq!(Power::Raise.visibility_radius(), 2);
        assert_eq!(Power::Lower.visibility_radius(), 2);
        assert_eq!(Power::Spawn.visibility_radius(), 2);
        assert_eq!(Power::Bless.visibility_radius(), 1);
        assert_eq!(Power::Paint(Biome::Forest).visibility_radius(), 1);
    }

    #[test]
    fn a_meteor_promotes_two_witnesses() {
        assert_eq!(Power::Meteor.witnesses_promoted(), 2);
        assert_eq!(Power::Bless.witnesses_promoted(), 1);
    }

    #[test]
    fn a_footprint_wraps_at_the_eastern_edge() {
        let tiles = tiles_around(0, 40, 1);
        assert_eq!(tiles.len(), 9);
        assert!(tiles.contains(&(40 * GRID_WIDTH + GRID_WIDTH - 1)));
    }

    #[test]
    fn footprints_are_clamped_at_the_poles() {
        assert_eq!(tiles_around(5, 0, 1).len(), 6);
    }
}
