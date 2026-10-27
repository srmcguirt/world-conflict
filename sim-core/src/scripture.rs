//! Scripture: what the people wrote down (design, Scripture lives in Rust).
//!
//! A verse stores the facts and the slots, never the rendered sentence, so
//! language drift (Milestone 5) can re-render the same verse in a descendant's
//! words. GDScript only displays what `scripture_text` returns.

use crate::powers::Power;

/// How the witness read the event.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Polarity {
    /// It hurt them.
    Wrath,
    /// It helped them.
    Mercy,
    /// Nothing came of it.
    Silence,
}

/// One line of holy text.
#[derive(Clone, Debug, PartialEq)]
pub struct Verse {
    pub tick: u64,
    pub power: Power,
    pub polarity: Polarity,
    pub witness: String,
    pub place: String,
    pub people: String,
    pub witness_id: Option<u32>,
    /// How far this copy has drifted from the original (language, Milestone 5).
    pub mutation: u8,
    /// The opening verse of a scripture, not the record of a power.
    pub opening: bool,
}

impl Verse {
    /// Renders the verse in the people's words.
    ///
    /// `mutation` is 0 for Milestone 2; drift arrives with language.
    pub fn render(&self) -> String {
        let place = self.place.as_str();
        let people = format!("the {}", self.people);
        let witness = self.witness.as_str();
        let text = if self.opening {
            "In the beginning {god} set the first people at {place}, and they called themselves {people}. Act, and they will remember."
        } else {
            let lines = templates(self.power, self.polarity);
            let pick = (crate::names::hash(self.witness.as_str()) as usize) % lines.len();
            lines[pick]
        };
        text.replace("{year}", &self.tick.to_string())
            .replace("{witness}", witness)
            .replace("{place}", place)
            .replace("{people}", &people)
            .replace("{god}", crate::names::GOD_NAME)
    }
}

/// The lines a power can be remembered by. Kept short and plain: scripture is
/// written by the people who watched, not by the narrator.
fn templates(power: Power, polarity: Polarity) -> &'static [&'static str] {
    match (power, polarity) {
        (Power::Meteor, Polarity::Wrath) => &[
            "In year {year}, {witness} of {place} saw the sky catch fire. {people} say {god} broke the earth for its pride.",
            "In year {year}, a stone fell at {place} and the ground remembered it. {witness} said {god} is wrathful, and {people} do not go there.",
            "In year {year}, the sky opened over {place}. {witness} covered their eyes. {people} say {god} aimed at them and missed, or did not.",
        ],
        (Power::Meteor, Polarity::Mercy) => &[
            "In year {year}, fire fell on {place} and left {witness} standing. {people} say {god} struck their enemies and spared them.",
            "In year {year}, a stone burned the hill above {place}. {witness} said {god} judged the proud and passed over the poor.",
        ],
        (Power::Meteor, Polarity::Silence) => &[
            "In year {year}, a stone fell at {place} and no one was harmed. {witness} said nothing, and {people} wondered.",
            "In year {year}, the sky threw a stone at {place} that broke nothing. {people} asked {witness} what it meant, and {witness} had no answer.",
        ],
        (Power::Tornado, Polarity::Wrath) => &[
            "In year {year}, a wind walked through {place}. {witness} said {god} was angry, and {people} left offerings at the door.",
            "In year {year}, the wind took the roofs off {place}. {witness} named it the breath of {god}, and {people} feared the open sky.",
        ],
        (Power::Tornado, Polarity::Mercy) => &[
            "In year {year}, a wind turned aside from {place}. {witness} said {god} held it back with a hand, and {people} sang.",
            "In year {year}, the storm circled {place} and went on. {witness} said {god} had marked the houses of {people} and passed them by.",
        ],
        (Power::Tornado, Polarity::Silence) => &[
            "In year {year}, a wind crossed {place} and took nothing. {witness} said {people} had been weighed and found light.",
        ],
        (Power::Bless, Polarity::Mercy) => &[
            "In year {year}, the fields of {place} gave twice over. {witness} blessed {god}, and {people} feasted and remembered.",
            "In year {year}, {place} had more than it needed. {witness} said {god} is generous, and {people} brought the first grain to the altar.",
            "In year {year}, the sick of {place} rose from their beds. {witness} said {god} has not forgotten {people}, and they sang until morning.",
        ],
        (Power::Bless, Polarity::Wrath) => &[
            "In year {year}, {place} grew fat and then sick. {witness} said {god} tests {people} because they are loved.",
        ],
        (Power::Bless, Polarity::Silence) => &[
            "In year {year}, {witness} prayed over {place} and the rain did not change. {people} said nothing at all.",
        ],
        (Power::Raise, Polarity::Mercy) => &[
            "In year {year}, the ground rose at {place}. {witness} said {god} builds a wall for {people}, and they climbed it.",
            "In year {year}, a hill stood where the sea had been at {place}. {witness} said {god} makes room for {people}.",
        ],
        (Power::Raise, _) => &[
            "In year {year}, the ground rose at {place} and {people} lost the road. {witness} said {god} closes doors.",
        ],
        (Power::Lower, Polarity::Wrath) => &[
            "In year {year}, the ground opened at {place}. {witness} said {god} took the land back, and {people} mourned.",
            "In year {year}, the sea came in at {place}. {witness} said {god} drowned what {people} had built, and no one argued.",
        ],
        (Power::Lower, _) => &[
            "In year {year}, the land sank at {place} and the water came in. {people} say {god} made a bed for the sea.",
        ],
        (Power::Paint(_), Polarity::Mercy) => &[
            "In year {year}, {place} turned green in a single season. {witness} said {god} paints with a wide brush.",
            "In year {year}, flowers came up where {people} had never seen them, at {place}. {witness} said {god} is not finished with the world.",
        ],
        (Power::Paint(_), _) => &[
            "In year {year}, {place} withered in a single season. {witness} said {god} paints with a wide brush, and {people} feared it.",
        ],
        (Power::Spawn, Polarity::Wrath) => &[
            "In year {year}, beasts came out of the hills at {place}. {witness} said {god} set them loose, and {people} keep fires burning.",
            "In year {year}, something with teeth walked into {place}. {witness} named it the god's dog, and {people} built a fence and prayed.",
        ],
        (Power::Spawn, _) => &[
            "In year {year}, herds came to {place} and stayed. {witness} said {god} feeds {people}, and they followed the herd.",
        ],
    }
}

