//! Saves and replays (design, Determinism; Snapshot contents; R10, R11).
//!
//! R10's form: no stored golden hash to re-bless after tuning. The same seed
//! and actions run twice must match, a save taken mid-run must continue to
//! the same history, and replaying the seed and action log must rebuild it.

use sim_core::powers::Power;
use sim_core::save::{self, LoadError, SAVE_VERSION};
use sim_core::world::{GRID_WIDTH, World};

const SEED: u64 = 7;

/// A player's hand: a few powers at fixed years, aimed at the Chosen's home.
fn act(world: &mut World) {
    let Some(home) = world.line_home(sim_core::culture::Line::Dirt) else {
        return;
    };
    let (x, y) = (home % GRID_WIDTH, home / GRID_WIDTH);
    match world.tick() {
        150 => world.queue_power(Power::Bless, x, y),
        420 => world.queue_power(Power::Meteor, x + 3, y),
        640 => world.queue_power(Power::Raise, x, y + 2),
        810 => world.queue_power(Power::Tornado, x + 1, y),
        _ => {}
    }
}

/// Plays from the world's current year to `until`, one year at a time.
fn play(world: &mut World, until: u64) {
    while world.tick() < until {
        act(world);
        world.step(1);
    }
}

fn played(until: u64) -> World {
    let mut world = World::new(SEED);
    play(&mut world, until);
    world
}

#[test]
fn the_same_seed_and_actions_give_the_same_history() {
    let a = played(1_000);
    let b = played(1_000);
    assert!(a.actions().len() >= 4, "every scripted power applied");
    assert_eq!(save::state_hash(&a), save::state_hash(&b));
}

#[test]
fn a_save_taken_mid_run_continues_the_same_history() {
    let straight = played(1_000);

    let mut before = World::new(SEED);
    play(&mut before, 500);
    let bytes = save::save(&before);
    drop(before);
    let (header, mut resumed) = save::load(&bytes).expect("this build loads its own save");
    assert_eq!(header.tick, 500);
    assert_eq!(header.seed, SEED);
    play(&mut resumed, 1_000);

    assert_eq!(save::state_hash(&resumed), save::state_hash(&straight));
}

#[test]
fn a_power_queued_before_saving_still_lands_after_loading() {
    // Saved between the click and the next tick: the queued action is part of
    // the snapshot (design, Snapshot contents: pending queued actions).
    let mut straight = played(300);
    let mut paused = played(300);
    let home = straight
        .line_home(sim_core::culture::Line::Dirt)
        .expect("Dirt is planted");
    let (x, y) = (home % GRID_WIDTH, home / GRID_WIDTH);
    straight.queue_power(Power::Meteor, x, y);
    paused.queue_power(Power::Meteor, x, y);

    let (_, mut resumed) = save::load(&save::save(&paused)).expect("loads");
    straight.step(40);
    resumed.step(40);
    assert_eq!(resumed.actions(), straight.actions());
    assert_eq!(save::state_hash(&resumed), save::state_hash(&straight));
}

#[test]
fn replaying_the_seed_and_action_log_rebuilds_the_world() {
    let live = played(1_000);
    let replayed = save::replay(SEED, live.actions(), 1_000);
    assert_eq!(save::state_hash(&replayed), save::state_hash(&live));
}

#[test]
fn a_loaded_world_saves_back_to_the_same_bytes() {
    let world = played(600);
    let bytes = save::save(&world);
    let (_, loaded) = save::load(&bytes).expect("loads");
    assert_eq!(
        save::save(&loaded),
        bytes,
        "nothing is lost or rounded in a round trip"
    );
}

#[test]
fn the_header_reads_without_the_world() {
    let world = played(250);
    let header = save::read_header(&save::save(&world)).expect("a header");
    assert_eq!(header.seed, SEED);
    assert_eq!(header.tick, 250);
    assert_eq!(header.sim_build, save::sim_build());
    assert!(
        save::can_replay(&header),
        "this build replays its own saves"
    );

    let other = save::Header {
        sim_build: format!("{}-patched", header.sim_build),
        ..header
    };
    assert!(
        !save::can_replay(&other),
        "another build may load but not replay (R11)"
    );
}

#[test]
fn a_save_from_another_format_is_refused_with_its_version() {
    let mut bytes = save::save(&played(50));
    for found in [SAVE_VERSION - 1, SAVE_VERSION + 1] {
        bytes[4..8].copy_from_slice(&found.to_le_bytes());
        assert_eq!(save::load(&bytes).err(), Some(LoadError::Version { found }));
        assert_eq!(
            save::read_header(&bytes).err(),
            Some(LoadError::Version { found })
        );
    }
    let older = LoadError::Version {
        found: SAVE_VERSION - 1,
    };
    assert_eq!(
        older.to_string(),
        format!(
            "This world is from an older version (format {}).",
            SAVE_VERSION - 1
        )
    );
}

#[test]
fn anything_else_is_refused_without_panicking() {
    assert_eq!(save::load(b"").err(), Some(LoadError::NotASave));
    assert_eq!(
        save::load(b"PNG\x89 not a world").err(),
        Some(LoadError::NotASave)
    );
    let bytes = save::save(&played(50));
    let cut = &bytes[..bytes.len() / 2];
    assert!(matches!(save::load(cut), Err(LoadError::Corrupt(_))));
    let mut padded = bytes.clone();
    padded.push(0);
    assert!(matches!(save::load(&padded), Err(LoadError::Corrupt(_))));
}
