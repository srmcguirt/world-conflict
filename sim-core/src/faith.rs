//! Faith: culture-level belief, doctrines, prophets, and schisms (design,
//! Belief model; Simulation contracts -> Schism).
//!
//! Belief lives per tile; doctrine reads only the culture-level value. A
//! doctrine crystallizes when one axis holds above [`DOCTRINE_THRESHOLD`] for
//! two consecutive 25-tick checks, and a schism founds a new faith when a
//! people sits at least [`SCHISM_DISTANCE`] from its faith's founding belief
//! for two consecutive checks.

use crate::belief::{self, Belief};
use crate::notables::Role;
use crate::scripture::{Polarity, Verse, VerseKind};
use crate::world::World;

/// How far above its starting doubt an axis must climb to become doctrine.
pub const DOCTRINE_THRESHOLD: f32 = 0.5;

/// A doctrine must hold this many consecutive checks.
pub const DOCTRINE_PASSES: u8 = 2;

/// Distance from a faith's founding belief that starts a schism (design).
pub const SCHISM_DISTANCE: f32 = 0.4;

/// Peoples diverging in the same check and within this distance share a faith.
pub const SCHISM_SHARE_DISTANCE: f32 = 0.2;

/// A schism must hold this many consecutive checks.
pub const SCHISM_PASSES: u8 = 2;

/// The doctrine a people live by. One per belief axis (design, Belief model);
/// `just` comes from covenants in Milestone 8, not from belief.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Doctrine {
    Wrathful,
    Benevolent,
    Capricious,
    Protective,
    Jealous,
    Absent,
}

impl Doctrine {
    /// The axis this doctrine reads.
    pub fn axis(self) -> usize {
        match self {
            Doctrine::Wrathful => belief::WRATHFUL,
            Doctrine::Benevolent => belief::BENEVOLENT,
            Doctrine::Capricious => belief::CAPRICIOUS,
            Doctrine::Protective => belief::PROTECTIVE,
            Doctrine::Jealous => belief::JEALOUS,
            Doctrine::Absent => belief::ABSENT,
        }
    }

    /// The doctrine an axis stands for.
    pub fn of_axis(axis: usize) -> Doctrine {
        match axis {
            belief::WRATHFUL => Doctrine::Wrathful,
            belief::BENEVOLENT => Doctrine::Benevolent,
            belief::CAPRICIOUS => Doctrine::Capricious,
            belief::PROTECTIVE => Doctrine::Protective,
            belief::JEALOUS => Doctrine::Jealous,
            _ => Doctrine::Absent,
        }
    }

    /// The doctrine's name, for the UI and the overlay legend.
    pub fn name(self) -> &'static str {
        match self {
            Doctrine::Wrathful => "wrathful",
            Doctrine::Benevolent => "benevolent",
            Doctrine::Capricious => "capricious",
            Doctrine::Protective => "protective",
            Doctrine::Jealous => "jealous",
            Doctrine::Absent => "absent",
        }
    }
}

/// One religion: the belief it was founded on, and when.
#[derive(Clone, Debug, PartialEq)]
pub struct Faith {
    pub id: u16,
    pub founded_tick: u64,
    pub founding_belief: Belief,
}

/// Euclidean distance over the six belief axes.
pub fn distance(a: &Belief, b: &Belief) -> f32 {
    let (left, right) = (a.to_axes(), b.to_axes());
    let mut sum = 0.0;
    for axis in 0..belief::AXES {
        let delta = left[axis] - right[axis];
        sum += delta * delta;
    }
    libm::sqrtf(sum)
}

/// Piety: the strength of a people's belief, minus how absent they think the
/// god is (design, Social balance -> Piety).
pub fn piety(value: &Belief) -> f32 {
    let axes = value.to_axes();
    let strength: f32 = axes
        .iter()
        .enumerate()
        .filter(|(axis, _)| *axis != belief::ABSENT)
        .map(|(_, value)| value)
        .sum::<f32>()
        / (belief::AXES - 1) as f32;
    (strength - value.absent).clamp(0.0, 1.0)
}

/// The axis a people clearly lean on, if any: at least 0.35 and ahead of the
/// second axis by 0.05. Below that they are merely unsure.
fn conviction(belief: &Belief) -> Option<usize> {
    // Doubt is not a conviction: nobody preaches that the god is absent, so
    // the absent axis neither sharpens in the telling nor founds a doctrine.
    let mut axes: Vec<(usize, f32)> = belief
        .to_axes()
        .into_iter()
        .enumerate()
        .filter(|(axis, _)| *axis != belief::ABSENT)
        .collect();
    axes.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let (axis, top) = axes[0];
    let second = axes[1].1;
    (top >= 0.35 && top - second >= 0.05).then_some(axis)
}

