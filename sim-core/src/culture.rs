//! Cultures as discrete entities: identity, favor, terrain costs, split
//! components and the culture overlay (design, Simulation contracts ->
//! Cultures are discrete entities).
//!
//! A culture is a small population-weighted vector drifting through its
//! region's biome target; tiles drift and blend, and a component that drifts
//! far enough from its parent's centroid becomes a daughter people.

use crate::belief::Belief;
use crate::faith::Doctrine;
use crate::names::{self, Phonemes};
use crate::scripture::Verse;
use crate::world::{GRID_HEIGHT, GRID_WIDTH, World};
use crate::worldgen::{Biome, latitude_area_factor};

/// Culture-vector dimensions: the 5-value profile of the design's culture
/// weight (martial, devout, mercantile, scholarly, communal).
pub const DIMS: usize = 5;

/// Axis indices for [`Culture::centroid`] and per-tile vectors.
pub const MARTIAL: usize = 0;
pub const DEVOUT: usize = 1;
pub const MERCANTILE: usize = 2;
pub const SCHOLARLY: usize = 3;
pub const COMMUNAL: usize = 4;

/// Sentinel in `World::culture_id` for a tile nobody lives on (design: only
/// tiles with a population of at least 1 carry a culture).
pub const NO_CULTURE: u16 = u16::MAX;

/// Which people the god favors. Exactly one culture is `Chosen` at any time.
#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Favor {
    Chosen,
    Other,
}

/// Which of the first family's two lines a people descends from (design,
/// Out of the Garden; D4). Set at the founding and inherited unchanged at
/// every split.
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Line {
    /// Adam's line, named for the ground it works. Starts Chosen.
    Dirt,
    /// The line of the snake, which refuses to wander and builds.
    Snake,
}

/// How many lines the first family yields; arrays indexed by `Line as usize`.
pub const LINES: usize = 2;

/// A settled distance between two culture vectors (Euclidean over [`DIMS`]).
pub fn distance(a: &[f32; DIMS], b: &[f32; DIMS]) -> f32 {
    let mut sum = 0.0;
    for k in 0..DIMS {
        let d = a[k] - b[k];
        sum += d * d;
    }
    libm::sqrtf(sum)
}

/// One discrete people (design, Cultures are discrete entities).
///
/// `id` is the index in `World::cultures`; `centroid` is the population-weighted
/// mean of its member tiles, refreshed every 10 ticks.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Culture {
    pub id: u16,
    pub parent_id: Option<u16>,
    pub founded_tick: u64,
    /// Seeded at founding; names are generated from it from Milestone 2.
    pub name_seed: u64,
    pub centroid: [f32; DIMS],
    pub favor: Favor,
    /// The line this people descends from (design, Out of the Garden; D4).
    pub line: Line,
    /// Cached aggregates, refreshed every 10 ticks (design: aggregates).
    pub population: f32,
    pub tiles: u32,
    /// The people's own name for themselves, e.g. "the Karethi".
    pub name: String,
    /// What their language sounds like; names are built from it.
    pub phonemes: Phonemes,
    /// Their holy text, oldest first.
    pub scripture: Vec<Verse>,
    /// Population-weighted belief, refreshed every 10 ticks.
    pub belief: Belief,
    /// The doctrine they live by, once one crystallizes.
    pub doctrine: Option<Doctrine>,
    /// Doctrine hysteresis counter.
    pub(crate) doctrine_passes: u8,
    /// The faith they belong to.
    pub faith_id: u16,
    /// Schism hysteresis counter.
    pub(crate) schism_passes: u8,
    /// Strength of belief minus absence (design, Piety).
    pub piety: f32,
    /// The tongue she speaks (design, Language; a split spawns a dialect).
    pub language_id: u16,
    /// The people's population-weighted home, as mean `(x, y)` of their
    /// tiles, refreshed on the aggregate cadence. Contact and isolation
    /// weight the tongue's drift.
    pub place: [f32; 2],
    /// Mean traits of the people, refreshed every 10 ticks
    /// (design, Physical adaptation). Dress and portraits read these.
    pub mean_skin: f32,
    pub mean_stature: f32,
    pub mean_cold: f32,
    pub mean_altitude: f32,
    /// The tile the god last blessed for them (design, Protective).
    pub sacred_tile: Option<usize>,
}

