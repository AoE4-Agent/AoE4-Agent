//! Pure-Rust minimap vision: HSV masking and connected components.
//! No image crate and no Windows types, so the same code runs in tests
//! on Linux and inside the capture loop on Windows.

mod components;
mod detect;
mod hsv;

pub use components::{connected_components, Blob};
pub use detect::{detect, DetectParams};
pub use hsv::{rgb_to_hsv, Hsv, HsvRange};
