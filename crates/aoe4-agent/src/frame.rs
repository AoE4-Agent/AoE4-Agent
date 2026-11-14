//! In-memory RGB frames and resolution-independent rectangles.
//!
//! Minimap placement is stored as fractions of the primary monitor so the
//! same config tracks 1080p, 1440p, and 4K. The game's own UI-scale slider
//! is a separate knob and is not inferred here.

use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<u8>,
}

impl Image {
    pub fn new(width: usize, height: usize, rgb: Vec<u8>) -> Result<Self> {
        if width == 0 || height == 0 {
            return Err(Error::Vision(
                "image width and height must be non-zero".into(),
            ));
        }
        let expected = width
            .checked_mul(height)
            .and_then(|n| n.checked_mul(3))
            .ok_or_else(|| Error::Vision("image dimensions overflow".into()))?;
        if rgb.len() != expected {
            return Err(Error::Vision(format!(
                "rgb buffer length {} does not match {width}x{height}",
                rgb.len()
            )));
        }
        Ok(Self { width, height, rgb })
    }

    pub fn solid(width: usize, height: usize, color: [u8; 3]) -> Result<Self> {
        let pixels = width
            .checked_mul(height)
            .ok_or_else(|| Error::Vision("image dimensions overflow".into()))?;
        let mut rgb = Vec::with_capacity(pixels.saturating_mul(3));
        for _ in 0..pixels {
            rgb.extend_from_slice(&color);
        }
        Self::new(width, height, rgb)
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: [u8; 3]) {
        let y1 = y.saturating_add(h).min(self.height);
        let x1 = x.saturating_add(w).min(self.width);
        for row in y..y1 {
            for col in x..x1 {
                let i = (row * self.width + col) * 3;
                self.rgb[i] = color[0];
                self.rgb[i + 1] = color[1];
                self.rgb[i + 2] = color[2];
            }
        }
    }

    pub fn pixel(&self, x: usize, y: usize) -> Option<[u8; 3]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y * self.width + x) * 3;
        Some([self.rgb[i], self.rgb[i + 1], self.rgb[i + 2]])
    }
}

/// Axis-aligned rectangle as fractions of a parent rectangle.
/// `x` and `y` are the top-left. Values are expected in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FractionRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Town-center watch disk, as fractions of the minimap crop.
/// `radius` is a fraction of the crop's shorter side.
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TownCenter {
    pub cx: f32,
    pub cy: f32,
    pub radius: f32,
}

impl TownCenter {
    pub fn pixels(&self, crop_w: usize, crop_h: usize) -> (f32, f32, f32) {
        let cx = self.cx * crop_w as f32;
        let cy = self.cy * crop_h as f32;
        let radius = self.radius * crop_w.min(crop_h) as f32;
        (cx, cy, radius)
    }
}

/// Map a fractional rectangle onto a monitor. The result is clamped so a
/// rounding error cannot address pixels outside the monitor.
pub fn fraction_to_pixels(parent_w: u32, parent_h: u32, frac: &FractionRect) -> PixelRect {
    let sw = parent_w.max(1);
    let sh = parent_h.max(1);
    let mut x = (frac.x * sw as f32).round().max(0.0) as u32;
    let mut y = (frac.y * sh as f32).round().max(0.0) as u32;
    let mut width = (frac.width * sw as f32).round().max(1.0) as u32;
    let mut height = (frac.height * sh as f32).round().max(1.0) as u32;
    if x >= sw {
        x = sw - 1;
    }
    if y >= sh {
        y = sh - 1;
    }
    if x.saturating_add(width) > sw {
        width = sw - x;
    }
    if y.saturating_add(height) > sh {
        height = sh - y;
    }
    PixelRect {
        x,
        y,
        width: width.max(1),
        height: height.max(1),
    }
}

pub fn fraction_in_unit(name: &str, value: f32) -> Result<()> {
    if (0.0..=1.0).contains(&value) && value.is_finite() {
        Ok(())
    } else {
        Err(Error::Config(format!(
            "{name} must be a finite fraction from 0 to 1, got {value}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractions_scale_across_resolutions() {
        let frac = FractionRect {
            x: 0.25,
            y: 0.50,
            width: 0.50,
            height: 0.25,
        };
        let hd = fraction_to_pixels(1920, 1080, &frac);
        assert_eq!(
            hd,
            PixelRect {
                x: 480,
                y: 540,
                width: 960,
                height: 270
            }
        );
        let uhd = fraction_to_pixels(3840, 2160, &frac);
        assert_eq!(uhd.x, hd.x * 2);
        assert_eq!(uhd.y, hd.y * 2);
        assert_eq!(uhd.width, hd.width * 2);
        assert_eq!(uhd.height, hd.height * 2);
    }

    #[test]
    fn fractions_stay_inside_common_monitors() {
        let frac = FractionRect {
            x: 0.006,
            y: 0.745,
            width: 0.172,
            height: 0.242,
        };
        for (w, h) in [
            (1280, 720),
            (1920, 1080),
            (2560, 1440),
            (3440, 1440),
            (3840, 2160),
        ] {
            let rect = fraction_to_pixels(w, h, &frac);
            assert!(rect.width >= 1 && rect.height >= 1, "{w}x{h}");
            assert!(rect.x + rect.width <= w, "{w}x{h}: {rect:?}");
            assert!(rect.y + rect.height <= h, "{w}x{h}: {rect:?}");
        }
    }

    #[test]
    fn town_center_radius_uses_the_shorter_side() {
        let tc = TownCenter {
            cx: 0.5,
            cy: 0.5,
            radius: 0.25,
        };
        let (cx, cy, radius) = tc.pixels(200, 100);
        assert!((cx - 100.0).abs() < 0.01);
        assert!((cy - 50.0).abs() < 0.01);
        assert!((radius - 25.0).abs() < 0.01);
    }
}
