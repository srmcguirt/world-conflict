//! Physical adaptation (design, Physical adaptation).
//!
//! Every tile holds its people's mean traits: skin, stature, cold tolerance
//! and lung capacity, each in [0, 1]. Children resemble the environment after
//! generations of small drift, which gives peoples a face of their own and a
//! small edge in the country that shaped them.
//!
//! The rule the design is strict about: adaptation gives *cosmetic
//! differences and small regional perks, and never a ranking of better or
//! worse peoples*. Skin and stature are purely how people look; only cold and
//! altitude touch growth, and only in the country they suit.

use crate::world::World;
use crate::worldgen::{self, SEA_LEVEL};

/// Trait axis indices in the flattened per-tile field.
pub const SKIN: usize = 0;
pub const STATURE: usize = 1;
pub const COLD: usize = 2;
pub const ALTITUDE: usize = 3;
pub const TRAITS: usize = 4;

/// Starting value of every trait: a blank slate the environment writes on.
pub const START: f32 = 0.5;

/// Trait drift per decade toward the environment's optimum.
pub const ADAPT_RATE: f32 = 0.05;

/// A fully cold-adapted people gains this much growth in cold country.
pub const COLD_PERK: f32 = 0.10;
/// Temperature below which the cold perk applies.
pub const COLD_LIMIT: f32 = 0.35;

/// A fully highland-adapted people gains this much growth at altitude.
pub const ALT_PERK: f32 = 0.10;
/// Elevation (normalized) above which the lung perk applies.
pub const ALT_LIMIT: f32 = 0.62;
pub const PORTRAIT_W: usize = 12;
pub const PORTRAIT_H: usize = 16;

/// The environment's ideal for each trait at one tile.
///
/// Skin tracks UV (the design maps it to latitude); stature and cold
/// tolerance track temperature; lungs track elevation.
pub fn optimum(elevation: f32, temperature: f32, latitude_deg: f32) -> [f32; TRAITS] {
    let uv = worldgen::latitude_area_factor(latitude_deg).clamp(0.0, 1.0);
    let land = ((elevation - SEA_LEVEL) / (1.0 - SEA_LEVEL)).clamp(0.0, 1.0);
    [uv, 1.0 - temperature, 1.0 - temperature, land]
}

/// Grows a tile's people toward the environment, one decade at a time.
///
/// A little hash noise keeps the drift from being too clean: lineages differ
/// even under the same sun.
pub fn tick_adaptation(world: &mut World) {
    for tile in 0..world.population.len() {
        if world.population[tile] < 1.0 {
            continue;
        }
        let latitude = worldgen::latitude_deg(tile / crate::world::GRID_WIDTH);
        let target = optimum(world.elevation[tile], world.temperature[tile], latitude);
        if !worldgen::settleable(world.biome[tile]) {
            continue;
        }
        let start = tile * TRAITS;
        let noise = (crate::names::hash(&format!("{}-{}", tick_decade(world), tile)) % 1000) as f32
            / 1000.0
            - 0.5;
        for (axis, slot) in world.traits[start..start + TRAITS].iter_mut().enumerate() {
            let drift = (target[axis] - *slot) * (ADAPT_RATE + noise.abs() * 0.002);
            *slot = (*slot + drift).clamp(0.0, 1.0);
        }
    }
}

fn tick_decade(world: &World) -> u64 {
    world.tick() / 10
}

/// The growth perk a tile's adapted bodies earn: cold tolerance in cold
/// country, lungs at altitude. Skin and stature change nothing.
pub fn perk(world: &World, tile: usize, growth: f32) -> f32 {
    let start = tile * TRAITS;
    let mut factor = 1.0;
    if world.temperature[tile] < COLD_LIMIT {
        factor += COLD_PERK * world.traits[start + COLD];
    }
    if world.elevation[tile] > SEA_LEVEL + ALT_LIMIT * (1.0 - SEA_LEVEL) {
        factor += ALT_PERK * world.traits[start + ALTITUDE];
    }
    growth * factor
}

/// Skin tone as a color: pale in the north, deep brown at the equator.
pub fn skin_color(skin: f32) -> [u8; 3] {
    let skin = skin.clamp(0.0, 1.0);
    let pale = [226_u8, 203, 180];
    let deep = [92_u8, 61, 43];
    [
        (f32::from(pale[0]) + (f32::from(deep[0]) - f32::from(pale[0])) * skin) as u8,
        (f32::from(pale[1]) + (f32::from(deep[1]) - f32::from(pale[1])) * skin) as u8,
        (f32::from(pale[2]) + (f32::from(deep[2]) - f32::from(pale[2])) * skin) as u8,
    ]
}

/// A people's dress color: the culture's banner color warmed by their skin.
pub fn dress_of(world: &World, culture_id: u16) -> [u8; 3] {
    let Some(culture) = world.cultures().get(culture_id as usize) else {
        return [120, 110, 100];
    };
    let mut banner = crate::culture::color(culture.id);
    let skin = skin_color(culture.mean_skin);
    for (mix, tint) in banner.iter_mut().zip(skin) {
        *mix = (f32::from(*mix) * 0.65 + f32::from(tint) * 0.35) as u8;
    }
    banner
}

