//! Generated names (design, Milestone 2 -> "generated names"; language ->
//! "a phoneme set, seeded at founding, used for names from Milestone 2").
//!
//! Every people draws its own consonant and vowel inventory from its founding
//! seed, so the Kareth and the Surani sound different without a dictionary.
//! Names are a pure function of `(name_seed, index)`: no RNG state, nothing to
//! serialize, and the same seed always yields the same names.

use crate::culture::Culture;

/// Consonant pool the inventories are drawn from.
const CONSONANTS: [&str; 22] = [
    "b", "d", "f", "g", "h", "k", "l", "m", "n", "p", "r", "s", "t", "v", "w", "y", "z", "th",
    "sh", "ch", "kh", "vr",
];

/// Vowel pool.
const VOWELS: [&str; 12] = [
    "a", "e", "i", "o", "u", "ae", "ei", "ou", "ia", "au", "ya", "ue",
];

/// What a people's language sounds like.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Phonemes {
    consonants: Vec<String>,
    vowels: Vec<String>,
}

impl Phonemes {
    /// Draws an inventory from `seed`.
    pub fn seeded(seed: u64) -> Self {
        let mut consonants = Vec::with_capacity(8);
        let mut vowels = Vec::with_capacity(5);
        for (i, sound) in CONSONANTS.iter().enumerate() {
            if !draw(seed, i as u32).is_multiple_of(3) {
                consonants.push((*sound).to_string());
            }
        }
        for (i, sound) in VOWELS.iter().enumerate() {
            if draw(seed ^ 0x9E37_79B9, i as u32).is_multiple_of(2) {
                vowels.push((*sound).to_string());
            }
        }
        // Every language needs something to build a syllable from.
        if consonants.len() < 3 {
            consonants.extend(["k", "r", "n"].iter().map(ToString::to_string));
        }
        if vowels.is_empty() {
            vowels.extend(["a", "i", "o"].iter().map(ToString::to_string));
        }
        Self { consonants, vowels }
    }

    pub(crate) fn consonant_for(&self, seed: u64, index: u32) -> &str {
        &self.consonants[(draw(seed, index) % self.consonants.len() as u32) as usize]
    }

    pub(crate) fn vowel_for(&self, seed: u64, index: u32) -> &str {
        &self.vowels[(draw(seed ^ 0xA5A5_5A5A, index) % self.vowels.len() as u32) as usize]
    }
}

/// A person's name, e.g. "Kareth" or "Suranmei".
pub fn person_name(culture: &Culture, index: u32) -> String {
    let seed = culture.name_seed ^ u64::from(index).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let syllables = 2 + (draw(seed, 99) % 2) as usize;
    let mut name = String::new();
    for syllable in 0..syllables {
        name.push_str(culture.phonemes.consonant_for(seed, syllable as u32 * 2));
        name.push_str(culture.phonemes.vowel_for(seed, syllable as u32 * 2 + 1));
    }
    if draw(seed, 100).is_multiple_of(2) {
        name.push_str(culture.phonemes.consonant_for(seed, 200));
    }
    capitalize(&name)
}

/// A people's own name for itself, e.g. "Karethi". Templates add "the".
pub fn people_name(culture: &Culture) -> String {
    let seed = culture.name_seed ^ 0x5EED_0000_0000_0001;
    let mut name = String::new();
    for syllable in 0..3 {
        name.push_str(culture.phonemes.consonant_for(seed, syllable * 2));
        name.push_str(culture.phonemes.vowel_for(seed, syllable * 2 + 1));
    }
    capitalize(&name)
}

/// A place's name, stable per tile.
pub fn place_name(culture: &Culture, tile: usize) -> String {
    let seed = culture.name_seed ^ (tile as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    let mut name = String::new();
    for syllable in 0..2 {
        name.push_str(culture.phonemes.consonant_for(seed, syllable * 2));
        name.push_str(culture.phonemes.vowel_for(seed, syllable * 2 + 1));
    }
    name.push_str(culture.phonemes.consonant_for(seed, 42));
    capitalize(&name)
}

/// A stable hash of a name, used to pick between verse variants.
pub fn hash(text: &str) -> u32 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for byte in text.as_bytes() {
        h ^= u64::from(*byte);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    (h >> 32) as u32
}

/// The god has one name, and every people uses it (design, scripture).
pub const GOD_NAME: &str = "the Watcher";

pub(crate) fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Deterministic integer hash (splitmix64 finalizer).
fn draw(seed: u64, index: u32) -> u32 {
    let mut z = seed
        .wrapping_add(u64::from(index).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add(0x1234_5678_9ABC_DEF0);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 32) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seeded_inventory_is_stable_and_pronounceable() {
        let a = Phonemes::seeded(7);
        let b = Phonemes::seeded(7);
        assert_eq!(a, b);
        assert!(!a.consonants.is_empty());
        assert!(!a.vowels.is_empty());
    }

    #[test]
    fn different_seeds_sound_different() {
        let a = Phonemes::seeded(1);
        let b = Phonemes::seeded(2);
        assert_ne!(a, b);
    }
}
