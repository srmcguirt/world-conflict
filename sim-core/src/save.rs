//! Saves and replays (design, Determinism; Snapshot contents; R10, R11, R12).
//!
//! A save is the seed, the player's action log and a snapshot of the whole
//! world, all of which `World` already carries. On disk it is:
//!
//! ```text
//! b"WCSV" | SAVE_VERSION (u32, little-endian) | header | world
//! ```
//!
//! with the header and world in `postcard`'s compact binary form, so every
//! float comes back bit-exact. The magic and version come first and are read
//! before anything else is decoded: a save from another format is refused with
//! a message, never half-read.
//!
//! R11 decides the gates: `SAVE_VERSION` (bumped by hand whenever the
//! snapshot's layout changes) gates loading; `SIM_BUILD` gates only exact
//! replay, so saves survive ordinary patches.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::buildings;
use crate::powers::Action;
use crate::world::World;

/// The snapshot layout's version. Bump it whenever any type a `World` holds
/// gains, loses or reorders a field, or `postcard` would misread old saves.
///
/// 2: the two lines (`Culture::line`, the Garden and each line's home; Out of
/// the Garden step 1).
///
/// 3: walking bands and the settlement fields they need (Out of the Garden
/// step 2).
pub const SAVE_VERSION: u32 = 3;

const MAGIC: &[u8; 4] = b"WCSV";
const PREAMBLE: usize = MAGIC.len() + 4;

/// What a save says about itself, readable without loading the world: for the
/// Continue button and the replay gate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    pub sim_build: String,
    pub seed: u64,
    pub tick: u64,
}

/// Why a save could not be loaded; each reads as the message the design's
/// Load row shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    NotASave,
    /// A save from another snapshot format (R11).
    Version {
        found: u32,
    },
    Corrupt(String),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::NotASave => write!(f, "This file is not a saved world."),
            LoadError::Version { found } if *found < SAVE_VERSION => {
                write!(f, "This world is from an older version (format {found}).")
            }
            LoadError::Version { found } => {
                write!(f, "This world is from a newer version (format {found}).")
            }
            LoadError::Corrupt(why) => write!(f, "This saved world is damaged ({why})."),
        }
    }
}

/// This build of the sim: crate version, git commit and the building
/// catalog's hash (design, Snapshot contents; Data, not code).
pub fn sim_build() -> String {
    format!(
        "{}+{}+{:016x}",
        env!("CARGO_PKG_VERSION"),
        env!("SIM_GIT_HASH"),
        buildings::load_catalog().hash()
    )
}

/// The whole world as save-file bytes.
///
/// # Panics
///
/// Never in practice: every type a `World` holds serializes to bytes.
pub fn save(world: &World) -> Vec<u8> {
    let header = Header {
        sim_build: sim_build(),
        seed: world.seed(),
        tick: world.tick(),
    };
    let mut bytes = Vec::with_capacity(4 << 20);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&SAVE_VERSION.to_le_bytes());
    let header = postcard::to_stdvec(&header).expect("a header always serializes");
    let body = postcard::to_stdvec(world).expect("a world always serializes");
    bytes.extend_from_slice(&header);
    bytes.extend_from_slice(&body);
    bytes
}

/// The header alone, for showing a save before loading it.
///
/// # Errors
///
/// [`LoadError`] when the bytes aren't a save of this snapshot format.
pub fn read_header(bytes: &[u8]) -> Result<Header, LoadError> {
    let rest = check_preamble(bytes)?;
    let (header, _) = postcard::take_from_bytes::<Header>(rest)
        .map_err(|error| LoadError::Corrupt(error.to_string()))?;
    Ok(header)
}

/// The header and the world a save holds.
///
/// # Errors
///
/// [`LoadError`] when the bytes aren't a save, are from another snapshot
/// format, or are cut short or padded.
pub fn load(bytes: &[u8]) -> Result<(Header, World), LoadError> {
    let rest = check_preamble(bytes)?;
    let (header, body) = postcard::take_from_bytes::<Header>(rest)
        .map_err(|error| LoadError::Corrupt(error.to_string()))?;
    let (world, trailing) = postcard::take_from_bytes::<World>(body)
        .map_err(|error| LoadError::Corrupt(error.to_string()))?;
    if !trailing.is_empty() {
        return Err(LoadError::Corrupt(format!(
            "{} stray bytes",
            trailing.len()
        )));
    }
    Ok((header, world))
}

fn check_preamble(bytes: &[u8]) -> Result<&[u8], LoadError> {
    if bytes.len() < PREAMBLE || &bytes[..MAGIC.len()] != MAGIC {
        return Err(LoadError::NotASave);
    }
    let mut version = [0_u8; 4];
    version.copy_from_slice(&bytes[MAGIC.len()..PREAMBLE]);
    let found = u32::from_le_bytes(version);
    if found != SAVE_VERSION {
        return Err(LoadError::Version { found });
    }
    Ok(&bytes[PREAMBLE..])
}

/// Whether this build can replay a save's action log exactly (R11: replay is
/// guaranteed only on the same sim build).
pub fn can_replay(header: &Header) -> bool {
    header.sim_build == sim_build()
}

/// The world a seed and an action log produce by `until` (design: the same
/// seed plus the same actions gives the same history).
pub fn replay(seed: u64, actions: &[Action], until: u64) -> World {
    let mut world = World::new(seed);
    world.queue_actions(actions);
    let years = until.saturating_sub(world.tick());
    world.step(u32::try_from(years).unwrap_or(u32::MAX));
    world
}

/// A fingerprint of the entire world state, for the determinism tests (R10):
/// FNV-1a over the snapshot bytes, so nothing the save keeps can differ
/// without the hash differing.
///
/// # Panics
///
/// Never in practice: every type a `World` holds serializes to bytes.
pub fn state_hash(world: &World) -> u64 {
    let body = postcard::to_stdvec(world).expect("a world always serializes");
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in body {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}
