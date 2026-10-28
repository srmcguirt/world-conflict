//! Procedural world generation (Milestone 1).
//!
//! Hand-rolled fBm value noise on the wrapping tile lattice: the `noise` crate
//! uses platform float math, which the libm determinism rule (R2-22) does not
//! cover. Hashing is integer math; the only transcendental is `cos` for
//! latitude, via `libm`. Octave scales are powers of two, and 160 = 2^5 * 5,
//! so the map wraps seamlessly east to west.

use libm::cosf;

use crate::world::{GRID_HEIGHT, GRID_WIDTH};

/// fBm octaves. Scale doubles each octave and always divides `GRID_WIDTH`
/// (160 = 2^5 * 5, so scales 4..32), so the map wraps seamlessly east to west.
const OCTAVES: u32 = 4;

/// Coarsest lattice scale: one cell per 4 tiles. Starting at scale 1 would be
/// per-tile white noise, which reads as static instead of terrain.
const FIRST_SCALE: u32 = 4;

/// Normalized elevation at or below which a tile is ocean.
pub const SEA_LEVEL: f32 = 0.5;

/// Above this latitude, land is permanent ice with zero carrying capacity.
pub const ICE_LATITUDE_DEG: f32 = 75.0;

/// Temperature lost per unit of elevation above sea level.
const LAPSE_RATE: f32 = 0.6;

/// Elevation bias below the noise midpoint, so oceans exist on every world.
const LAND_BIAS: f32 = 0.02;

/// Distinct noise streams per field, so elevation and moisture are independent.
const SEED_ELEVATION: u64 = 0x0E1E_7A70_1234_5678;
const SEED_MOISTURE: u64 = 0x4015_7A2E_5598_7654;

/// The six biomes of the first playable (design, Milestone 1).
///
/// `Ice` is not generated: it is the latitude override above ±75°.
#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Biome {
    Ocean,
    Beach,
    Grassland,
    Forest,
    Desert,
    Mountain,
    /// Latitude override above [`ICE_LATITUDE_DEG`], with zero capacity.
    Ice,
}

/// A freshly generated world: one entry per tile, row-major (y * WIDTH + x).
pub struct GeneratedWorld {
    pub elevation: Vec<f32>,
    pub moisture: Vec<f32>,
    pub temperature: Vec<f32>,
    pub biome: Vec<Biome>,
    /// Population the tile can carry, corrected for latitude (design, Latitude correction).
    pub carrying_capacity: Vec<f32>,
}

/// Latitude of a tile row in degrees, +90 at row 0 down to -90.
pub fn latitude_deg(y: usize) -> f32 {
    90.0 - (y as f32 + 0.5) * (180.0 / GRID_HEIGHT as f32)
}

/// Tile area and east-west step cost scale with cos(latitude).
///
/// R2-10: the correction *multiplies* cost by cos(lat) (tiles narrow toward the
/// poles); dividing by it would make polar movement ~3.9x more expensive.
pub fn latitude_area_factor(lat_deg: f32) -> f32 {
    cosf(lat_deg * (core::f32::consts::PI / 180.0))
}

/// Classifies a tile from elevation and moisture.
///
/// `_temperature` is reserved for later tuning; the latitude ice rule is
/// applied by [`generate`] before classification.
pub fn classify(elevation: f32, moisture: f32, _temperature: f32) -> Biome {
    if elevation <= SEA_LEVEL {
        Biome::Ocean
    } else if elevation < SEA_LEVEL + 0.02 {
        Biome::Beach
    } else if elevation > 0.8 {
        Biome::Mountain
    } else if moisture < 0.3 {
        Biome::Desert
    } else if moisture > 0.65 {
        Biome::Forest
    } else {
        Biome::Grassland
    }
}

/// Generates a whole world from `seed`. Same seed, same world.
pub fn generate(seed: u64) -> GeneratedWorld {
    let tiles = GRID_WIDTH * GRID_HEIGHT;
    let mut world = GeneratedWorld {
        elevation: vec![0.0; tiles],
        moisture: vec![0.0; tiles],
        temperature: vec![0.0; tiles],
        biome: vec![Biome::Ocean; tiles],
        carrying_capacity: vec![0.0; tiles],
    };

    for y in 0..GRID_HEIGHT as i32 {
        let lat = latitude_deg(y as usize);
        let area = latitude_area_factor(lat);
        for x in 0..GRID_WIDTH as i32 {
            let i = y as usize * GRID_WIDTH + x as usize;
            let elevation = contrast(fbm(x, y, seed ^ SEED_ELEVATION), 1.25) - LAND_BIAS;
            let moisture = contrast(fbm(x, y, seed ^ SEED_MOISTURE), 1.3);
            let temperature =
                (area - (elevation - SEA_LEVEL).max(0.0) * LAPSE_RATE).clamp(0.0, 1.0);

            let biome = if lat.abs() > ICE_LATITUDE_DEG {
                Biome::Ice
            } else {
                classify(elevation, moisture, temperature)
            };

            world.elevation[i] = elevation;
            world.moisture[i] = moisture;
            world.temperature[i] = temperature;
            world.biome[i] = biome;
            world.carrying_capacity[i] = base_capacity(biome) * area;
        }
    }

    world
}