/// Checks every people for a doctrine. Runs on the 25-tick cadence.
pub fn tick_doctrines(world: &mut World) {
    let mut new_doctrines = Vec::new();
    for culture in &mut world.cultures {
        let dominant = culture.belief.dominant();
        let strength = culture.belief.to_axes()[dominant];
        let doctrine = Doctrine::of_axis(dominant);
        if strength < DOCTRINE_THRESHOLD || culture.doctrine == Some(doctrine) {
            culture.doctrine_passes = 0;
            continue;
        }
        culture.doctrine_passes += 1;
        if culture.doctrine_passes >= DOCTRINE_PASSES {
            culture.doctrine_passes = 0;
            culture.doctrine = Some(doctrine);
            new_doctrines.push((culture.id, doctrine));
        }
    }
    for (culture_id, doctrine) in new_doctrines {
        elevate_prophet(world, culture_id, doctrine);
    }
}

/// Elevates the most salient witness of a people to prophet, or raises one if
/// nobody saw anything. Writes the doctrine's founding verse.
fn elevate_prophet(world: &mut World, culture_id: u16, doctrine: Doctrine) {
    let existing = world
        .notables
        .iter()
        .enumerate()
        .filter(|(_, notable)| notable.culture_id == culture_id && notable.role == Role::Witness)
        .max_by(|(_, a), (_, b)| a.salience.total_cmp(&b.salience))
        .map(|(index, _)| index);

    let witness_id = if let Some(index) = existing {
        world.notables[index].role = Role::Prophet;
        world.notables[index].id
    } else {
        let Some(tile) = most_populous_tile(world, culture_id) else {
            return;
        };
        let Some(id) = world.promote_witness(tile) else {
            return;
        };
        if let Some(index) = world.notables.iter().position(|n| n.id == id) {
            world.notables[index].role = Role::Prophet;
        }
        id
    };

    world.write_verse_kind(VerseKind::Doctrine(doctrine), witness_id);
}

/// The most populous tile of a people.
fn most_populous_tile(world: &World, culture_id: u16) -> Option<usize> {
    (0..world.population.len())
        .filter(|&tile| world.culture_id[tile] == culture_id && world.population[tile] >= 1.0)
        .max_by(|&a, &b| world.population[a].total_cmp(&world.population[b]))
}

/// Checks every people for a schism. Runs on the 25-tick cadence.
pub fn tick_schisms(world: &mut World) {
    let faiths = world.faiths.clone();
    let mut divergent = Vec::new();

    for culture in &mut world.cultures {
        let Some(faith) = faiths.get(culture.faith_id as usize) else {
            continue;
        };
        if distance(&culture.belief, &faith.founding_belief) >= SCHISM_DISTANCE {
            culture.schism_passes += 1;
            if culture.schism_passes >= SCHISM_PASSES {
                culture.schism_passes = 0;
                divergent.push(culture.id);
            }
        } else {
            culture.schism_passes = 0;
        }
    }
    if divergent.is_empty() {
        return;
    }

    // Peoples who diverged in the same check and see nearly the same god share
    // one new faith (design, Schism).
    let mut groups: Vec<Vec<u16>> = Vec::new();
    for culture_id in divergent {
        let belief = world.cultures[culture_id as usize].belief;
        let mut joined = false;
        for group in &mut groups {
            let first = world.cultures[group[0] as usize].belief;
            if distance(&belief, &first) <= SCHISM_SHARE_DISTANCE {
                group.push(culture_id);
                joined = true;
                break;
            }
        }
        if !joined {
            groups.push(vec![culture_id]);
        }
    }

    for group in groups {
        let founding = world.cultures[group[0] as usize].belief;
        let id = world.faiths.len() as u16;
        world.faiths.push(Faith {
            id,
            founded_tick: world.tick(),
            founding_belief: founding,
        });
        for culture_id in group {
            world.cultures[culture_id as usize].faith_id = id;
            let witness = world
                .notables
                .iter()
                .filter(|notable| notable.culture_id == culture_id)
                .max_by(|a, b| a.salience.total_cmp(&b.salience))
                .map(|notable| (notable.id, notable.name.clone(), notable.tile as usize));
            let verse = Verse {
                tick: world.tick(),
                kind: VerseKind::Doctrine(Doctrine::of_axis(founding.dominant())),
                polarity: Polarity::Silence,
                witness: witness
                    .as_ref()
                    .map_or_else(|| "the elders".to_string(), |(_, name, _)| name.clone()),
                place: witness
                    .as_ref()
                    .map(|(_, _, tile)| {
                        crate::names::place_name(&world.cultures[culture_id as usize], *tile)
                    })
                    .unwrap_or_default(),
                place_tile: witness
                    .as_ref()
                    .map_or(u32::MAX, |(_, _, tile)| *tile as u32),
                people: world.cultures[culture_id as usize].name.clone(),
                other: String::new(),
                witness_id: witness.map(|(id, _, _)| id),
                mutation: 0,
            };
            world.cultures[culture_id as usize].scripture.push(verse);
        }
    }
}

/// How fast testimony spreads within a people each year.
pub const TESTIMONY_SPREAD: f32 = 0.03;

/// How much the dominant belief grows each year without new evidence: the
/// story grows in the telling (design, scripture as an unreliable narrator).
pub const TESTIMONY_GROWTH: f32 = 0.004;

/// A wrathful god takes a share of the people each decade (design, doctrine
/// side effects: sacrifice).
pub const SACRIFICE: f32 = 0.98;

