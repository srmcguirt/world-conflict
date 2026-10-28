//! The real world's shaping events, set symbolically on a generated one
//! (design, D28).
//!
//! The rifts, collisions and floods that shaped where people lived are laid
//! onto every world around its cradle, the best land of its largest landmass,
//! where the Garden is: an ocean split through the old supercontinent with
//! one worn range on both shores, a great range and plateau thrown up by a
//! collision, a narrow sea and an inland sea beside the cradle, a land bridge
//! between far continents, a far continent reached only by island-hopping, a
//! great island offshore. During play a rift valley deepens beside the Garden,
//! a fertile lowland drowns, a land bridge and a lowland neck sink behind the
//! peoples who crossed them, a shelf breaks into islands, a strait breaches
//! into a dry basin, and a volcanic island blows apart. The real names are
//! kept here only as each event's symbol; the world names nothing after them.
//!
//! Each event edits raw height (before the rescale to sea level) over an area,
//! eased in over its years, so the geology passes carry it through play like
//! any other change of land (D27).

use libm::sqrtf;

use crate::world::{GRID_HEIGHT, GRID_WIDTH};
use crate::worldgen::{self, Biome};

/// Raw height a drowned area sinks to: shallow sea.
const SEA: f32 = 0.38;

/// Raw height of a fertile lowland just above the shore, the kind of land
/// the sea takes back.
const LOWLAND: f32 = 0.52;

/// Raw height of a raised island or bridge.
const ISLAND: f32 = 0.58;

/// Most water a land bridge may span, in tiles.
const BRIDGE_REACH: f32 = 16.0;

/// Least size of a landmass that counts as a continent here.
const CONTINENT: usize = 60;

/// When an event happens.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum When {
    /// Before year 0: part of the starting map.
    Before,
    /// Eased in from the first year to the second.
    Over(u64, u64),
    /// All at once in that year, a multiple of the geology pass.
    At(u64),
}

/// Where an event happens, in tile coordinates (x east, y south).
#[derive(Copy, Clone, Debug, PartialEq)]
enum Area {
    /// Tiles within `half` of the segment from `a` to `b`.
    Strip { a: [f32; 2], b: [f32; 2], half: f32 },
    /// Tiles inside the ellipse with semi-axes `axes`.
    Oval { center: [f32; 2], axes: [f32; 2] },
}

/// What an event does to raw height.
#[derive(Copy, Clone, Debug, PartialEq)]
enum Change {
    /// Sinks land to shallow sea.
    Drown,
    /// Lifts the area to at least this height.
    Lift(f32),
    /// Adds height, most at the area's middle line.
    Fold(f32),
    /// Lowers by this much, most at the middle line.
    Sink(f32),
}

/// One event of the catalog, placed on this world.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Event {
    /// The real event it stands for, for the design record and tests.
    pub symbol: &'static str,
    pub when: When,
    area: Area,
    change: Change,
}

/// A wave from a volcano's collapse: coastal people within reach are lost.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Wave {
    pub year: u64,
    pub center: [f32; 2],
    pub reach: f32,
}

/// The catalog placed on one world.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    /// The best land of the largest landmass, where the Garden goes.
    pub cradle: Option<usize>,
    pub events: Vec<Event>,
    pub waves: Vec<Wave>,
    /// Ragged edges and wandering lines are drawn from the world's seed.
    seed: u64,
}

/// Places the catalog on the world the plates made for `seed`.
pub fn plan(seed: u64) -> Plan {
    let base = worldgen::base_world(seed);
    let masses = Masses::of(&base.biome);
    let Some(cradle) = cradle(&base, &masses) else {
        return Plan::default();
    };
    let c = coords(cradle);
    let mut plan = Plan {
        cradle: Some(cradle),
        events: Vec::new(),
        waves: Vec::new(),
        seed,
    };
    deep_past(&mut plan, &base.biome, &masses, c);
    within_play(&mut plan, &base.biome, &masses, c);
    plan
}