/// A generated pixel portrait, RGBA, [`PORTRAIT_W`] x [`PORTRAIT_H`].
///
/// The face comes from the person's traits: skin tone, build from stature.
/// The rest comes deterministically from who they are: hair rolls by id, and
/// the cloak wears their people's dress color.
pub fn portrait(notable: &crate::notables::Notable, dress: [u8; 3]) -> Vec<u8> {
    let skin = skin_color(notable.skin);
    let hair_roll = (notable.id as usize * 31 + 7) % 3;
    let hair = [
        (u32::from(dress[0]) * 7 / 10) as u8,
        (u32::from(dress[1]) * 7 / 10) as u8,
        (u32::from(dress[2]) * 7 / 10) as u8,
    ];
    // Build widens the shoulders: broad or narrow, never better.
    let shoulder = 6 + (notable.stature * 3.0).round() as usize;
    let center = PORTRAIT_W / 2;

    let mut pixels = vec![0_u8; PORTRAIT_W * PORTRAIT_H * 4];
    let mut fill = |x: usize, y: usize, color: [u8; 3]| {
        if x >= PORTRAIT_W || y >= PORTRAIT_H {
            return;
        }
        let index = (y * PORTRAIT_W + x) * 4;
        pixels[index..index + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
    };

    for y in 0..PORTRAIT_H {
        for x in 0..PORTRAIT_W {
            fill(x, y, [18, 17, 16]); // HUD matte ground
        }
    }
    // Hair and face.
    for y in 0..6_usize {
        for x in (center - 3)..(center + 3) {
            if y < 2 || (y == 2 && (x + hair_roll).is_multiple_of(2)) {
                fill(x, y, hair);
            } else {
                fill(x, y, skin);
            }
        }
    }
    // Eyes.
    for x in [center - 2, center + 1] {
        fill(x, 4, [40, 32, 28]);
    }
    // Shoulders and cloak.
    let top: usize = 6;
    for y in top..PORTRAIT_H {
        let spread = if y == top {
            shoulder / 2
        } else {
            shoulder / 2 + 1
        };
        for x in (center - spread)..(center + spread) {
            let hem = y >= PORTRAIT_H - 2;
            if hem {
                let darker = [
                    (u32::from(dress[0]) * 3 / 4) as u8,
                    (u32::from(dress[1]) * 3 / 4) as u8,
                    (u32::from(dress[2]) * 3 / 4) as u8,
                ];
                fill(x, y, darker);
            } else {
                fill(x, y, dress);
            }
        }
        // Hands at the sides of the cloak.
        if y >= top + 8 {
            fill(center - spread - 1, y, skin);
            fill(center + spread, y, skin);
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optima_match_the_design_table() {
        // Equator: dark skin; poles: pale.
        let equator = optimum(0.4, 0.8, 0.0);
        let pole = optimum(0.4, 0.2, 75.0);
        assert!(equator[SKIN] > 0.9);
        assert!(pole[SKIN] < 0.3);
        // Cold country: sturdy and cold-tolerant; hot country: neither-axis
        // preference beyond the climate's pull.
        assert!(pole[STATURE] > equator[STATURE]);
        assert!(pole[COLD] > equator[COLD]);
        // Highlands: lungs; lowlands: little.
        assert!(optimum(0.95, 0.5, 30.0)[ALTITUDE] > 0.8);
        assert!(optimum(SEA_LEVEL + 0.01, 0.5, 30.0)[ALTITUDE] < 0.05);
    }

    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "no perk returns the base unchanged, bit for bit"
    )]
    fn perks_apply_only_where_the_country_is_hard() {
        let mut world = World::new(1);
        let (cold_tile, warm_tile) = {
            let mut cold = None;
            let mut warm = None;
            for tile in 0..world.population.len() {
                if world.temperature[tile] < COLD_LIMIT && cold.is_none() {
                    cold = Some(tile);
                }
                if world.temperature[tile] > 0.7 && warm.is_none() {
                    warm = Some(tile);
                }
            }
            (cold.unwrap(), warm.unwrap())
        };

        // Bare any adaptation first, so the tile's elevation can't sneak in.
        world.traits[cold_tile * TRAITS + COLD] = 0.0;
        world.traits[cold_tile * TRAITS + ALTITUDE] = 0.0;
        assert_eq!(
            perk(&world, cold_tile, 1.0),
            1.0,
            "the unadapted earn nothing"
        );
        world.traits[cold_tile * TRAITS + COLD] = 1.0;
        let adapted = perk(&world, cold_tile, 1.0);
        assert!((adapted - 1.10).abs() < 0.001, "cold bodies earn the perk");
        assert_eq!(perk(&world, warm_tile, 1.0), 1.0, "looks cost nothing");
    }

    #[test]
    fn skin_reads_as_a_warm_dark_ramp() {
        let pale = skin_color(0.0);
        let dark = skin_color(1.0);
        assert!(pale[0] > 200);
        assert!(dark[2] < dark[0], "brown reads warm");
    }
}