/// Belief spreads through a people and grows in the telling.
///
/// A witness's testimony reaches their neighbors and the story sharpens with
/// every retelling, so a single act can become a doctrine after a generation.
/// Runs on the 10-tick aggregate cadence, with decade-scaled rates.
pub fn tick_testimony(world: &mut World) {
    let means: Vec<Belief> = world
        .cultures
        .iter()
        .map(|culture| culture.belief)
        .collect();
    // A story only sharpens when the people already lean one way: a doubtful
    // people does not talk itself into a doctrine.
    let dominants: Vec<Option<usize>> = means.iter().map(conviction).collect();
    let spread = 1.0 - (1.0 - TESTIMONY_SPREAD).powi(10);
    let growth = TESTIMONY_GROWTH * 10.0;

    for tile in 0..world.population.len() {
        let id = world.culture_id[tile];
        if id == crate::culture::NO_CULTURE || world.population[tile] < 1.0 {
            continue;
        }
        let Some(mean) = means.get(id as usize) else {
            continue;
        };
        let dominant = dominants[id as usize];
        let start = tile * belief::AXES;
        for axis in 0..belief::AXES {
            let mut value = world.belief[start + axis];
            value += (mean.to_axes()[axis] - value) * spread;
            if dominant == Some(axis) {
                value += growth;
            }
            world.belief[start + axis] = value.clamp(0.0, 1.0);
        }
    }
}

/// Applies the doctrine effects that exist before buildings do. Runs on the
/// 10-tick aggregate cadence.
pub fn tick_doctrine_effects(world: &mut World) {
    let wrathful: Vec<u16> = world
        .cultures
        .iter()
        .filter(|culture| culture.doctrine == Some(Doctrine::Wrathful))
        .map(|culture| culture.id)
        .collect();
    if wrathful.is_empty() {
        return;
    }
    for tile in 0..world.population.len() {
        if wrathful.contains(&world.culture_id[tile]) && world.population[tile] >= 1.0 {
            world.population[tile] *= SACRIFICE;
        }
    }
}

/// Anoints a people: the god turns to them and away from the old Chosen.
pub fn anoint(world: &mut World, culture_id: u16) {
    if world.cultures.get(culture_id as usize).is_none() {
        return;
    }
    let Some(old) = world
        .cultures
        .iter()
        .find(|culture| culture.favor == crate::culture::Favor::Chosen)
        .map(|culture| culture.id)
    else {
        return;
    };
    if old == culture_id {
        return;
    }

    let old_name = world.cultures[old as usize].name.clone();
    let new_name = world.cultures[culture_id as usize].name.clone();

    world.cultures[old as usize].favor = crate::culture::Favor::Other;
    world.cultures[culture_id as usize].favor = crate::culture::Favor::Chosen;
    world.cultures[old as usize].belief.jealous =
        (world.cultures[old as usize].belief.jealous + 0.15).min(1.0);

    // Both scriptures record it.
    for (id, kind, other) in [
        (old, VerseKind::Anoint { chosen: false }, new_name),
        (culture_id, VerseKind::Anoint { chosen: true }, old_name),
    ] {
        let witness = world
            .notables
            .iter()
            .filter(|notable| notable.culture_id == id)
            .max_by(|a, b| a.salience.total_cmp(&b.salience))
            .map(|notable| (notable.id, notable.name.clone(), notable.tile as usize));
        let verse = Verse {
            tick: world.tick(),
            kind,
            polarity: Polarity::Silence,
            witness: witness
                .as_ref()
                .map_or_else(|| "the elders".to_string(), |(_, name, _)| name.clone()),
            place: witness
                .as_ref()
                .map(|(_, _, tile)| crate::names::place_name(&world.cultures[id as usize], *tile))
                .unwrap_or_default(),
            place_tile: witness
                .as_ref()
                .map_or(u32::MAX, |(_, _, tile)| *tile as u32),
            people: world.cultures[id as usize].name.clone(),
            other,
            witness_id: witness.map(|(id, _, _)| id),
            mutation: 0,
        };
        world.cultures[id as usize].scripture.push(verse);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctrine_axes_round_trip() {
        for axis in 0..belief::AXES {
            assert_eq!(Doctrine::of_axis(axis).axis(), axis);
        }
    }

    #[test]
    fn piety_rises_with_conviction_and_falls_with_doubt() {
        let convicted = Belief {
            wrathful: 1.0,
            benevolent: 0.5,
            capricious: 0.5,
            protective: 0.5,
            jealous: 0.5,
            absent: 0.0,
        };
        let doubting = Belief {
            absent: 1.0,
            ..convicted
        };
        assert!(piety(&convicted) > piety(&doubting));
        assert!((0.0..=1.0).contains(&piety(&doubting)));
    }

    #[test]
    #[expect(clippy::float_cmp, reason = "distance to itself is exactly zero")]
    fn distance_is_zero_only_for_the_same_belief() {
        assert_eq!(distance(&Belief::START, &Belief::START), 0.0);
        assert!(distance(&Belief::START, &Belief::START.witness(-1.0)) > 0.0);
    }
}