impl Culture {
    /// Founds the first people: its own name, its own sound, an opening verse.
    pub fn found(
        id: u16,
        line: Line,
        name_seed: u64,
        centroid: [f32; DIMS],
        favor: Favor,
        population: f32,
        tiles: u32,
    ) -> Self {
        let mut culture = Self {
            id,
            parent_id: None,
            founded_tick: 0,
            name_seed,
            centroid,
            favor,
            line,
            population,
            tiles,
            name: String::new(),
            phonemes: Phonemes::seeded(name_seed),
            scripture: Vec::new(),
            belief: Belief::START,
            doctrine: None,
            doctrine_passes: 0,
            faith_id: 0,
            schism_passes: 0,
            language_id: 0,
            place: [0.0; 2],
            piety: 0.0,
            sacred_tile: None,
            mean_skin: crate::traits::START,
            mean_stature: crate::traits::START,
            mean_cold: crate::traits::START,
            mean_altitude: crate::traits::START,
        };
        culture.name = names::people_name(&culture);
        culture
    }

    /// Founds a daughter people: it inherits the parent's sound and scripture,
    /// then drifts on its own (design, Split rule).
    #[expect(
        clippy::too_many_arguments,
        reason = "a daughter people is founded from every one of these at once"
    )]
    pub fn daughter(
        parent: &Culture,
        id: u16,
        founded_tick: u64,
        name_seed: u64,
        centroid: [f32; DIMS],
        favor: Favor,
        population: f32,
        tiles: u32,
    ) -> Self {
        let mut culture = Self {
            id,
            parent_id: Some(parent.id),
            founded_tick,
            name_seed,
            centroid,
            favor,
            line: parent.line,
            population,
            tiles,
            name: String::new(),
            phonemes: parent.phonemes.clone(),
            scripture: parent.scripture.clone(),
            belief: parent.belief,
            doctrine: parent.doctrine,
            doctrine_passes: 0,
            faith_id: parent.faith_id,
            schism_passes: 0,
            language_id: parent.language_id,
            place: parent.place,
            piety: parent.piety,
            sacred_tile: parent.sacred_tile,
            mean_skin: parent.mean_skin,
            mean_stature: parent.mean_stature,
            mean_cold: parent.mean_cold,
            mean_altitude: parent.mean_altitude,
        };
        culture.name = names::people_name(&culture);
        // The copy mutates at the split: the same events, told in the
        // daughter's words (design, Scripture drifts with the language).
        let name = culture.name.clone();
        // Respell each inherited verse once, up front: the loop below only
        // mutates.
        let respelled: Vec<(bool, String, String)> = culture
            .scripture
            .iter()
            .map(|verse| {
                let place = (verse.place_tile != u32::MAX)
                    .then(|| names::place_name(&culture, verse.place_tile as usize));
                let witness = verse.witness_id.map(|id| names::person_name(&culture, id));
                (
                    place.is_some(),
                    place.unwrap_or_default(),
                    witness.unwrap_or_default(),
                )
            })
            .collect();
        for (verse, (spoken, place, witness)) in culture.scripture.iter_mut().zip(respelled) {
            verse.people.clone_from(&name);
            if spoken {
                verse.place = place;
                verse.witness = witness;
            }
        }
        culture
    }
}

