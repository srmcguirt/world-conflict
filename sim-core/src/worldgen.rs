//! Procedural world generation (Milestone 1; continents from plates).
//!
//! Land comes from a supercontinent breaking up (D25, D26): rifts start at
//! random points (the borders of a dozen plates), and each plate carries its
//! piece across the sphere to the starting map. Where pieces collide they
//! fold into ranges, a continent riding over ocean floor raises a coastal
//! range, two ocean floors raise island arcs, and where pieces part young
//! ocean opens, so facing coasts across a rift still match. Hotspots fixed in
//! the mantle leave volcanic island chains along each plate's path, and
//! erosion wears ranges into foothills. Hand-rolled fBm
//! value noise then roughens coasts and hills: the `noise` crate uses platform float
//! math, which the libm determinism rule (R2-22) does not cover. Hashing is
//! integer math and every transcendental goes through `libm`. Octave scales
//! are powers of two, and 160 = 2^5 * 5, so the map wraps seamlessly east to
//! west.

use core::f32::consts::TAU;

use libm::{acosf, asinf, atan2f, cosf, sinf, sqrtf};

use crate::hex;
use crate::world::{GRID_HEIGHT, GRID_WIDTH};

/// fBm octaves. Scale doubles each octave and always divides `GRID_WIDTH`
/// (160 = 2^5 * 5, so scales 4..32), so the map wraps seamlessly east to west.
const OCTAVES: u32 = 4;

/// Finest lattice scale: one cell per 4 tiles. Going down to scale 1 would be
/// per-tile white noise, which reads as static instead of terrain.
const FIRST_SCALE: u32 = 4;

/// Normalized elevation at or below which a tile is ocean.
pub const SEA_LEVEL: f32 = 0.5;

/// Above this latitude, land is permanent ice with zero carrying capacity.
pub const ICE_LATITUDE_DEG: f32 = 75.0;

/// Temperature lost per unit of elevation above sea level.
const LAPSE_RATE: f32 = 0.6;

/// Tectonic plates the supercontinent breaks into.
const PLATES: usize = 12;

/// Share of the sphere the supercontinent covers before it breaks up.
const CONTINENTAL_SHARE: f32 = 0.42;

/// Raw height of continental and oceanic crust before boundaries and detail;
/// the shore falls midway between them.
const CONTINENT_HEIGHT: f32 = 0.62;
const OCEAN_HEIGHT: f32 = 0.30;

/// Young ocean floor opened where drifting pieces left a gap: a mid-ocean
/// ridge, higher than old floor but still under water.
const RIDGE_HEIGHT: f32 = 0.36;

/// How far the supercontinent shelves down to the ocean floor, in radians
/// of arc (one tile is about 0.04).
const SHELF_WIDTH: f32 = 0.15;

/// How far noise moves the supercontinent's outer coast, in radians.
const COAST_NOISE: f32 = 0.35;

/// Least and most a plate rotates about its pole before year 0, in radians:
/// some pieces barely leave home, some cross an ocean.
const DRIFT_MIN: f32 = 0.12;
const DRIFT_MAX: f32 = 0.55;

/// How far the plates turn during play, as a share of their drift before
/// year 0, over [`PLAY_SPAN_YEARS`]: the fastest cross about 13 tiles, an
/// ocean opening as wide as a young Atlantic (D27). Tuned in the sweep.
const PLAY_DRIFT: f32 = 0.9;

/// The span of play the drift above is measured over (design: "how you
/// behaved for 5,000 years").
const PLAY_SPAN_YEARS: f32 = 5_000.0;

/// Height of the supercontinent's ancient, worn ranges.
const OLD_RANGES: f32 = 0.22;

/// Height two continents add where they collide and fold.
const UPLIFT: f32 = 0.6;

/// Height a collision's plateau stands above the continents that made it.
const PLATEAU: f32 = 0.06;

/// Share of [`UPLIFT`] a continent raises riding over ocean floor.
const COASTAL_RANGE: f32 = 0.35;

/// Share of [`UPLIFT`] two ocean floors raise where one dives under the
/// other: a broken chain of island arcs.
const ISLAND_ARC: f32 = 0.5;

/// Weight of the broad fBm relief on top of the crust.
const DETAIL: f32 = 0.30;

/// Weight of the fine roughness that breaks coasts, ranges and island arcs.
const ROUGHNESS: f32 = 0.16;

