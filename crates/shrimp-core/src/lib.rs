//! Shrimp Player's core (SPEC §4).
//!
//! This crate has no UI and no platform code, so everything in it can be
//! tested headless on think. It must never depend on Slint, JNI or any
//! OS-specific crate (SPEC §3.1).

pub mod queue;
pub mod track;

pub use queue::{Previous, Queue, Repeat};
pub use track::TrackId;