/// The starting map's events, around the cradle `c`.
fn deep_past(plan: &mut Plan, biome: &[Biome], masses: &Masses, c: [f32; 2]) {
    atlantic(plan, biome, c);
    let at = |dx: f32, dy: f32| [c[0] + dx, c[1] + dy];
    // A fast piece of crust slams into the cradle's continent: a high range
    // with a plateau behind it.
    plan.push(
        "india_asia",
        When::Before,
        strip(at(14.0, -12.0), at(34.0, -14.0), 1.5),
        Change::Fold(0.35),
    );
    plan.push(
        "tibet",
        When::Before,
        oval(at(24.0, -18.0), [10.0, 4.0]),
        Change::Lift(0.70),
    );
    plan.push(
        "red_sea",
        When::Before,
        strip(at(6.0, 6.0), at(11.0, -8.0), 0.8),
        Change::Drown,
    );
    // An inland sea with a narrow strait to the ocean.
    let med = at(-3.0, -13.0);
    plan.push(
        "mediterranean",
        When::Before,
        oval(med, [9.0, 2.5]),
        Change::Drown,
    );
    let gibraltar = [med[0] - 9.0, med[1]];
    plan.push(
        "gibraltar",
        When::Before,
        strip(gibraltar, [gibraltar[0] - 14.0, gibraltar[1]], 0.6),
        Change::Drown,
    );
    plan.push(
        "thera_rises",
        When::Before,
        oval([med[0] + 3.0, med[1] + 1.0], [1.0, 1.0]),
        Change::Lift(0.62),
    );
    if let Some(island) = offshore(biome, c, [1.0, 1.2], 3.0) {
        plan.push(
            "madagascar",
            When::Before,
            oval(island, [2.5, 5.0]),
            Change::Lift(ISLAND + 0.04),
        );
    }
    if let Some((a, b)) = masses.nearest_pair_apart_from_main() {
        plan.push(
            "panama",
            When::Before,
            strip(a, b, 1.0),
            Change::Lift(ISLAND),
        );
    }
    if let Some((a, b)) = masses.farthest_from(c) {
        stepping_stones(plan, biome, a, b);
    }
}

/// The events of the 5,000 years, around the cradle `c`.
fn within_play(plan: &mut Plan, biome: &[Biome], masses: &Masses, c: [f32; 2]) {
    let at = |dx: f32, dy: f32| [c[0] + dx, c[1] + dy];
    plan.push(
        "east_african_rift",
        When::Over(0, 5_000),
        strip(at(-3.0, 10.0), at(-1.0, -8.0), 1.5),
        Change::Sink(0.16),
    );
    let gulf = oval(at(16.0, -4.0), [5.0, 1.8]);
    plan.push(
        "persian_gulf_land",
        When::Before,
        gulf,
        Change::Lift(LOWLAND),
    );
    plan.push("persian_gulf", When::Over(200, 400), gulf, Change::Drown);
    if let Some((a, b)) = masses.bridge_from_main(c) {
        let neck = strip(a, b, 1.2);
        plan.push("beringia_land", When::Before, neck, Change::Lift(LOWLAND));
        plan.push("beringia", When::Over(800, 1_500), neck, Change::Drown);
    }
    if let Some(island) = offshore(biome, c, [-1.0, -0.4], 5.0) {
        let coast = offshore(biome, c, [-1.0, -0.4], 0.0).unwrap_or(island);
        plan.push(
            "britain",
            When::Before,
            oval(island, [4.0, 3.0]),
            Change::Lift(ISLAND),
        );
        let neck = strip(coast, island, 1.5);
        plan.push("doggerland_land", When::Before, neck, Change::Lift(LOWLAND));
        plan.push("doggerland", When::Over(1_000, 1_800), neck, Change::Drown);
    }
    if let Some(shelf) = offshore(biome, c, [1.0, 0.3], 4.0) {
        let area = oval(shelf, [7.0, 4.0]);
        plan.push("sundaland", When::Before, area, Change::Lift(LOWLAND));
        plan.push("sunda", When::Over(1_000, 2_500), area, Change::Sink(0.10));
    }
    // A dry basin beyond the inland sea, until the strait between breaches.
    let med = at(-3.0, -13.0);
    let basin = oval(at(8.0, -20.0), [4.5, 2.0]);
    plan.push(
        "black_sea_basin",
        When::Before,
        basin,
        Change::Lift(LOWLAND),
    );
    plan.push("black_sea", When::At(1_500), basin, Change::Drown);
    plan.push(
        "bosporus",
        When::At(1_500),
        strip([med[0] + 9.0, med[1]], at(8.0, -20.0), 0.6),
        Change::Drown,
    );
    let thera = [med[0] + 3.0, med[1] + 1.0];
    plan.push(
        "thera",
        When::At(2_500),
        oval(thera, [1.8, 1.8]),
        Change::Drown,
    );
    plan.waves.push(Wave {
        year: 2_500,
        center: thera,
        reach: 12.0,
    });
}

