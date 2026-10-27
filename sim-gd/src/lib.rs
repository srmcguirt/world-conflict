//! World Conflict's Godot bindings (gdext).
//!
//! The only crate that depends on Godot. It exposes [`api::Sim`] to GDScript
//! and holds no simulation state of its own: all state lives in `sim-core`
//! (design, Language decision).

mod api;

use godot::prelude::*;

/// Extension entry point. Registers every `#[derive(GodotClass)]` type, so
/// `Sim` shows up in the editor and in GDScript.
struct WorldConflictExtension;

#[gdextension]
unsafe impl ExtensionLibrary for WorldConflictExtension {}