/// Mantle hotspots, fixed while the plates drift over them.
const HOTSPOTS: u32 = 10;

/// Height the youngest volcano over a hotspot adds to its tile.
const VOLCANO: f32 = 0.45;

/// Arc a plate drifts between one volcano on a track and the next.
const TRACK_STEP: f32 = 0.06;

/// Share of track steps where the hotspot slept: the gaps in a chain.
const QUIET_SHARE: f32 = 0.35;

/// Erosion passes, the slope rock holds before it slides, the share of the
/// excess that slides per pass, and how far land relaxes toward its
/// neighbors per pass.
const EROSION_PASSES: u32 = 4;
const TALUS: f32 = 0.11;
const SHED: f32 = 0.4;
const WEATHERING: f32 = 0.08;

/// How far noise bends the rifts between plates, in radians, so they wander
/// like coasts.
const WARP: f32 = 0.18;

/// Distinct noise streams per field, so elevation and moisture are independent.
const SEED_ELEVATION: u64 = 0x0E1E_7A70_1234_5678;
const SEED_MOISTURE: u64 = 0x4015_7A2E_5598_7654;
const SEED_PLATES: u64 = 0x91A7_E5C0_0000_0001;
const SEED_WARP: u64 = 0x5EA1_0000_77A3_0002;
const SEED_COAST: u64 = 0xC0A5_7000_0000_0003;
const SEED_HOTSPOTS: u64 = 0x407_5907_0000_0004;
const SEED_SUTURES: u64 = 0x5_0700_0000_0005;

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

    let elevation_field = elevation_at(seed, 0);
    for y in 0..GRID_HEIGHT as i32 {
        let lat = latitude_deg(y as usize);
        let area = latitude_area_factor(lat);
        for x in 0..GRID_WIDTH as i32 {
            let i = y as usize * GRID_WIDTH + x as usize;
            let elevation = elevation_field[i];
            let wet =
                0.7 * fbm(x, y, seed ^ SEED_MOISTURE) + 0.3 * fine(x, y, seed ^ SEED_MOISTURE);
            let moisture = contrast(wet, 1.5);
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

/// A piece of the supercontinent's crust: the plate it was before the breakup
/// (its center), and the rotation that carried it to the starting map.
struct Plate {
    center: [f32; 3],
    /// Euler pole of the plate's drift.
    axis: [f32; 3],
    /// How far it turned about that pole, in radians.
    angle: f32,
}

/// The world before year 0: one supercontinent and the plates it breaks into.
struct Breakup {
    plates: Vec<Plate>,
    /// Center and arc radius of the supercontinent.
    heart: [f32; 3],
    radius: f32,
    seed: u64,
}

/// Normalized elevation for every tile `years` into play: the broken
/// supercontinent, its collisions and rifts, then relief, rescaled so the
/// shore falls midway between continental and oceanic crust. The plates keep
/// drifting during play (D27), so the same seed at a later year gives the
/// same land moved on: rifts wider, collisions higher, hotspot chains longer.
pub fn elevation_at(seed: u64, years: u64) -> Vec<f32> {
    let mut raw = vec![0.0_f32; GRID_WIDTH * GRID_HEIGHT];
    crust_rows(seed, years, 0..GRID_HEIGHT, &mut raw);
    finish_ground(seed, years, raw)
}

/// The costly half of [`elevation_at`], a band of rows at a time, so geology
/// can spread it over the years before a pass (D27): each plate's crust plus
/// relief, before erosion.
pub fn crust_rows(seed: u64, years: u64, rows: std::ops::Range<usize>, raw: &mut [f32]) {
    let breakup = Breakup::new(seed, years);
    for y in rows {
        let y = y as i32;
        for x in 0..GRID_WIDTH as i32 {
            let i = y as usize * GRID_WIDTH + x as usize;
            let detail = fbm(x, y, seed ^ SEED_ELEVATION) - 0.5;
            let rough = fine(x, y, seed ^ SEED_ELEVATION) - 0.5;
            let crust = breakup.crust(tile_point(x, y), rough);
            raw[i] = crust + DETAIL * detail + ROUGHNESS * rough;
        }
    }
}

/// The cheap, whole-map half of [`elevation_at`]: erosion, then volcanoes,
/// then the rescale to sea level.
pub fn finish_ground(seed: u64, years: u64, mut raw: Vec<f32>) -> Vec<f32> {
    erode(&mut raw);
    // Volcanoes after weathering: a cone's age already wears it down.
    Breakup::new(seed, years).volcanoes(&mut raw);
    rescale(&raw)
}

/// Weathering after the breakup: steep slopes shed rock downhill until no
/// drop exceeds [`TALUS`] by much, and land relaxes toward its neighbors, so
/// ranges wear into foothills and plateaus soften (thermal erosion).
fn erode(raw: &mut [f32]) {
    let shore = f32::midpoint(CONTINENT_HEIGHT, OCEAN_HEIGHT);
    for _ in 0..EROSION_PASSES {
        let before = raw.to_vec();
        for tile in 0..before.len() {
            let (x, y) = (tile % GRID_WIDTH, tile / GRID_WIDTH);
            let mut sum = 0.0;
            let mut count = 0.0;
            for (nx, ny) in hex::neighbors(x, y) {
                let next = ny * GRID_WIDTH + nx;
                sum += before[next];
                count += 1.0;
                let drop = before[tile] - before[next];
                if drop > TALUS {
                    let shed = SHED * (drop - TALUS) / 6.0;
                    raw[tile] -= shed;
                    raw[next] += shed;
                }
            }
            if before[tile] > shore && count > 0.0 {
                raw[tile] += WEATHERING * (sum / count - before[tile]);
            }
        }
    }
}

impl Breakup {
    /// Seeded supercontinent near the equator, plates at random points (the
    /// rifts run between them), and a random drift for each plate, carried
    /// on `years` into play.
    fn new(seed: u64, years: u64) -> Self {
        let carried_on = 1.0 + PLAY_DRIFT * years as f32 / PLAY_SPAN_YEARS;
        let unit =
            |k: u32, n: u32| hash2(k as i32, n as i32, seed ^ SEED_PLATES) as f32 / u32::MAX as f32;
        let on_sphere = |z: f32, lon: f32| {
            let r = sqrtf(1.0 - z * z);
            [r * cosf(lon), r * sinf(lon), z]
        };
        let plates = (0..PLATES as u32)
            .map(|k| Plate {
                center: on_sphere(unit(k, 0) * 2.0 - 1.0, unit(k, 1) * TAU),
                axis: on_sphere(unit(k, 2) * 2.0 - 1.0, unit(k, 3) * TAU),
                angle: (DRIFT_MIN + (DRIFT_MAX - DRIFT_MIN) * unit(k, 4)) * carried_on,
            })
            .collect();
        // A spherical cap's share of the sphere is (1 - cos r) / 2.
        let radius = acosf(1.0 - 2.0 * CONTINENTAL_SHARE);
        let heart = on_sphere((unit(99, 0) - 0.5) * 0.8, unit(99, 1) * TAU);
        Self {
            plates,
            heart,
            radius,
            seed,
        }
    }

    /// Hotspots stay put in the mantle while plates drift over them, so each
    /// leaves a chain of volcanoes along its plate's path: the youngest over
    /// the hotspot stands tallest, older ones worn down and drowned toward
    /// the far end, with gaps where it slept.
    fn volcanoes(&self, raw: &mut [f32]) {
        let unit = |k: u32, n: u32| {
            hash2(k as i32, n as i32, self.seed ^ SEED_HOTSPOTS) as f32 / u32::MAX as f32
        };
        for spot in 0..HOTSPOTS {
            let z = unit(spot, 0) * 2.0 - 1.0;
            let lon = unit(spot, 1) * TAU;
            let r = sqrtf(1.0 - z * z);
            let hotspot = [r * cosf(lon), r * sinf(lon), z];
            for (k, plate) in self.plates.iter().enumerate() {
                let mut step = 0;
                let mut age = 0.0;
                while age <= plate.angle {
                    let now = rotate(hotspot, plate.axis, age);
                    let carried = self.owner(rotate(now, plate.axis, -plate.angle)) == k;
                    let erupted = unit(spot * 64 + k as u32, step + 8) > QUIET_SHARE;
                    if carried && erupted {
                        // Old volcanoes wear and sink, but their stumps stay.
                        let youth = (1.0 - age / plate.angle.max(DRIFT_MAX)).max(0.35);
                        cone(raw, tile_of(now), VOLCANO * youth);
                    }
                    age += TRACK_STEP;
                    step += 1;
                }
            }
        }
    }

    /// The crust at `point` on the starting map: every plate whose drifted
    /// piece now covers it, and what they do where they meet.
    fn crust(&self, point: [f32; 3], rough: f32) -> f32 {
        let shore = f32::midpoint(CONTINENT_HEIGHT, OCEAN_HEIGHT);
        let (mut lands, mut seas) = (0, 0);
        let (mut land, mut sea) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
        for (k, plate) in self.plates.iter().enumerate() {
            // Where this point's crust sat before the breakup, if plate k
            // carried it here.
            let origin = rotate(point, plate.axis, -plate.angle);
            if self.owner(origin) != k {
                continue;
            }
            let height = self.before(origin);
            if height > shore {
                lands += 1;
                land = land.max(height);
            } else {
                seas += 1;
                sea = sea.max(height);
            }
        }
        match (lands, seas) {
            // Pieces parted here and left a gap: young ocean opens.
            (0, 0) => RIDGE_HEIGHT,
            (0, 1) => sea,
            // One floor dives under another: islands where it breaks surface.
            (0, _) => sea + ISLAND_ARC * UPLIFT * (0.5 + rough).max(0.0),
            (1, 0) => land,
            // A continent rides over ocean floor and raises a coastal range.
            (1, _) => land + COASTAL_RANGE * UPLIFT * ridge(rough),
            // Continents collide: a raised plateau folded into chains of
            // ranges, not one solid block of peaks.
            _ => land + PLATEAU + UPLIFT * ridge(rough),
        }
    }

    /// Which plate a pre-breakup point belonged to: the nearest center, with
    /// the rifts bent by noise taken in the plate's own frame, so a coast's
    /// shape travels with it.
    fn owner(&self, origin: [f32; 3]) -> usize {
        let (x, y) = tile_of(origin);
        let bend = |stream: u64| {
            let broad = fbm(x, y, self.seed ^ SEED_WARP ^ stream) - 0.5;
            let jitter = fine(x, y, self.seed ^ SEED_WARP ^ stream) - 0.5;
            (broad + 0.5 * jitter) * 2.0 * WARP
        };
        let bent = normalized([
            origin[0] + bend(1),
            origin[1] + bend(2),
            origin[2] + bend(3),
        ]);
        let mut best = (0, f32::NEG_INFINITY);
        for (k, plate) in self.plates.iter().enumerate() {
            let closeness = dot(bent, plate.center);
            if closeness > best.1 {
                best = (k, closeness);
            }
        }
        best.0
    }

    /// Crust height before the breakup: the supercontinent, shelving to the
    /// ocean floor at a coast that noise pushes in and out.
    fn before(&self, origin: [f32; 3]) -> f32 {
        let (x, y) = tile_of(origin);
        let edge = (fbm(x, y, self.seed ^ SEED_COAST) - 0.5) * 2.0 * COAST_NOISE;
        let inland =
            ((self.radius + edge - arc(origin, self.heart)) / SHELF_WIDTH).clamp(-1.0, 1.0);
        let mid = f32::midpoint(CONTINENT_HEIGHT, OCEAN_HEIGHT);
        let crust = mid + (CONTINENT_HEIGHT - OCEAN_HEIGHT) * 0.5 * inland;
        // Ancient ranges from the supercontinent's own assembly, long worn
        // low: the breakup splits them onto facing coasts, as the Atlantic
        // split one old chain into the Appalachians and the highlands of
        // Britain and Norway.
        let suture = fbm(x, y, self.seed ^ SEED_SUTURES) - 0.5;
        crust + OLD_RANGES * ridge(suture) * inland.max(0.0)
    }
}

/// Raises a volcano of `height` on a tile, half as much on its neighbors.
fn cone(raw: &mut [f32], (x, y): (i32, i32), height: f32) {
    for (cx, cy) in hex::disc(x as usize, y as usize, 1) {
        let tile = cy * GRID_WIDTH + cx;
        let share = if (cx, cy) == (x as usize, y as usize) {
            1.0
        } else {
            0.5
        };
        raw[tile] += height * share;
    }
}

/// Ridged noise: 1 along the crests where `rough` crosses its midpoint,
/// falling to 0 between them, so uplift runs in chains.
fn ridge(rough: f32) -> f32 {
    let crest = (1.0 - 4.0 * rough.abs()).max(0.0);
    crest * crest
}

/// A tile's center on the unit sphere (odd rows sit half a tile east).
fn tile_point(x: i32, y: i32) -> [f32; 3] {
    let row_offset = if y % 2 == 1 { 0.5 } else { 0.0 };
    let lon = (x as f32 + 0.5 + row_offset) / GRID_WIDTH as f32 * TAU;
    let lat = latitude_deg(y as usize).to_radians();
    [cosf(lat) * cosf(lon), cosf(lat) * sinf(lon), sinf(lat)]
}

/// The tile a point on the sphere falls in, for sampling noise there.
fn tile_of(point: [f32; 3]) -> (i32, i32) {
    let lon = atan2f(point[1], point[0]).rem_euclid(TAU);
    let lat = asinf(point[2].clamp(-1.0, 1.0)).to_degrees();
    let x = (lon / TAU * GRID_WIDTH as f32) as i32 % GRID_WIDTH as i32;
    let y = ((90.0 - lat) / (180.0 / GRID_HEIGHT as f32)) as i32;
    (x, y.clamp(0, GRID_HEIGHT as i32 - 1))
}

/// `v` turned by `angle` about the unit `axis` (Rodrigues' rotation).
fn rotate(v: [f32; 3], axis: [f32; 3], angle: f32) -> [f32; 3] {
    let (sin, cos) = (sinf(angle), cosf(angle));
    let cross = [
        axis[1] * v[2] - axis[2] * v[1],
        axis[2] * v[0] - axis[0] * v[2],
        axis[0] * v[1] - axis[1] * v[0],
    ];
    let along = dot(axis, v) * (1.0 - cos);
    std::array::from_fn(|k| v[k] * cos + cross[k] * sin + axis[k] * along)
}

fn normalized(v: [f32; 3]) -> [f32; 3] {
    let length = sqrtf(dot(v, v)).max(1e-6);
    v.map(|c| c / length)
}

/// Great-circle distance between two unit vectors, in radians.
fn arc(a: [f32; 3], b: [f32; 3]) -> f32 {
    acosf(dot(a, b).clamp(-1.0, 1.0))
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Maps raw heights so the shore lies at [`SEA_LEVEL`]: below it onto
/// [0, sea level], above it onto (sea level, 1]. The top is the 99.5th
/// percentile, not the single highest peak, so a few tall volcanoes cannot
/// flatten every range beneath them.
fn rescale(raw: &[f32]) -> Vec<f32> {
    let cut = f32::midpoint(CONTINENT_HEIGHT, OCEAN_HEIGHT);
    let mut sorted = raw.to_vec();
    sorted.sort_by(f32::total_cmp);
    let low = sorted[0].min(cut - 1e-3);
    let high = sorted[sorted.len() * 998 / 1000].max(cut + 1e-3);
    raw.iter()
        .map(|&h| {
            if h <= cut {
                SEA_LEVEL * (h - low) / (cut - low)
            } else {
                (SEA_LEVEL + (1.0 - SEA_LEVEL) * (h - cut) / (high - cut)).min(1.0)
            }
        })
        .collect()
}

/// Fractal Brownian motion over [`OCTAVES`] doubling lattice scales, the
/// broadest weighted most, so fields form regions roughened at the edges
/// rather than tile-sized static.
fn fbm(tx: i32, ty: i32, seed: u64) -> f32 {
    let mut sum = 0.0;
    let mut norm = 0.0;
    let mut amplitude = 0.5;
    let mut scale = FIRST_SCALE << (OCTAVES - 1);
    for octave in 0..OCTAVES {
        sum += amplitude * lattice_noise(tx, ty, scale, seed.wrapping_add(u64::from(octave)));
        norm += amplitude;
        amplitude *= 0.5;
        scale /= 2;
    }
    sum / norm
}

/// The two finest octaves alone: tile-scale roughness for coasts and edges.
fn fine(tx: i32, ty: i32, seed: u64) -> f32 {
    let salted = seed.wrapping_add(0xF1_4E);
    (2.0 * lattice_noise(tx, ty, FIRST_SCALE, salted)
        + lattice_noise(tx, ty, FIRST_SCALE * 2, salted.wrapping_add(1)))
        / 3.0
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
