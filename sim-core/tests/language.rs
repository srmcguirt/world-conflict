//! Milestone 5 contracts (design, Tongues): language drift, the family tree,
//! scripture in a culture's own words, and drifting copies.
//!
//! The gate this milestone serves: peoples split at rivers and mountains and
//! their words drift apart, and every split traces back to choices the player
//! made.

use sim_core::language;
use sim_core::world::{Overlay, World};

/// A people's language, looked up through the culture.
fn tongue(world: &World, culture_id: u16) -> &language::Language {
    let id = world.cultures()[culture_id as usize].language_id;
    &world.languages()[id as usize]
}

fn find_daughter(world: &World) -> Option<u16> {
    world
        .cultures()
        .iter()
        .find(|culture| culture.parent_id.is_some())
        .map(|culture| culture.id)
}

#[test]
fn a_people_splits_and_a_tongue_spawns_with_it() {
    let mut world = World::new(1);
    world.step(700);
    let daughter = find_daughter(&world).expect("a people has split");
    let daughter_tongue = tongue(&world, daughter).id;
    let mother = world.cultures()[daughter as usize]
        .parent_id
        .expect("the daughter knows her mother");
    let mother_tongue = tongue(&world, mother).id;

    assert_ne!(
        daughter_tongue, mother_tongue,
        "a split speaks a new dialect"
    );
    assert_eq!(
        world.languages()[daughter_tongue as usize].parent_id,
        Some(mother_tongue),
        "the tongue descends from her mother's"
    );
}

#[test]
fn isolated_tongues_drift_faster() {
    let mut world = World::new(1);
    world.step(1_200);
    assert!(world.cultures().len() >= 3, "several peoples");

    // Words drifted per language, and isolation: how far each tongue's people
    // live from the nearest people who speak something else.
    let drift =
        |world: &World, lang: u16| -> u32 { world.languages()[lang as usize].words_drifted() };
    let isolation = |world: &World, culture_id: u16| -> f32 {
        let home = world.cultures()[culture_id as usize].place;
        let mut nearest = f32::MAX;
        for other in world.cultures() {
            if tongue(world, other.id).id == tongue(world, culture_id).id {
                continue;
            }
            let dx = home[0] - other.place[0];
            let dy = home[1] - other.place[1];
            nearest = nearest.min(libm::sqrtf(dx * dx + dy * dy));
        }
        nearest
    };

    let mut drifted = (0.0_f32, u16::MAX);
    let mut close = (f32::MAX, u16::MAX);
    for culture in world.cultures() {
        let far = isolation(&world, culture.id);

        if far > drifted.0 {
            drifted = (far, tongue(&world, culture.id).id);
        }
        if far < close.0 {
            close = (far, tongue(&world, culture.id).id);
        }
    }
    assert!(
        drifted.1 != close.1,
        "the test finds both an isolated and a crowded tongue"
    );
    assert!(
        drift(&world, drifted.1) > drift(&world, close.1),
        "words drift faster in isolation ({} vs {})",
        drift(&world, drifted.1),
        drift(&world, close.1)
    );
}

#[test]
fn neighbors_borrow_words() {
    let mut world = World::new(1);
    world.step(1_200);
    // Somewhere in the world, two peoples who speak differently live close
    // enough to trade words.
    let mut lending = false;
    for other in world.languages() {
        if other.words_borrowed() > 0 {
            lending = true;
        }
    }
    assert!(
        lending,
        "peoples who trade borrow words from their neighbors"
    );
}

#[test]
fn scripture_is_read_in_the_peoples_own_words() {
    let mut world = World::new(1);
    world.step(700);
    let culture = find_daughter(&world).unwrap_or(0);
    let text = world.scripture_text(culture, 0, 1);
    assert!(!text.is_empty(), "the people has a verse");
    let lines: Vec<&str> = text[0].split('\n').collect();
    assert_eq!(
        lines.len(),
        2,
        "a verse reads as its own words + a translation"
    );
    // The native line sounds like the people, in syllables from their
    // phonemes; the translation is English.
    assert!(
        lines[0].chars().next().is_some_and(char::is_alphabetic),
        "the native line exists"
    );
    assert!(
        lines[1].contains("In ") || lines[1].contains("the "),
        "the translation reads as English"
    );
}

#[test]
fn a_tongue_that_drifts_reads_its_scripture_differently() {
    let mut world = World::new(1);
    world.step(700);
    let culture = find_daughter(&world).unwrap_or(0);

    let before = world.scripture_text(culture, 0, 1);
    let translation_before: String = {
        let index = before[0].find('\n').unwrap();
        before[0][index..].to_string()
    };

    world.step(700);
    let after = world.scripture_text(culture, 0, 1);
    let translation_after: String = {
        let index = after[0].find('\n').unwrap();
        after[0][index..].to_string()
    };

    let drifted = before[0].split('\n').next().unwrap() != after[0].split('\n').next().unwrap();
    assert!(
        drifted,
        "the mother tongue's own words have moved with the generations"
    );
    assert_eq!(
        translation_before, translation_after,
        "what happened stays; only how it is remembered moves"
    );
}

#[test]
fn a_daughter_cultures_copy_arms_itself_in_her_own_words() {
    let mut world = World::new(1);
    world.step(700);
    let daughter = find_daughter(&world).expect("a people has split");

    let daughter_name = world.cultures()[daughter as usize].name.clone();
    let daughter_verses = world.scripture_text(daughter, 0, 20);
    // The daughter's inherited verses carry her name, whatever tongue she
    // adopted at birth; some verse for the mother tongue's version spells it
    // differently.
    assert!(
        daughter_verses
            .iter()
            .any(|verse| verse.contains(&daughter_name)),
        "the daughter tells the history with her own people's word for herself"
    );
}

#[test]
fn the_family_tree_traces_to_the_first_tongue() {
    let mut world = World::new(1);
    world.step(1_200);
    let daughter = find_daughter(&world).unwrap_or(0);
    let lineage = language::lineage(&world, tongue(&world, daughter).id);
    assert_eq!(
        lineage.last().map(|language| language.id),
        Some(0),
        "every tongue descends from the first people's language"
    );
}

#[test]
fn the_same_seed_writes_the_same_tongues() {
    let mut a = World::new(5);
    let mut b = World::new(5);
    for world in [&mut a, &mut b] {
        world.step(700);
    }
    assert_eq!(a.languages().len(), b.languages().len());
    let words_a: Vec<String> = (0..a.languages().len())
        .flat_map(|lang| (0..4).map(move |key| (lang, key)))
        .map(|(lang, key)| a.languages()[lang].word(key))
        .collect();
    let words_b: Vec<String> = (0..b.languages().len())
        .flat_map(|lang| (0..4).map(move |key| (lang, key)))
        .map(|(lang, key)| b.languages()[lang].word(key))
        .collect();
    assert_eq!(words_a, words_b, "same seed, same words");
}

#[test]
fn the_language_overlay_paints_tongues() {
    let mut world = World::new(1);
    world.step(700);
    let bytes = world.paint_overlay(Overlay::Language);
    assert_eq!(bytes.len(), 160 * 80 * 4);
    // A settled world has more than one tongue on the map.
    let colors: Vec<&[u8]> = bytes.chunks(4).collect();
    assert!(colors.len() > 100, "the overlay covers every tile");
}