/// Cost of one migration step into `biome` at `lat_deg`.
///
/// `east_west` multiplies by cos(lat): an east-west step near the poles covers
/// less real distance because tiles narrow there (design R2-10). North-south
/// cost is unchanged. Impassable biomes are infinite.
pub fn step_cost(biome: Biome, lat_deg: f32, east_west: bool) -> f32 {
    let base = match biome {
        Biome::Ocean | Biome::Ice => return f32::INFINITY,
        Biome::Grassland => 1.0,
        Biome::Beach => 1.2,
        Biome::Forest => 1.5,
        Biome::Desert => 2.5,
        Biome::Mountain => 3.5,
    };
    if east_west {
        base * latitude_area_factor(lat_deg)
    } else {
        base
    }
}

/// The culture vector a region's people drift toward.
///
/// Deserts breed hard, mobile peoples; forests breed devout scholars; coasts
/// breed merchants; grasslands breed communal herders. This is the seed of the
/// visible divergence the Milestone 1 gate checks.
pub fn biome_target(biome: Biome) -> [f32; DIMS] {
    match biome {
        Biome::Grassland => [0.35, 0.35, 0.45, 0.35, 0.70],
        Biome::Forest => [0.30, 0.75, 0.30, 0.65, 0.45],
        Biome::Beach => [0.45, 0.35, 0.85, 0.50, 0.45],
        Biome::Desert => [0.80, 0.40, 0.65, 0.30, 0.35],
        Biome::Mountain => [0.70, 0.55, 0.25, 0.40, 0.60],
        Biome::Ocean | Biome::Ice => [0.5; DIMS],
    }
}

/// The six neighbors of a tile, east-west wrapping (design, Grid).
pub fn neighbors(x: usize, y: usize) -> impl Iterator<Item = (usize, usize)> {
    crate::hex::neighbors(x, y)
}

/// Connected components of `culture_id`'s settled tiles, 8-neighbor.
///
/// Components are computed over *member* tiles only, so empty mountains,
/// deserts and oceans separate peoples (design, Membership). Ordering is
/// deterministic: tiles ascending, components by their smallest tile.
pub fn components(world: &World, culture_id: u16) -> Vec<Vec<usize>> {
    let tiles = GRID_WIDTH * GRID_HEIGHT;
    let mut seen = vec![false; tiles];
    let mut out = Vec::new();

    for start in 0..tiles {
        if seen[start] || world.culture_id[start] != culture_id || world.population[start] < 1.0 {
            continue;
        }
        let mut component = Vec::new();
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(i) = stack.pop() {
            component.push(i);
            let (x, y) = (i % GRID_WIDTH, i / GRID_WIDTH);
            for (nx, ny) in neighbors(x, y) {
                let n = ny * GRID_WIDTH + nx;
                if !seen[n] && world.culture_id[n] == culture_id && world.population[n] >= 1.0 {
                    seen[n] = true;
                    stack.push(n);
                }
            }
        }
        component.sort_unstable();
        out.push(component);
    }
    out
}

/// Population-weighted mean culture vector over `tiles`.
pub fn weighted_mean(world: &World, tiles: &[usize]) -> [f32; DIMS] {
    let mut sum = [0.0; DIMS];
    let mut weight = 0.0;
    for &i in tiles {
        let pop = world.population[i].max(0.0);
        weight += pop;
        for (k, slot) in sum.iter_mut().enumerate() {
            *slot += world.culture_vec[i * DIMS + k] * pop;
        }
    }
    if weight <= 0.0 {
        return [0.0; DIMS];
    }
    for slot in &mut sum {
        *slot /= weight;
    }
    sum
}

/// A component must cover at least this many tiles to become a daughter.
pub const SPLIT_MIN_TILES: usize = 30;

/// A component must hold at least this share of the parent's people.
pub const SPLIT_MIN_SHARE: f32 = 0.05;

/// A component must sit at least this far from the parent centroid.
pub const SPLIT_MIN_DISTANCE: f32 = 0.25;

/// A candidate must pass this many consecutive checks (hysteresis).
pub const SPLIT_PASSES: u8 = 2;