/// Whether people can settle this biome.
///
/// Mountains and deserts stay empty on purpose: "empty mountains and deserts
/// separate peoples" (design, Membership), and that separation is what makes
/// peoples split along terrain.
pub fn settleable(biome: Biome) -> bool {
    matches!(biome, Biome::Grassland | Biome::Forest | Biome::Beach)
}

/// People a tile of this biome can carry at this latitude.
pub fn capacity_for(biome: Biome, lat_deg: f32) -> f32 {
    base_capacity(biome) * latitude_area_factor(lat_deg)
}

/// Flat RGB for a biome, shared by the biome and culture overlays.
pub fn biome_color(biome: Biome) -> [u8; 3] {
    match biome {
        Biome::Ocean => [26, 62, 110],
        Biome::Beach => [214, 197, 148],
        Biome::Grassland => [124, 148, 84],
        Biome::Forest => [58, 102, 60],
        Biome::Desert => [206, 174, 110],
        Biome::Mountain => [138, 134, 126],
        Biome::Ice => [232, 240, 246],
    }
}

/// RGBA bytes for the biome overlay, one 4-byte pixel per tile.
///
/// Flat biome colors for Milestone 1; the hillshaded relief of the Kinetic
/// Sand direction (design D7) replaces this in the render pass.
pub fn overlay_biome(biomes: &[Biome]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(biomes.len() * 4);
    for &biome in biomes {
        let [r, g, b] = biome_color(biome);
        bytes.extend_from_slice(&[r, g, b, 255]);
    }
    bytes
}

/// People a tile of this biome can carry, before the latitude correction.
fn base_capacity(biome: Biome) -> f32 {
    match biome {
        Biome::Ocean | Biome::Ice => 0.0,
        Biome::Beach => 4.0,
        Biome::Desert => 3.0,
        Biome::Mountain => 2.0,
        Biome::Forest => 8.0,
        Biome::Grassland => 12.0,
    }
}

/// Fractal Brownian motion over [`OCTAVES`] doubling lattice scales.
fn fbm(tx: i32, ty: i32, seed: u64) -> f32 {
    let mut sum = 0.0;
    let mut norm = 0.0;
    let mut amplitude = 0.5;
    let mut scale = FIRST_SCALE;
    for octave in 0..OCTAVES {
        sum += amplitude * lattice_noise(tx, ty, scale, seed.wrapping_add(u64::from(octave)));
        norm += amplitude;
        amplitude *= 0.5;
        scale *= 2;
    }
    sum / norm
}

/// Bilinear value noise on the wrapping tile lattice, one cell per `scale` tiles.
///
/// The east-west axis wraps: lattice columns are taken modulo the cell count,
/// so the map has no seam (design, Grid).
fn lattice_noise(tx: i32, ty: i32, scale: u32, seed: u64) -> f32 {
    let cells = (GRID_WIDTH as i32) / scale as i32;
    let fx = tx as f32 / scale as f32;
    let fy = ty as f32 / scale as f32;
    let lx = fx.floor() as i32;
    let ly = fy.floor() as i32;
    let sx = smoothstep(fx - lx as f32);
    let sy = smoothstep(fy - ly as f32);

    let n = |cx: i32, cy: i32| hash2(cx.rem_euclid(cells), cy, seed) as f32 / u32::MAX as f32;
    let n00 = n(lx, ly);
    let n10 = n(lx + 1, ly);
    let n01 = n(lx, ly + 1);
    let n11 = n(lx + 1, ly + 1);

    let top = n00 + (n10 - n00) * sx;
    let bottom = n01 + (n11 - n01) * sx;
    top + (bottom - top) * sy
}

fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Pushes a [0, 1] noise value away from its midpoint.
///
/// Smooth fBm concentrates near 0.5, which would make almost every land tile
/// grassland; a mild contrast gives forests, deserts and mountains distinct
/// bands (tuning knob for `worldsim`).
fn contrast(value: f32, amount: f32) -> f32 {
    ((value - 0.5) * amount + 0.5).clamp(0.0, 1.0)
}

/// Deterministic 2D integer hash (splitmix64 finalizer). Integer math only.
fn hash2(x: i32, y: i32, seed: u64) -> u32 {
    let mut h = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    h = mix(h ^ (i64::from(x) as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    h = mix(h ^ (i64::from(y) as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F));
    (h >> 32) as u32
}

fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