/// The verse that opens every scripture, at Eden (design, Interaction states).
pub fn genesis(people: &str, place: &str) -> Verse {
    Verse {
        tick: 0,
        power: Power::Bless,
        polarity: Polarity::Mercy,
        witness: "the first people".to_string(),
        place: place.to_string(),
        people: people.to_string(),
        witness_id: None,
        mutation: 0,
        opening: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verse(power: Power, polarity: Polarity) -> Verse {
        Verse {
            tick: 12,
            power,
            polarity,
            witness: "Kareth".to_string(),
            place: "Suran".to_string(),
            people: "the Melka".to_string(),
            witness_id: Some(1),
            mutation: 0,
            opening: false,
        }
    }

    #[test]
    fn a_verse_names_its_year_witness_and_place() {
        let text = verse(Power::Meteor, Polarity::Wrath).render();
        assert!(text.contains("12"), "{text}");
        assert!(text.contains("Kareth"), "{text}");
        assert!(text.contains("Suran"), "{text}");
    }

    #[test]
    fn mercy_and_wrath_read_differently_for_the_same_power() {
        assert_ne!(
            verse(Power::Meteor, Polarity::Wrath).render(),
            verse(Power::Meteor, Polarity::Mercy).render()
        );
    }

    #[test]
    fn the_opening_verse_reads_as_a_beginning() {
        let text = genesis("Karethi", "Suran").render();
        assert!(text.starts_with("In the beginning"), "{text}");
        assert!(text.contains("Suran"));
        assert!(text.contains("the Karethi"));
    }

    #[test]
    fn every_power_and_polarity_renders_something() {
        for power in [
            Power::Raise,
            Power::Lower,
            Power::Paint(crate::worldgen::Biome::Forest),
            Power::Bless,
            Power::Meteor,
            Power::Tornado,
            Power::Spawn,
        ] {
            for polarity in [Polarity::Wrath, Polarity::Mercy, Polarity::Silence] {
                let text = verse(power, polarity).render();
                assert!(text.len() > 20, "{power:?}/{polarity:?} produced {text:?}");
            }
        }
    }
}