/// The great ocean split through the old supercontinent, on the side of the
/// cradle with less land beyond it, with one worn range left on both shores.
fn atlantic(plan: &mut Plan, biome: &[Biome], c: [f32; 2]) {
    let share = |side: f32, reach: f32| {
        let x = c[0] + side * reach;
        land_share(biome, strip([x, c[1] - 28.0], [x, c[1] + 28.0], 3.0))
    };
    // The land the split would cut away: the cradle keeps the larger piece,
    // as Africa and Eurasia kept theirs and the Americas went.
    let beyond = |side: f32| {
        let x = c[0] + side * 40.0;
        land_share(biome, oval([x, c[1]], [18.0, 28.0]))
    };
    let side = if beyond(-1.0) <= beyond(1.0) {
        -1.0
    } else {
        1.0
    };
    // Nearest first, so the split runs through land rather than open sea.
    let Some(reach) = [20.0, 14.0, 26.0, 32.0]
        .into_iter()
        .find(|&reach| share(side, reach) >= 0.3)
    else {
        already_open(plan, biome, c, side);
        return;
    };
    // A wandering line, not a ruled one: each joint swings up to four tiles
    // either way, and the ocean is wider in some reaches than others.
    let joints: Vec<[f32; 2]> = (0..=ATLANTIC_JOINTS)
        .map(|k| {
            let swing = (unit(plan.seed, k, 1) - 0.5) * 8.0;
            let y = c[1] - 28.0 + 56.0 * k as f32 / ATLANTIC_JOINTS as f32;
            [c[0] + side * reach + swing, y]
        })
        .collect();
    for (k, pair) in joints.windows(2).enumerate() {
        let half = 2.5 + 1.5 * unit(plan.seed, k as u32, 2);
        let (a, b) = (pair[0], pair[1]);
        plan.push("atlantic", When::Before, strip(a, b, half), Change::Drown);
        for shore in [-(half + 2.5), half + 2.5] {
            let worn = strip([a[0] + shore, a[1]], [b[0] + shore, b[1]], 1.5);
            plan.push(
                "appalachian_caledonian",
                When::Before,
                worn,
                Change::Fold(0.12),
            );
        }
    }
}

/// The plates already opened an ocean on that side: it is the split, and the
/// worn range is laid along both of its shores.
fn already_open(plan: &mut Plan, biome: &[Biome], c: [f32; 2], side: f32) {
    let Some(near) = offshore(biome, c, [side, 0.0], 0.0) else {
        return;
    };
    let mut at = near;
    let far = (0..GRID_WIDTH).find_map(|_| {
        at[0] += side;
        land(biome[index(at)]).then_some(at)
    });
    let Some(far) = far else {
        return;
    };
    let water_end = [far[0] - side, far[1]];
    plan.push(
        "atlantic",
        When::Before,
        strip(near, water_end, 1.0),
        Change::Drown,
    );
    for x in [near[0] - side * 3.0, far[0] + side * 3.0] {
        let worn = strip([x, c[1] - 20.0], [x, c[1] + 20.0], 1.5);
        plan.push(
            "appalachian_caledonian",
            When::Before,
            worn,
            Change::Fold(0.12),
        );
    }
}

/// Joints in the ocean split's wandering line.
const ATLANTIC_JOINTS: u32 = 6;

/// How far a ragged edge reaches in or out, as a share of the distance to an
/// area's middle.
const RAGGED: f32 = 0.35;