/// Refreshes each culture's cached aggregates and population-weighted centroid.
pub fn refresh_aggregates(world: &mut World) {
    let mut members = vec![Vec::new(); world.cultures.len()];
    for i in 0..world.population.len() {
        let id = world.culture_id[i];
        if id != NO_CULTURE && world.population[i] >= 1.0 {
            members[id as usize].push(i);
        }
    }
    for (culture, tiles) in world.cultures.iter_mut().zip(members.iter()) {
        culture.tiles = tiles.len() as u32;
    }
    // Walkers count toward their people without holding tile population
    // (D8): headcount, culture and belief, weighted like any tile's people.
    let mut walkers = vec![(0.0_f32, [0.0_f32; DIMS]); world.cultures.len()];
    for band in &world.bands {
        let (count, vec) = &mut walkers[band.culture_id as usize];
        *count += band.headcount;
        for (sum, value) in vec.iter_mut().zip(band.culture_vec) {
            *sum += value * band.headcount;
        }
    }
    let refreshed: Vec<(f32, [f32; DIMS])> = members
        .iter()
        .zip(&walkers)
        .map(|(tiles, (walking, walking_vec))| {
            let settled: f32 = tiles.iter().map(|&i| world.population[i]).sum();
            let mean = weighted_mean(world, tiles);
            let total = settled + walking;
            let centroid = std::array::from_fn(|k| {
                if total > 0.0 {
                    (mean[k] * settled + walking_vec[k]) / total
                } else {
                    0.0
                }
            });
            (total, centroid)
        })
        .collect();
    for (culture, (population, centroid)) in world.cultures.iter_mut().zip(refreshed) {
        culture.population = population;
        if population > 0.0 {
            culture.centroid = centroid;
        }
    }
    refresh_belief(world);
    // The people's home as mean (x, y): isolation and contact weigh a
    // tongue's drift (design, Tongues).
    for (culture, tiles) in world.cultures.iter_mut().zip(members.iter()) {
        let count = tiles.len() as f32;
        if count > 0.0 {
            let x: f32 = tiles.iter().map(|&i| (i % GRID_WIDTH) as f32).sum();
            let y: f32 = tiles.iter().map(|&i| (i / GRID_WIDTH) as f32).sum();
            culture.place = [x / count + 0.5, y / count + 0.5];
        }
    }
    // Trait means: what the people look like on average, for dress tints and
    // the traits overlay legend.
    let mut trait_sums = vec![[0.0_f32; crate::traits::TRAITS]; world.cultures.len()];
    for tile in 0..world.population.len() {
        let id = world.culture_id[tile];
        if id == NO_CULTURE || world.population[tile] < 1.0 {
            continue;
        }
        let start = tile * crate::traits::TRAITS;
        for (axis, sum) in trait_sums[id as usize].iter_mut().enumerate() {
            *sum += world.traits[start + axis];
        }
    }
    for (index, culture) in world.cultures.iter_mut().enumerate() {
        let tiles = members[index].len() as f32;
        if tiles > 0.0 {
            culture.mean_skin = trait_sums[index][crate::traits::SKIN] / tiles;
            culture.mean_stature = trait_sums[index][crate::traits::STATURE] / tiles;
            culture.mean_cold = trait_sums[index][crate::traits::COLD] / tiles;
            culture.mean_altitude = trait_sums[index][crate::traits::ALTITUDE] / tiles;
        }
    }
}

