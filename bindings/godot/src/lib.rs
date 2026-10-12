//! Godot extension for manykey: tells your game which physical keyboard each
//! key press came from. See `README.md` in this directory.

// The `key_event` signal carries seven values, each useful to a script, and the macro
// that registers it generates a function with one parameter per value plus `self`.
#![allow(clippy::too_many_arguments)]

mod keymap;
mod node;

use godot::prelude::*;

struct ManyKeyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for ManyKeyExtension {}