/// A seeded number in [0, 1).
fn unit(seed: u64, k: u32, stream: u32) -> f32 {
    let mut z = seed ^ (u64::from(k) << 32) ^ u64::from(stream).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 24) as f32
}

/// Islands every few tiles across the water between two coasts, so the far
/// continent is reached only by island-hopping.
fn stepping_stones(plan: &mut Plan, biome: &[Biome], a: [f32; 2], b: [f32; 2]) {
    let (dx, dy) = (wrap_dx(b[0] - a[0]), b[1] - a[1]);
    let length = sqrtf(dx * dx + dy * dy);
    let stones = (length / 4.0) as usize;
    for k in 1..stones {
        let t = k as f32 / stones as f32;
        let at = [a[0] + dx * t, a[1] + dy * t];
        if biome[index(at)] == Biome::Ocean {
            plan.push(
                "wallacea",
                When::Before,
                oval(at, [1.0, 1.0]),
                Change::Lift(ISLAND),
            );
        }
    }
}

impl Plan {
    fn push(&mut self, symbol: &'static str, when: When, area: Area, change: Change) {
        self.events.push(Event {
            symbol,
            when,
            area,
            change,
        });
    }

    /// Applies every event as far as it has run by `years` to raw height.
    pub(crate) fn apply(&self, years: u64, raw: &mut [f32]) {
        for event in &self.events {
            let progress = progress(event.when, years);
            if progress > 0.0 {
                event.apply(progress, raw, self.seed);
            }
        }
    }

    /// Whether any event has a symbol.
    pub fn has(&self, symbol: &str) -> bool {
        self.events.iter().any(|event| event.symbol == symbol)
    }

    /// The tiles an event with `symbol` covers.
    pub fn tiles_of(&self, symbol: &str) -> Vec<usize> {
        self.events
            .iter()
            .filter(|event| event.symbol == symbol)
            .flat_map(|event| {
                tiles(event.area, self.seed)
                    .into_iter()
                    .map(|(tile, _)| tile)
            })
            .collect()
    }
}

impl Event {
    fn apply(&self, progress: f32, raw: &mut [f32], seed: u64) {
        for (tile, middle) in tiles(self.area, seed) {
            let now = raw[tile];
            let full = match self.change {
                Change::Drown => now.min(SEA),
                Change::Lift(height) => now.max(height),
                Change::Fold(height) => now + height * middle,
                Change::Sink(depth) => now - depth * middle,
            };
            raw[tile] = now + (full - now) * progress;
        }
    }
}

/// How far an event has run by `years`, 0 to 1.
fn progress(when: When, years: u64) -> f32 {
    match when {
        When::Before => 1.0,
        When::At(year) => f32::from(u8::from(years >= year)),
        When::Over(start, end) => {
            ((years as f32 - start as f32) / (end - start) as f32).clamp(0.0, 1.0)
        }
    }
}

/// Every tile in an area, with how near it is to the middle (1 on the middle
/// line or center, 0 at the edge). Edges are ragged by a seeded amount per
/// tile, so seas and ranges do not come out ruled.
fn tiles(area: Area, seed: u64) -> Vec<(usize, f32)> {
    let mut out = Vec::new();
    let (low, high) = match area {
        Area::Strip { a, b, half } => (
            [
                a[0].min(a[0] + wrap_dx(b[0] - a[0])) - half,
                a[1].min(b[1]) - half,
            ],
            [
                a[0].max(a[0] + wrap_dx(b[0] - a[0])) + half,
                a[1].max(b[1]) + half,
            ],
        ),
        Area::Oval { center, axes } => (
            [center[0] - axes[0], center[1] - axes[1]],
            [center[0] + axes[0], center[1] + axes[1]],
        ),
    };
    let rows = (low[1].floor().max(0.0) as usize)
        ..=(high[1].ceil().min(GRID_HEIGHT as f32 - 1.0) as usize);
    for y in rows {
        for x in (low[0].floor() as i32)..=(high[0].ceil() as i32) {
            let point = [x as f32, y as f32];
            let near = match area {
                Area::Strip { a, b, half } => 1.0 - segment_distance(point, a, b) / half,
                Area::Oval { center, axes } => {
                    let dx = wrap_dx(point[0] - center[0]) / axes[0];
                    let dy = (point[1] - center[1]) / axes[1];
                    1.0 - sqrtf(dx * dx + dy * dy)
                }
            };
            let tile = index(point);
            let ragged =
                near + (unit(seed, tile as u32, 3) - 0.5) * 2.0 * RAGGED * (1.0 - near).min(1.0);
            if ragged >= 0.0 {
                out.push((tile, ragged.min(1.0)));
            }
        }
    }
    out
}

