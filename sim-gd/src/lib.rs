//! World Conflict's Godot bindings (gdext).
//!
//! The only crate that depends on Godot. It exposes [`api::Sim`] to `GDScript`
//! and holds no simulation state of its own: all state lives in `sim-core`
//! (design, Language decision).
#![expect(
    unsafe_code,
    reason = "gdext's entry point is an `unsafe impl`; this crate writes no other unsafe code"
)]

mod api;

use godot::prelude::*;

/// Extension entry point. Registers every `#[derive(GodotClass)]` type, so
/// `Sim` shows up in the editor and in `GDScript`.
struct WorldConflictExtension;

// SAFETY: gdext requires `unsafe impl` for the entry point; this crate adds no
// unsafe code of its own and defines no custom init levels.
#[gdextension]
unsafe impl ExtensionLibrary for WorldConflictExtension {}
