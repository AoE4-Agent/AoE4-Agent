//! RGB to HSV, and inclusive HSV ranges that may wrap around hue 0.
//!
//! Hue is degrees in `[0, 360)`, saturation and value are `[0, 1]`.
//! A range with `h_min > h_max` wraps (red sits on both sides of 0).

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsv {
    pub h: f32,
    pub s: f32,
    pub v: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HsvRange {
    pub h_min: f32,
    pub h_max: f32,
    pub s_min: f32,
    pub s_max: f32,
    pub v_min: f32,
    pub v_max: f32,
}

impl HsvRange {
    pub fn contains(&self, hsv: Hsv) -> bool {
        if !hsv.h.is_finite() || !hsv.s.is_finite() || !hsv.v.is_finite() {
            return false;
        }
        if !unit_contains(hsv.s, self.s_min, self.s_max)
            || !unit_contains(hsv.v, self.v_min, self.v_max)
        {
            return false;
        }
        hue_contains(hsv.h, self.h_min, self.h_max)
    }

    pub fn validate(&self, name: &str) -> crate::Result<()> {
        for (label, value) in [("h_min", self.h_min), ("h_max", self.h_max)] {
            if !value.is_finite() || !(0.0..=360.0).contains(&value) {
                return Err(crate::Error::Config(format!(
                    "color {name} {label} must be a finite hue from 0 to 360, got {value}"
                )));
            }
        }
        for (label, lo, hi) in [("s", self.s_min, self.s_max), ("v", self.v_min, self.v_max)] {
            if !lo.is_finite()
                || !hi.is_finite()
                || !(0.0..=1.0).contains(&lo)
                || !(0.0..=1.0).contains(&hi)
                || lo > hi
            {
                return Err(crate::Error::Config(format!(
                    "color {name} {label} range must be inside 0..=1 with min <= max, got {lo}..={hi}"
                )));
            }
        }
        Ok(())
    }
}

fn unit_contains(value: f32, min: f32, max: f32) -> bool {
    value >= min && value <= max
}

fn hue_contains(h: f32, min: f32, max: f32) -> bool {
    let h = h.rem_euclid(360.0);
    if min <= 0.0 && max >= 360.0 {
        return true;
    }
    if min <= max {
        h >= min && h <= max
    } else {
        // Wrap: red is typically 345..=15.
        h >= min || h <= max
    }
}

pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> Hsv {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let mut h = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta).rem_euclid(6.0))
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    if h < 0.0 {
        h += 360.0;
    }
    if h >= 360.0 {
        h = 0.0;
    }
    let s = if max == 0.0 { 0.0 } else { delta / max };
    Hsv { h, s, v: max }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.05
    }

    #[test]
    fn primary_and_secondary_hues() {
        let red = rgb_to_hsv(255, 0, 0);
        assert!(near(red.h, 0.0) && near(red.s, 1.0) && near(red.v, 1.0));
        let green = rgb_to_hsv(0, 255, 0);
        assert!(near(green.h, 120.0) && near(green.s, 1.0));
        let blue = rgb_to_hsv(0, 0, 255);
        assert!(near(blue.h, 240.0) && near(blue.s, 1.0));
        let yellow = rgb_to_hsv(255, 255, 0);
        assert!(near(yellow.h, 60.0));
        let cyan = rgb_to_hsv(0, 255, 255);
        assert!(near(cyan.h, 180.0));
        let magenta = rgb_to_hsv(255, 0, 255);
        assert!(near(magenta.h, 300.0));
    }

    #[test]
    fn white_is_unsaturated_and_black_is_dark() {
        let white = rgb_to_hsv(255, 255, 255);
        assert!(near(white.s, 0.0) && near(white.v, 1.0));
        let black = rgb_to_hsv(0, 0, 0);
        assert!(near(black.s, 0.0) && near(black.v, 0.0));
    }

    #[test]
    fn red_range_wraps_through_zero() {
        let red = HsvRange {
            h_min: 345.0,
            h_max: 15.0,
            s_min: 0.4,
            s_max: 1.0,
            v_min: 0.3,
            v_max: 1.0,
        };
        assert!(red.contains(rgb_to_hsv(255, 0, 0)));
        assert!(red.contains(Hsv {
            h: 350.0,
            s: 0.8,
            v: 0.8
        }));
        assert!(red.contains(Hsv {
            h: 345.0,
            s: 0.4,
            v: 0.3
        }));
        assert!(red.contains(Hsv {
            h: 15.0,
            s: 1.0,
            v: 1.0
        }));
        assert!(!red.contains(Hsv {
            h: 16.0,
            s: 1.0,
            v: 1.0
        }));
        assert!(!red.contains(Hsv {
            h: 344.0,
            s: 1.0,
            v: 1.0
        }));
        assert!(!red.contains(rgb_to_hsv(0, 255, 0)));
        // Gray and very dark red are not enemy pixels.
        assert!(!red.contains(rgb_to_hsv(40, 40, 40)));
        assert!(!red.contains(rgb_to_hsv(40, 0, 0)));
    }

    #[test]
    fn full_hue_span_matches_every_hue() {
        let any = HsvRange {
            h_min: 0.0,
            h_max: 360.0,
            s_min: 0.0,
            s_max: 1.0,
            v_min: 0.0,
            v_max: 1.0,
        };
        assert!(any.contains(Hsv {
            h: 0.0,
            s: 0.0,
            v: 0.0
        }));
        assert!(any.contains(Hsv {
            h: 359.0,
            s: 1.0,
            v: 1.0
        }));
    }
}
