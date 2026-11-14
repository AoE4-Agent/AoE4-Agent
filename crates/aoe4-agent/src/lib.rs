//! AoE4 Agent library: minimap color detection, taunt decisions, and
//! build-order coaching. Windows-only capture and input live in
//! [`platform`] and are compiled out of non-Windows builds.
//!
//! See `plan.md` for the design this crate implements.

#![deny(unsafe_code)]

pub mod coach;
pub mod config;
pub mod engine;
pub mod error;
pub mod frame;
pub mod guides;
pub mod platform;
pub mod ppm;
pub mod taunt;
pub mod vision;

pub use error::{Error, Result};
