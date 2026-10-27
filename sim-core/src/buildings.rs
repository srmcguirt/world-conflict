//! Building catalog and construction (design, Buildings and settlement
//! advancement; Milestone 6a).

use serde::{Deserialize, Serialize};

/// A building's era gate (0 = Stone, 1 = Bronze, 2 = Iron in Milestone 6a).
pub type Era = u8;

/// One level of one building line.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct BuildingLevel {
    pub name: String,
    pub era: Era,
    pub upkeep: u32,
    pub effect: String,
}

/// A building line: a sequence of levels that upgrade in place.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BuildingLine {
    pub id: u8,
    pub name: String,
    pub levels: Vec<BuildingLevel>,
}

/// The whole catalog, loaded from `sim-core/data/buildings.ron`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BuildingCatalog {
    pub lines: Vec<BuildingLine>,
}

impl BuildingCatalog {
    /// Parse the catalog from RON text.
    ///
    /// # Errors
    ///
    /// Returns a parse error if the text is not valid RON.
    pub fn from_ron(text: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(text)
    }

    /// A stable hash of the catalog's contents, for `SIM_BUILD`.
    pub fn hash(&self) -> u64 {
        // FNV-1a over the serialized form: deterministic, no platform deps.
        let text = ron::to_string(self).unwrap_or_default();
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in text.as_bytes() {
            h ^= u64::from(*byte);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }

    /// Number of building lines.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// The highest level available in a given era.
    pub fn max_level(&self, line: u8, era: Era) -> Option<u8> {
        self.lines
            .iter()
            .find(|l| l.id == line)?
            .levels
            .iter()
            .enumerate()
            .filter(|(_, level)| level.era <= era)
            .map(|(i, _)| i as u8)
            .next_back()
    }
}

/// A building slot's contents: which line, at which level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Building {
    pub line: u8,
    pub level: u8,
}

/// What a settlement is currently building.
#[derive(Clone, Debug)]
pub struct BuildJob {
    pub line: u8,
    pub target_level: u8,
    pub progress: f32,
    pub labor_needed: f32,
}

/// Load the catalog embedded at compile time.
///
/// # Panics
///
/// Panics if the embedded `buildings.ron` fails to parse.
pub fn load_catalog() -> BuildingCatalog {
    let text = include_str!("../data/buildings.ron");
    BuildingCatalog::from_ron(text).expect("buildings.ron parses at compile time")
}

/// The sim's build hash, from the catalog.
///
/// # Panics
///
/// Panics if the embedded `buildings.ron` fails to parse.
pub fn sim_build() -> u64 {
    load_catalog().hash()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalog_loads_and_has_five_lines() {
        let catalog = load_catalog();
        assert_eq!(catalog.line_count(), 5, "five building lines in M6a");
        for line in &catalog.lines {
            assert_eq!(
                line.levels.len(),
                3,
                "three levels per line in M6a (Stone/Bronze/Iron)"
            );
        }
    }

    #[test]
    fn the_catalog_hash_is_stable() {
        assert_eq!(sim_build(), sim_build(), "same catalog, same hash");
    }

    #[test]
    fn max_level_respects_era_gates() {
        let catalog = load_catalog();
        // Stone era (0): only level 0 is available.
        assert_eq!(catalog.max_level(0, 0), Some(0));
        // Bronze era (1): levels 0 and 1.
        assert_eq!(catalog.max_level(0, 1), Some(1));
        // Iron era (2): all three levels.
        assert_eq!(catalog.max_level(0, 2), Some(2));
    }

    #[test]
    fn the_catalog_round_trips_through_ron() {
        let catalog = load_catalog();
        let text = ron::to_string(&catalog).expect("serializes");
        let parsed = BuildingCatalog::from_ron(&text).expect("re-parses");
        assert_eq!(
            parsed.hash(),
            catalog.hash(),
            "round trip preserves the hash"
        );
    }
}