/// Belief reads per culture, population-weighted over its tiles and its
/// walkers (design: belief lives per tile; doctrine reads per culture; D8).
fn refresh_belief(world: &mut World) {
    let mut belief_sums = vec![[0.0_f32; crate::belief::AXES]; world.cultures.len()];
    let mut belief_weights = vec![0.0_f32; world.cultures.len()];
    for tile in 0..world.population.len() {
        let id = world.culture_id[tile];
        if id == NO_CULTURE || world.population[tile] < 1.0 {
            continue;
        }
        let weight = world.population[tile];
        belief_weights[id as usize] += weight;
        let start = tile * crate::belief::AXES;
        for (axis, slot) in belief_sums[id as usize].iter_mut().enumerate() {
            *slot += world.belief[start + axis] * weight;
        }
    }
    for band in &world.bands {
        let index = band.culture_id as usize;
        belief_weights[index] += band.headcount;
        for (slot, value) in belief_sums[index].iter_mut().zip(band.belief) {
            *slot += value * band.headcount;
        }
    }
    for (index, culture) in world.cultures.iter_mut().enumerate() {
        let weight = belief_weights[index];
        if weight > 0.0 {
            let axes: Vec<f32> = belief_sums[index].iter().map(|sum| sum / weight).collect();
            culture.belief = Belief::from_axes(&axes);
        }
        culture.piety = crate::faith::piety(&culture.belief);
    }
}

/// Checks every culture for a daughter people (design, Split rule).
///
/// Runs every 25 ticks. A candidate passing [`SPLIT_PASSES`] checks in a row
/// splits off as a new culture with the parent's id and a fresh name seed.
pub fn tick_splits(world: &mut World) {
    let cultures = world.cultures.len();
    for cid in 0..cultures {
        let Some((signature, _)) = split_candidate(world, cid as u16) else {
            world.split_watch[cid] = 0;
            continue;
        };
        world.split_watch[cid] += 1;
        if world.split_watch[cid] >= SPLIT_PASSES {
            world.split_watch[cid] = 0;
            split_off(world, cid as u16, signature);
        }
    }
}

/// The component of `culture_id` most likely to become a daughter, as
/// `(smallest tile index, population)`. The parent body (largest component)
/// never splits off from itself.
fn split_candidate(world: &World, culture_id: u16) -> Option<(usize, f32)> {
    let parts = components(world, culture_id);
    if parts.len() < 2 {
        return None;
    }
    let main = parts
        .iter()
        .enumerate()
        .max_by_key(|(_, part)| part.len())
        .map(|(i, _)| i)?;
    let parent_pop = world.cultures[culture_id as usize].population.max(1.0);
    // The parent centroid is the *body's* mean, not the whole culture's: after a
    // lobe drifts, a whole-culture mean sits between the two and no lobe ever
    // looks far enough away to split.
    let parent_centroid = weighted_mean(world, &parts[main]);

    let mut best: Option<(usize, f32)> = None;
    let body_tiles = parts[main].len();
    for (index, part) in parts.iter().enumerate() {
        // A daughter branches off the body; it never becomes the body.
        if index == main || part.len() < SPLIT_MIN_TILES || part.len() > body_tiles {
            continue;
        }
        let population: f32 = part.iter().map(|&i| world.population[i]).sum();
        if population < SPLIT_MIN_SHARE * parent_pop {
            continue;
        }
        if distance(&weighted_mean(world, part), &parent_centroid) < SPLIT_MIN_DISTANCE {
            continue;
        }
        if best.is_none_or(|(_, best_pop)| population > best_pop) {
            best = Some((part[0], population));
        }
    }
    best
}

/// Creates the daughter culture from the component starting at `signature`.
fn split_off(world: &mut World, parent: u16, signature: usize) {
    let Some(part) = components(world, parent)
        .into_iter()
        .find(|part| part.first() == Some(&signature) && part.len() >= SPLIT_MIN_TILES)
    else {
        return;
    };

    let population: f32 = part.iter().map(|&i| world.population[i]).sum();
    let centroid = weighted_mean(world, &part);
    let parent_culture = &world.cultures[parent as usize];

    // Fresh, not the 10-tick cache: a split must not hand favor to a rump.
    let parent_remaining: f32 = (0..world.population.len())
        .filter(|&i| world.culture_id[i] == parent && world.population[i] >= 1.0)
        .map(|i| world.population[i])
        .sum::<f32>()
        - population;

    // Favor follows the heir's tile from Milestone 3. Until there is a family
    // line, the parent body keeps Chosen unless the daughter has become the
    // larger people of the two.
    let favor_moves = parent_culture.favor == Favor::Chosen && population > parent_remaining;
    let favor = if favor_moves {
        Favor::Chosen
    } else {
        Favor::Other
    };

    let Some(id) = push_daughter(
        world,
        parent,
        centroid,
        favor,
        population,
        part.len() as u32,
    ) else {
        return;
    };
    if favor_moves {
        world.cultures[parent as usize].favor = Favor::Other;
    }
    for &i in &part {
        world.culture_id[i] = id;
    }
}

