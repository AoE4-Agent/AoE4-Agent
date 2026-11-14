//! 4- and 8-connected component labeling via union-find.
//!
//! Only backward neighbors are linked, so each foreground pixel is unioned
//! with the pixels already visited. A second pass accumulates area and
//! centroid sums. Labels are deterministic: larger blobs come first, then
//! top-most, then left-most.

use crate::{Error, Result};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blob {
    pub area: usize,
    pub min_x: usize,
    pub min_y: usize,
    pub max_x: usize,
    pub max_y: usize,
    pub sum_x: u64,
    pub sum_y: u64,
}

impl Blob {
    pub fn centroid(&self) -> (f32, f32) {
        if self.area == 0 {
            return (0.0, 0.0);
        }
        (
            self.sum_x as f32 / self.area as f32,
            self.sum_y as f32 / self.area as f32,
        )
    }
}

pub fn connected_components(
    mask: &[bool],
    width: usize,
    height: usize,
    eight: bool,
) -> Result<Vec<Blob>> {
    if width == 0 || height == 0 {
        return Err(Error::Vision("mask dimensions must be non-zero".into()));
    }
    let expected = width
        .checked_mul(height)
        .ok_or_else(|| Error::Vision("mask dimensions overflow".into()))?;
    if mask.len() != expected {
        return Err(Error::Vision(format!(
            "mask length {} does not match {width}x{height}",
            mask.len()
        )));
    }

    let mut parent: Vec<usize> = (0..mask.len()).collect();
    let mut rank = vec![0u8; mask.len()];

    let neighbors: &[(isize, isize)] = if eight {
        &[(-1, 0), (0, -1), (-1, -1), (1, -1)]
    } else {
        &[(-1, 0), (0, -1)]
    };

    for y in 0..height {
        for x in 0..width {
            let i = y * width + x;
            if !mask[i] {
                continue;
            }
            for (dx, dy) in neighbors {
                let nx = x as isize + dx;
                let ny = y as isize + dy;
                if nx < 0 || ny < 0 || nx >= width as isize || ny >= height as isize {
                    continue;
                }
                let j = ny as usize * width + nx as usize;
                if mask[j] {
                    union(&mut parent, &mut rank, i, j);
                }
            }
        }
    }

    let mut acc: BTreeMap<usize, Blob> = BTreeMap::new();
    for y in 0..height {
        for x in 0..width {
            let i = y * width + x;
            if !mask[i] {
                continue;
            }
            let root = find(&mut parent, i);
            let blob = acc.entry(root).or_insert(Blob {
                area: 0,
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
                sum_x: 0,
                sum_y: 0,
            });
            blob.area += 1;
            blob.min_x = blob.min_x.min(x);
            blob.min_y = blob.min_y.min(y);
            blob.max_x = blob.max_x.max(x);
            blob.max_y = blob.max_y.max(y);
            blob.sum_x += x as u64;
            blob.sum_y += y as u64;
        }
    }

    let mut blobs: Vec<Blob> = acc.into_values().collect();
    blobs.sort_by(|a, b| {
        b.area
            .cmp(&a.area)
            .then(a.min_y.cmp(&b.min_y))
            .then(a.min_x.cmp(&b.min_x))
    });
    Ok(blobs)
}

fn find(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

fn union(parent: &mut [usize], rank: &mut [u8], a: usize, b: usize) {
    let mut ra = find(parent, a);
    let mut rb = find(parent, b);
    if ra == rb {
        return;
    }
    if rank[ra] < rank[rb] {
        std::mem::swap(&mut ra, &mut rb);
    }
    parent[rb] = ra;
    if rank[ra] == rank[rb] {
        rank[ra] = rank[ra].saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mask_from(rows: &[&str]) -> (Vec<bool>, usize, usize) {
        let height = rows.len();
        let width = rows[0].len();
        let mask = rows
            .iter()
            .flat_map(|row| row.chars().map(|c| c == '1'))
            .collect();
        (mask, width, height)
    }

    #[test]
    fn diagonal_pixels_depend_on_connectivity() {
        let (mask, w, h) = mask_from(&["10", "01"]);
        let four = connected_components(&mask, w, h, false).unwrap();
        let eight = connected_components(&mask, w, h, true).unwrap();
        assert_eq!(four.len(), 2);
        assert_eq!(eight.len(), 1);
        assert_eq!(eight[0].area, 2);
    }

    #[test]
    fn separated_runs_stay_separate() {
        let (mask, w, h) = mask_from(&["111000111"]);
        let blobs = connected_components(&mask, w, h, true).unwrap();
        assert_eq!(blobs.len(), 2);
        assert_eq!(blobs[0].area, 3);
        assert_eq!(blobs[1].area, 3);
        assert_eq!(blobs[0].min_x, 0);
        assert_eq!(blobs[1].min_x, 6);
    }

    #[test]
    fn ring_merges_into_one_blob() {
        let (mask, w, h) = mask_from(&["111", "101", "111"]);
        let blobs = connected_components(&mask, w, h, false).unwrap();
        assert_eq!(blobs.len(), 1);
        assert_eq!(blobs[0].area, 8);
        let (cx, cy) = blobs[0].centroid();
        assert!((cx - 1.0).abs() < 0.01, "{cx}");
        assert!((cy - 1.0).abs() < 0.01, "{cy}");
    }

    #[test]
    fn empty_and_single_pixel() {
        let (empty, w, h) = mask_from(&["000", "000"]);
        assert!(connected_components(&empty, w, h, true).unwrap().is_empty());
        let (one, w, h) = mask_from(&["010"]);
        let blobs = connected_components(&one, w, h, true).unwrap();
        assert_eq!(blobs.len(), 1);
        assert_eq!(blobs[0].area, 1);
        assert_eq!(blobs[0].centroid(), (1.0, 0.0));
    }

    #[test]
    fn length_mismatch_is_an_error() {
        let err = connected_components(&[true, false], 2, 2, true).unwrap_err();
        assert!(err.to_string().contains("mask length"));
    }
}