fn strip(a: [f32; 2], b: [f32; 2], half: f32) -> Area {
    Area::Strip { a, b, half }
}

fn oval(center: [f32; 2], axes: [f32; 2]) -> Area {
    Area::Oval { center, axes }
}

/// Distance from `p` to the segment `a`-`b`, wrapping east to west.
fn segment_distance(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (ax, bx) = (0.0, wrap_dx(b[0] - a[0]));
    let px = wrap_dx(p[0] - a[0]);
    let (dx, dy) = (bx - ax, b[1] - a[1]);
    let length = dx * dx + dy * dy;
    let t = if length > 0.0 {
        (((px - ax) * dx + (p[1] - a[1]) * dy) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (ex, ey) = (px - (ax + dx * t), p[1] - (a[1] + dy * t));
    sqrtf(ex * ex + ey * ey)
}

/// An east-west difference wrapped to the shorter way round.
fn wrap_dx(dx: f32) -> f32 {
    let width = GRID_WIDTH as f32;
    (dx + width * 0.5).rem_euclid(width) - width * 0.5
}

/// The tile under a point, wrapping east to west and clamping at the poles.
fn index(p: [f32; 2]) -> usize {
    let x = (p[0].round() as i32).rem_euclid(GRID_WIDTH as i32) as usize;
    let y = p[1].round().clamp(0.0, GRID_HEIGHT as f32 - 1.0) as usize;
    y * GRID_WIDTH + x
}

fn coords(tile: usize) -> [f32; 2] {
    [(tile % GRID_WIDTH) as f32, (tile / GRID_WIDTH) as f32]
}

fn land(biome: Biome) -> bool {
    !matches!(biome, Biome::Ocean | Biome::Ice)
}

fn land_share(biome: &[Biome], area: Area) -> f32 {
    let tiles = tiles(area, 0);
    let count = tiles.iter().filter(|&&(tile, _)| land(biome[tile])).count();
    count as f32 / tiles.len().max(1) as f32
}

/// Walks from `from` along `dir` to the coast, then `gap` tiles out to sea.
fn offshore(biome: &[Biome], from: [f32; 2], dir: [f32; 2], gap: f32) -> Option<[f32; 2]> {
    let norm = sqrtf(dir[0] * dir[0] + dir[1] * dir[1]);
    let step = [dir[0] / norm, dir[1] / norm];
    let mut at = from;
    for _ in 0..60 {
        at = [at[0] + step[0], at[1] + step[1]];
        if at[1] < 1.0 || at[1] > GRID_HEIGHT as f32 - 2.0 {
            return None;
        }
        if !land(biome[index(at)]) {
            return Some([at[0] + step[0] * gap, at[1] + step[1] * gap]);
        }
    }
    None
}

/// The best settleable land of the largest landmass.
fn cradle(base: &worldgen::GeneratedWorld, masses: &Masses) -> Option<usize> {
    let main = masses.main?;
    (0..base.biome.len())
        .filter(|&t| masses.label[t] == Some(main) && worldgen::settleable(base.biome[t]))
        .max_by(|&a, &b| {
            base.carrying_capacity[a]
                .total_cmp(&base.carrying_capacity[b])
                .then(b.cmp(&a))
        })
}

/// Landmasses joined over land, and their coasts.
struct Masses {
    label: Vec<Option<usize>>,
    sizes: Vec<usize>,
    coasts: Vec<Vec<usize>>,
    main: Option<usize>,
}

impl Masses {
    fn of(biome: &[Biome]) -> Self {
        let mut label = vec![None; biome.len()];
        let mut sizes = Vec::new();
        let mut coasts = Vec::new();
        for start in 0..biome.len() {
            if !land(biome[start]) || label[start].is_some() {
                continue;
            }
            let id = sizes.len();
            let (mut size, mut coast) = (0, Vec::new());
            let mut stack = vec![start];
            label[start] = Some(id);
            while let Some(tile) = stack.pop() {
                size += 1;
                let mut shore = false;
                for (x, y) in crate::hex::neighbors(tile % GRID_WIDTH, tile / GRID_WIDTH) {
                    let next = y * GRID_WIDTH + x;
                    if !land(biome[next]) {
                        shore = true;
                    } else if label[next].is_none() {
                        label[next] = Some(id);
                        stack.push(next);
                    }
                }
                if shore {
                    coast.push(tile);
                }
            }
            coast.sort_unstable();
            sizes.push(size);
            coasts.push(coast);
        }
        let main = (0..sizes.len()).max_by_key(|&m| (sizes[m], std::cmp::Reverse(m)));
        Self {
            label,
            sizes,
            coasts,
            main,
        }
    }

    /// Continents other than the main one, largest first.
    fn others(&self) -> Vec<usize> {
        let mut out: Vec<usize> = (0..self.sizes.len())
            .filter(|&m| Some(m) != self.main && self.sizes[m] >= CONTINENT)
            .collect();
        out.sort_by_key(|&m| (std::cmp::Reverse(self.sizes[m]), m));
        out
    }

    /// The closest coasts of two landmasses, as points and their distance.
    fn gap(&self, one: usize, other: usize) -> ([f32; 2], [f32; 2], f32) {
        let mut best = ([0.0; 2], [0.0; 2], f32::INFINITY);
        for &here in &self.coasts[one] {
            for &there in &self.coasts[other] {
                let (from, to) = (coords(here), coords(there));
                let (dx, dy) = (wrap_dx(to[0] - from[0]), to[1] - from[1]);
                let apart = sqrtf(dx * dx + dy * dy);
                if apart < best.2 {
                    best = (from, to, apart);
                }
            }
        }
        best
    }

    /// A bridge between the two nearest continents, neither the main one.
    fn nearest_pair_apart_from_main(&self) -> Option<([f32; 2], [f32; 2])> {
        let others = self.others();
        let mut best: Option<([f32; 2], [f32; 2], f32)> = None;
        for (i, &a) in others.iter().enumerate() {
            for &b in &others[i + 1..] {
                let found = self.gap(a, b);
                if found.2 <= BRIDGE_REACH && best.is_none_or(|top| found.2 < top.2) {
                    best = Some(found);
                }
            }
        }
        best.map(|(a, b, _)| (a, b))
    }

    /// The main continent's nearest neck to another continent, far from the
    /// cradle: the bridge the first peoples cross before it drowns.
    fn bridge_from_main(&self, cradle: [f32; 2]) -> Option<([f32; 2], [f32; 2])> {
        let main = self.main?;
        let mut best: Option<([f32; 2], [f32; 2], f32)> = None;
        for other in self.others() {
            let found = self.gap(main, other);
            let far =
                (wrap_dx(found.0[0] - cradle[0])).abs() + (found.0[1] - cradle[1]).abs() >= 30.0;
            if far && found.2 <= BRIDGE_REACH && best.is_none_or(|top| found.2 < top.2) {
                best = Some(found);
            }
        }
        best.map(|(a, b, _)| (a, b))
    }

    /// The continent farthest from the cradle, as the main continent's nearest
    /// coast to it and its own.
    fn farthest_from(&self, cradle: [f32; 2]) -> Option<([f32; 2], [f32; 2])> {
        let main = self.main?;
        let distance = |m: usize| {
            self.coasts[m]
                .iter()
                .map(|&t| {
                    let p = coords(t);
                    wrap_dx(p[0] - cradle[0]).abs() + (p[1] - cradle[1]).abs()
                })
                .fold(f32::INFINITY, f32::min)
        };
        let far = self
            .others()
            .into_iter()
            .max_by(|&a, &b| distance(a).total_cmp(&distance(b)).then(b.cmp(&a)))?;
        let (a, b, _) = self.gap(main, far);
        Some((a, b))
    }
}