/// Founds a daughter people of `parent`, speaking a dialect of her mother's
/// tongue (design, Split rule; Tongues). Both splits and far-flung camps found
/// daughters this way (D20). `None` once every id below [`NO_CULTURE`] is
/// taken: the id is range-checked before the cast (outside voice).
pub(crate) fn push_daughter(
    world: &mut World,
    parent: u16,
    centroid: [f32; DIMS],
    favor: Favor,
    population: f32,
    tiles: u32,
) -> Option<u16> {
    let id = world.cultures.len();
    if id >= usize::from(NO_CULTURE) {
        return None;
    }
    let id = id as u16;
    let parent_seed = world.cultures[parent as usize].name_seed;
    let name_seed = mix(parent_seed ^ world.tick() ^ (u64::from(id) << 32));
    let daughter = Culture::daughter(
        &world.cultures[parent as usize],
        id,
        world.tick(),
        name_seed,
        centroid,
        favor,
        population,
        tiles,
    );
    world.cultures.push(daughter);
    world.split_watch.push(0);

    // A split is a split of tongue too: the daughter speaks a dialect that
    // descends from her mother's (design, Tongues).
    let parent_language = world.cultures[parent as usize].language_id;
    let tongue_id = world.languages.len() as u16;
    world.languages.push(crate::language::Language::daughter(
        tongue_id,
        parent_language,
        id,
        name_seed,
    ));
    world.cultures[id as usize].language_id = tongue_id;
    Some(id)
}

fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A color for a culture's overlay: golden-angle hue steps, so ids stay visibly
/// distinct (design, Map states).
pub fn color(id: u16) -> [u8; 3] {
    let hue = (f32::from(id) * 0.618_034).fract();
    hsv_rgb(hue, 0.62, 0.95)
}

/// Saturation-drained biome color for land nobody lives on (design, Map
/// states: unsettled land desaturated).
pub fn unsettled_color(biome: Biome) -> [u8; 3] {
    let [r, g, b] = crate::worldgen::biome_color(biome);
    let gray = 0.30 * f32::from(r) + 0.59 * f32::from(g) + 0.11 * f32::from(b);
    [
        (f32::from(r) * 0.25 + gray * 0.75) as u8,
        (f32::from(g) * 0.25 + gray * 0.75) as u8,
        (f32::from(b) * 0.25 + gray * 0.75) as u8,
    ]
}

/// HSV (h in [0, 1)) to RGB, sector math only, no transcendentals.
#[expect(
    clippy::many_single_char_names,
    reason = "standard HSV-to-RGB notation"
)]
fn hsv_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
    let h6 = h * 6.0;
    let sector = h6.floor() as i32 % 6;
    let f = h6 - h6.floor();
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match sector {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbor_count_shrinks_at_the_poles_but_wraps_east_west() {
        assert_eq!(neighbors(0, 0).count(), 4);
        assert_eq!(neighbors(0, 40).count(), 6);
        assert_eq!(neighbors(GRID_WIDTH - 1, 40).count(), 6, "east edge wraps");
    }

    #[test]
    fn culture_colors_are_distinct_for_the_first_few_ids() {
        assert_ne!(color(0), color(1));
        assert_ne!(color(1), color(2));
        assert_ne!(color(0), color(2));
    }
}
