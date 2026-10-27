//! Tongues (design, Language; Milestone 5).
//!
//! Every people speaks a language: a phoneme inventory and a core vocabulary
//! of [`CORE_WORDS`] words, all generated, all deterministic. Words drift at a
//! rate scaled by isolation — a people alone with their god shifts the words
//! for themselves faster than a people on the trade road. Neighbors borrow
//! words from each other. Scripture is facts plus slots, so a verse rendered
//! through today's tongue reads differently as the tongue moves, while the
//! translation keeps the facts.
//!
//! Names still come from `names.rs` through the culture's phonemes; the
//! vocabulary holds what a people *calls things*, and the family tree lives in
//! [`Language::parent_id`].

use crate::names::{self, Phonemes};
use crate::world::World;

/// The vocabulary a language may draw on.
pub const CORE_WORDS: usize = 100;

/// How many words a tongue touches each check, before isolation scales it.
pub const DRIFT_BASE: u32 = 1;

/// How much isolation adds. The world's diagonal is about 180 tiles.
pub const DRIFT_ISOLATION: f32 = 2.0 / 60.0;

/// Borrowings happen when two peoples' homes sit within this many tiles.
pub const CONTACT_RADIUS: f32 = 14.0;

/// Lexeme keys: what a people's words are *for*.
///
/// The design's 100-word core; these sixteen are what scripture reads today.
/// The rest are spare — later systems (decree kinds, buildings, all of it)
/// pick keys rather than inventing words.
pub const LEX_YEAR: usize = 0;
pub const LEX_SKY: usize = 1; // the meteor
pub const LEX_STORM: usize = 2; // the tornado
pub const LEX_GREEN: usize = 3; // grass painted
pub const LEX_SPAWN: usize = 4; // beasts made
pub const LEX_LAND: usize = 5; // raise / lower
pub const LEX_GIFT: usize = 6; // blessing
pub const LEX_WRATH: usize = 7; // wrathful
pub const LEX_BENEVOLENT: usize = 8;
pub const LEX_CAPRICIOUS: usize = 9;
pub const LEX_PROTECTIVE: usize = 10;
pub const LEX_JEALOUS: usize = 11;
pub const LEX_ABSENT: usize = 12;
pub const LEX_FATE: usize = 13; // an anointing
pub const LEX_BEGIN: usize = 14; // the first people
pub const LEX_PEOPLE: usize = 15;

/// The word a power tells scripture about.
pub fn power_word(power: crate::powers::Power) -> usize {
    use crate::powers::Power;
    match power {
        Power::Bless => LEX_GIFT,
        Power::Meteor => LEX_SKY,
        Power::Tornado => LEX_STORM,
        Power::Spawn => LEX_SPAWN,
        Power::Raise | Power::Lower => LEX_LAND,
        Power::Paint(_) => LEX_GREEN,
    }
}

/// One language: phonemes, a drift counter per core word, and whose words it
/// borrowed.
#[derive(Clone, Debug)]
pub struct Language {
    pub id: u16,
    /// The tongue she descends from, `None` for the first people's language.
    pub parent_id: Option<u16>,
    /// The people who speak her; other peoples may too.
    pub speaker_id: u16,
    /// Phonemes she was born with (names generate from these anyway).
    pub(crate) phonemes: Phonemes,
    /// Her identity seed.
    pub(crate) seed: u64,
    /// Per-word drift counters: how many sound changes that word has taken.
    drifts: Vec<u32>,
    /// Words this tongue took from a neighbor: `(their language, their key)`,
    /// resolved at borrowing time into this tongue's own sounds.
    pub(crate) borrowed: Vec<Option<String>>,
}

impl Language {
    /// The first people's language.
    pub fn root(speaker_id: u16, seed: u64, phonemes: Phonemes) -> Self {
        Self {
            id: 0,
            parent_id: None,
            speaker_id,
            phonemes,
            seed,
            drifts: vec![0; CORE_WORDS],
            borrowed: vec![None; CORE_WORDS],
        }
    }

    /// A daughter tongue: her mother's inventory, her own name.
    pub fn daughter(id: u16, parent_id: u16, speaker_id: u16, seed: u64) -> Self {
        Self {
            id,
            parent_id: Some(parent_id),
            speaker_id,
            phonemes: Phonemes::seeded(seed),
            seed,
            drifts: vec![0; CORE_WORDS],
            borrowed: vec![None; CORE_WORDS],
        }
    }

    /// The vocabulary's word for `key`, as she says it today.
    pub fn word(&self, key: usize) -> String {
        let key = key.min(CORE_WORDS - 1);
        if let Some(word) = &self.borrowed[key] {
            return word.clone();
        }
        let drift = u64::from(self.drifts[key]).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        syllable_name(
            &self.phonemes,
            self.seed ^ drift ^ ((key as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)),
            2,
        )
    }

    /// How many sound changes this tongue has taken in total.
    pub fn words_drifted(&self) -> u32 {
        self.drifts.iter().sum()
    }

    /// How many words this tongue has taken from her neighbors.
    pub fn words_borrowed(&self) -> u32 {
        self.borrowed.iter().flatten().count() as u32
    }

    /// The tongue's own name.
    pub fn name(&self) -> String {
        syllable_name(&self.phonemes, self.seed ^ 0x1A5A_0000_0000_0001, 3)
    }
}

/// A word from phonemes, independent of the naming module's types.
///
/// Two syllables or three, capitalized: "Taku", "Rovetha".
pub fn syllable_name(phonemes: &Phonemes, seed: u64, syllables: usize) -> String {
    let mut name = String::new();
    for syllable in 0..syllables {
        let index = (syllable * 2) as u32;
        name.push_str(phonemes_name_consonant(phonemes, seed, index));
        name.push_str(phonemes_name_vowel(phonemes, seed, index + 1));
    }
    names::capitalize(&name)
}

