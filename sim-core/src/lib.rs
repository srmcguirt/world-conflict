//! World Conflict simulation core.
//!
//! Pure simulation, no Godot types and no rendering state: everything the game
//! simulates lives here behind [`world::World`]. The Godot layer lives in the
//! `sim-gd` crate (design R1: two-crate workspace, so `cargo test` and
//! `cargo bench` always run headless).
//!
//! Determinism contracts:
//! - one tick = one sim-year in every era (D24);
//! - one seeded RNG per system, derived from the world seed;
//! - no `std` hash collections (R9, enforced by clippy `disallowed-types`);
//! - transcendental math through `libm` (design, Determinism scope).

pub mod belief;
pub mod buildings;
pub mod culture;
pub mod demography;
pub mod events;
pub mod faith;
pub mod hex;
pub mod language;
pub mod names;
pub mod notables;
pub mod powers;
pub mod save;
pub mod scripture;
pub mod settlements;
pub mod society;
pub mod traits;
pub mod world;
pub mod worldgen;

/// The simulation world. Re-exported so the gdext layer has one canonical path.
pub use world::World;
