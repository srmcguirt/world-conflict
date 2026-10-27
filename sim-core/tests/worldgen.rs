//! Worldgen contract tests (Milestone 1).
//!
//! Written before the implementation (TDD). These lock the design contracts:
//! same seed -> same world, latitude-driven temperature and area correction,
//! permanent ice above ±75°, biome classification table, and the RGBA overlay.
#![expect(
    clippy::float_cmp,
    reason = "these contracts compare exact values: untouched land, infinite cost, symmetric tables"
)]

use sim_core::world::{GRID_HEIGHT, GRID_WIDTH};
use sim_core::worldgen::{
    Biome, ICE_LATITUDE_DEG, classify, generate, latitude_area_factor, latitude_deg, overlay_biome,
};

fn tile_f32(field: &[f32], x: usize, y: usize) -> f32 {
    field[y * GRID_WIDTH + x]
}

#[test]
fn same_seed_gives_the_same_world() {
    let a = generate(42);
    let b = generate(42);
    assert_eq!(a.elevation.len(), a.biome.len());
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            assert_eq!(
                tile_f32(&a.elevation, x, y).to_bits(),
                tile_f32(&b.elevation, x, y).to_bits(),
                "elevation differs at ({x}, {y})"
            );
            assert_eq!(a.biome[y * GRID_WIDTH + x], b.biome[y * GRID_WIDTH + x]);
        }
    }

    let c = generate(43);
    let differing = (0..a.elevation.len())
        .filter(|&i| a.elevation[i].to_bits() != c.elevation[i].to_bits())
        .count();
    assert!(
        differing > a.elevation.len() / 2,
        "a different seed should produce a different heightmap"
    );
}

#[test]
fn latitude_runs_from_north_pole_to_south_pole() {
    // Tile centers: the edge rows sit half a tile inside the poles.
    assert!(
        (latitude_deg(0) - 90.0).abs() < 2.0,
        "row 0 is the north polar row"
    );
    let mid = GRID_HEIGHT / 2;
    assert!(latitude_deg(mid).abs() < 2.0, "middle row is the equator");
    assert!(
        (latitude_deg(GRID_HEIGHT - 1) + 90.0).abs() < 2.0,
        "last row is the south polar row"
    );
}

#[test]
fn temperature_falls_from_equator_to_poles() {
    let world = generate(7);
    let equator = GRID_HEIGHT / 2;
    let near_pole = 4;
    assert!(
        tile_f32(&world.temperature, GRID_WIDTH / 2, equator)
            > tile_f32(&world.temperature, GRID_WIDTH / 2, near_pole),
        "equator must be warmer than near-pole at the same elevation class"
    );
}

#[test]
fn area_factor_is_cos_of_latitude() {
    assert!(
        (latitude_area_factor(0.0) - 1.0).abs() < 1e-6,
        "equator = 1.0"
    );
    assert!(
        (latitude_area_factor(60.0) - 0.5).abs() < 1e-3,
        "cos(60) = 0.5"
    );
    assert!(
        latitude_area_factor(80.0) < 0.2,
        "cos(80) ~ 0.17: E-W cost shrinks toward the poles (R2-10: multiply, not divide)"
    );
    assert!((latitude_area_factor(-45.0) - latitude_area_factor(45.0)).abs() < 1e-6);
}

#[test]
fn rows_above_75_degrees_are_ice_with_zero_capacity() {
    let world = generate(11);
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let i = y * GRID_WIDTH + x;
            if latitude_deg(y).abs() > ICE_LATITUDE_DEG {
                assert_eq!(world.biome[i], Biome::Ice, "({x}, {y}) must be ice");
                assert_eq!(
                    world.carrying_capacity[i], 0.0,
                    "ice has zero carrying capacity"
                );
            }
        }
    }
}

#[test]
fn classify_follows_the_biome_table() {
    // Below sea level is always ocean, whatever the moisture.
    assert_eq!(classify(0.2, 0.9, 0.9), Biome::Ocean);
    // Coastline band just above sea level.
    assert_eq!(classify(0.51, 0.5, 0.5), Biome::Beach);
    // High ground is mountain.
    assert_eq!(classify(0.95, 0.5, 0.5), Biome::Mountain);
    // Dry mid-elevation is desert, wet is forest, in between is grassland.
    assert_eq!(classify(0.6, 0.1, 0.5), Biome::Desert);
    assert_eq!(classify(0.6, 0.9, 0.5), Biome::Forest);
    assert_eq!(classify(0.6, 0.5, 0.5), Biome::Grassland);
}

#[test]
fn land_fraction_is_sane_across_seeds() {
    for seed in [1_u64, 2, 3, 99, 12345] {
        let world = generate(seed);
        let land = world.biome.iter().filter(|b| **b != Biome::Ocean).count();
        let fraction = land as f32 / world.biome.len() as f32;
        assert!(
            (0.15..0.75).contains(&fraction),
            "seed {seed}: land fraction {fraction} out of range"
        );
    }
}

#[test]
fn overlay_is_rgba_and_colors_read_as_their_biome() {
    let world = generate(5);
    let bytes = overlay_biome(&world.biome);
    assert_eq!(bytes.len(), GRID_WIDTH * GRID_HEIGHT * 4);

    let at = |x: usize, y: usize| -> (u8, u8, u8) {
        let i = (y * GRID_WIDTH + x) * 4;
        (bytes[i], bytes[i + 1], bytes[i + 2])
    };

    // Find one tile of each kind and check the color reads correctly.
    let mut seen = Vec::new();
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let biome = world.biome[y * GRID_WIDTH + x];
            if seen.contains(&biome) {
                continue;
            }
            seen.push(biome);
            let (r, g, b) = at(x, y);
            match biome {
                Biome::Ocean => assert!(b > r && b > g, "ocean must read blue"),
                Biome::Desert => assert!(r > b && g >= b, "desert must read sandy"),
                Biome::Forest => assert!(g > r && g > b, "forest must read green"),
                Biome::Grassland => assert!(g >= r && g >= b, "grassland reads green-ish"),
                Biome::Mountain => {
                    assert!(r.abs_diff(g) < 24 && g.abs_diff(b) < 24, "mountain is grey");
                }
                Biome::Beach | Biome::Ice => {}
            }
        }
    }
    assert!(seen.len() >= 4, "a generated world should show variety");
}