// Thin accessors so the language module can use Phonemes without names.rs
// making the per-syllable pick logic public. Identical picking rules to
// `names::person_name`, which keeps every generated word in one family.
fn phonemes_name_consonant(phonemes: &Phonemes, seed: u64, index: u32) -> &str {
    phonemes.consonant_for(seed, index)
}

fn phonemes_name_vowel(phonemes: &Phonemes, seed: u64, index: u32) -> &str {
    phonemes.vowel_for(seed, index)
}

/// The tongues in the world, indexed by language id.
pub(crate) fn tick_languages(world: &mut World) {
    if world.languages.is_empty() {
        return;
    }
    // Isolation: how far each culture's home is from the nearest people who
    // speak a different tongue.
    let speakers: Vec<u16> = world
        .cultures
        .iter()
        .map(|culture| culture.language_id)
        .collect();
    let homes: Vec<[f32; 2]> = world.cultures.iter().map(|culture| culture.place).collect();

    for index in 0..world.languages.len() {
        let own = index as u16;
        let home = homes[speakers[index] as usize];
        let mut nearest = f32::INFINITY;
        let mut neighbor: Option<usize> = None;
        for (other, &tongue_id) in speakers.iter().enumerate() {
            if tongue_id == own {
                continue;
            }
            let dx = home[0] - homes[other][0];
            let dy = home[1] - homes[other][1];
            let distance = libm::sqrtf(dx * dx + dy * dy);
            if distance < nearest {
                nearest = distance;
                neighbor = Some(other);
            }
        }
        if nearest.is_infinite() {
            nearest = 180.0; // alone on the map: maximum isolation
        }

        // Drift, scaled by isolation (design: words drift at a rate scaled by
        // isolation; contact blends vocabularies).
        let shifted = (DRIFT_BASE as f32 + nearest * DRIFT_ISOLATION).round() as u32;
        for nth in 0..shifted {
            let key = (names::hash(&format!("{own}-{nth}-{}", world.tick())) as usize) % CORE_WORDS;
            world.languages[index].drifts[key] += 1;
        }

        // Contact: people who live close hear their neighbors and borrow a
        // word.
        if nearest <= CONTACT_RADIUS {
            let neighbor =
                neighbor.filter(|&other| (speakers[other] as usize) < world.languages.len());
            if let Some(other) = neighbor {
                let their_tongue = speakers[other];
                let key =
                    (names::hash(&format!("borrow-{own}-{}", world.tick())) as usize) % CORE_WORDS;
                let their_key = (names::hash(&format!("give-{their_tongue}-{}", world.tick()))
                    as usize)
                    % CORE_WORDS;
                let word = world.languages[their_tongue as usize].word(their_key);
                world.languages[index].borrowed[key] = Some(word);
            }
        }
    }
}

/// The tongue family tree from `language_id` up to the first people's
/// language: daughter first, root last.
pub fn lineage(world: &World, language_id: u16) -> Vec<&Language> {
    let mut chain = Vec::new();
    let mut current = Some(language_id);
    while let Some(id) = current {
        let language = world.languages.get(id as usize);
        match language {
            Some(language) => {
                chain.push(language);
                current = language.parent_id;
            }
            None => break,
        }
    }
    chain
}

/// A verse's line in the speaker's own words.
pub fn native_line(world: &World, culture_id: u16, verse: &crate::scripture::Verse) -> String {
    let tongue_id = world
        .cultures
        .get(culture_id as usize)
        .map_or(0, |culture| culture.language_id);
    let language = &world.languages[tongue_id as usize];
    let lexeme = match verse.kind {
        crate::scripture::VerseKind::Genesis => LEX_BEGIN,
        crate::scripture::VerseKind::Power(power) => power_word(power),
        crate::scripture::VerseKind::Doctrine(doctrine) => match doctrine {
            crate::faith::Doctrine::Wrathful => LEX_WRATH,
            crate::faith::Doctrine::Benevolent => LEX_BENEVOLENT,
            crate::faith::Doctrine::Capricious => LEX_CAPRICIOUS,
            crate::faith::Doctrine::Protective => LEX_PROTECTIVE,
            crate::faith::Doctrine::Jealous => LEX_JEALOUS,
            crate::faith::Doctrine::Absent => LEX_ABSENT,
        },
        crate::scripture::VerseKind::Anoint { .. } => LEX_FATE,
    };
    format!(
        "{} {} {} {} {}",
        language.word(LEX_YEAR),
        language.word(lexeme),
        verse.place,
        verse.witness,
        language.word(LEX_PEOPLE)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tongue_speaks_stable_distinct_words() {
        let language = Language::root(0, 7, Phonemes::seeded(7));
        let a = language.word(LEX_SKY);
        let b = language.word(LEX_SKY);
        assert_eq!(a, b, "the same word today as yesterday");
        assert_ne!(language.word(LEX_SKY), language.word(LEX_YEAR));
        assert!(!language.name().is_empty());
    }

    #[test]
    fn drift_changes_the_word() {
        let mut language = Language::root(0, 7, Phonemes::seeded(7));
        let before = language.word(LEX_SKY);
        language.drifts[LEX_SKY] += 1;
        assert_ne!(
            before,
            language.word(LEX_SKY),
            "a shifted sound is a new word"
        );
    }

    #[test]
    fn a_borrowed_word_replaces_the_drift() {
        let mut language = Language::root(0, 7, Phonemes::seeded(7));
        let other = Language::daughter(1, 0, 1, 11);
        let their_word = other.word(LEX_SKY);
        language.borrowed[LEX_SKY] = Some(their_word.clone());
        assert_eq!(language.word(LEX_SKY), their_word);
    }
}
