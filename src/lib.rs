//! Rust reimplementation of the mkxp-z RGSS runtime core.
//!
//! The C++ player renders with OpenGL, mixes audio with OpenAL, and runs game
//! scripts on MRI. This crate ports the behavior those bindings sit on:
//! `Color` / `Tone` / `Rect` / `Table`, RGSSAD archives, the case-insensitive
//! filesystem, `mkxp.json` and `Game.ini`, the input repeat and direction
//! state machine, audio channel state, and a software `Graphics` compositor
//! for sprites, planes, windows, viewports, and XP tilemaps.
//!
//! Script execution (MRI), GPU scaling shaders, and device audio playback are
//! outside this crate. Bitmap text uses a built-in 3x5 glyph set so layout,
//! alignment, outline, and shadow do not need FreeType.

#![deny(unsafe_code)]

mod archive;
mod audio;
mod autotile;
mod bitmap;
mod blend;
mod color;
mod config;
mod error;
mod filesystem;
mod font;
mod input;
mod rect;
mod scene;
mod table;

pub use archive::Archive;
pub use audio::Audio;
pub use bitmap::Bitmap;
pub use color::{Color, Tone};
pub use config::Config;
pub use error::{Error, Result};
pub use filesystem::FileSystem;
pub use font::Font;
pub use input::{Button, Input};
pub use rect::Rect;
pub use scene::{Demo, Graphics, Handle, Plane, Sprite, Tilemap, Viewport, Window};
pub use table::Table;
