//! Turn a minimap crop into the enemy blobs that sit near the town center.

use super::components::{connected_components, Blob};
use super::hsv::{rgb_to_hsv, HsvRange};
use crate::frame::{Image, TownCenter};
use crate::Result;

#[derive(Debug, Clone)]
pub struct DetectParams {
    pub enemies: Vec<HsvRange>,
    pub own: Option<HsvRange>,
    pub eight_connected: bool,
    pub min_cluster_pixels: usize,
    pub town_center: Option<TownCenter>,
}

pub fn detect(image: &Image, params: &DetectParams) -> Result<Vec<Blob>> {
    let mask = enemy_mask(image, &params.enemies, params.own.as_ref());
    let blobs = connected_components(&mask, image.width, image.height, params.eight_connected)?;
    let mut blobs: Vec<Blob> = blobs
        .into_iter()
        .filter(|blob| blob.area >= params.min_cluster_pixels)
        .collect();
    if let Some(tc) = params.town_center {
        let (cx, cy, radius) = tc.pixels(image.width, image.height);
        let radius_sq = radius * radius;
        blobs.retain(|blob| {
            let (x, y) = blob.centroid();
            let dx = x - cx;
            let dy = y - cy;
            dx * dx + dy * dy <= radius_sq
        });
    }
    Ok(blobs)
}

pub fn enemy_mask(image: &Image, enemies: &[HsvRange], own: Option<&HsvRange>) -> Vec<bool> {
    let mut mask = vec![false; image.width * image.height];
    for (i, px) in image.rgb.as_chunks::<3>().0.iter().enumerate() {
        let hsv = rgb_to_hsv(px[0], px[1], px[2]);
        if own.is_some_and(|range| range.contains(hsv)) {
            continue;
        }
        if enemies.iter().any(|range| range.contains(hsv)) {
            mask[i] = true;
        }
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Image;

    fn red_only() -> DetectParams {
        DetectParams {
            enemies: vec![HsvRange {
                h_min: 345.0,
                h_max: 15.0,
                s_min: 0.4,
                s_max: 1.0,
                v_min: 0.3,
                v_max: 1.0,
            }],
            own: Some(HsvRange {
                h_min: 205.0,
                h_max: 255.0,
                s_min: 0.35,
                s_max: 1.0,
                v_min: 0.25,
                v_max: 1.0,
            }),
            eight_connected: true,
            min_cluster_pixels: 8,
            town_center: Some(TownCenter {
                cx: 0.28,
                cy: 0.78,
                radius: 0.38,
            }),
        }
    }

    #[test]
    fn near_red_cluster_counts_and_far_red_does_not() {
        let params = red_only();
        let mut near = Image::solid(100, 100, [20, 20, 20]).unwrap();
        near.fill_rect(25, 75, 6, 6, [255, 0, 0]);
        let blobs = detect(&near, &params).unwrap();
        assert_eq!(blobs.len(), 1);
        assert_eq!(blobs[0].area, 36);
        let (x, y) = blobs[0].centroid();
        assert!((x - 27.5).abs() < 0.01);
        assert!((y - 77.5).abs() < 0.01);

        let mut far = Image::solid(100, 100, [20, 20, 20]).unwrap();
        far.fill_rect(90, 2, 6, 6, [255, 0, 0]);
        assert!(detect(&far, &params).unwrap().is_empty());
    }

    #[test]
    fn own_color_and_specks_are_ignored() {
        let params = red_only();
        let mut image = Image::solid(100, 100, [20, 20, 20]).unwrap();
        image.fill_rect(25, 75, 6, 6, [0, 0, 255]);
        image.fill_rect(26, 76, 2, 2, [255, 0, 0]);
        assert!(detect(&image, &params).unwrap().is_empty());
    }

    #[test]
    fn own_range_wins_when_it_overlaps_an_enemy_range() {
        let mut params = red_only();
        params.own = Some(params.enemies[0]);
        params.town_center = None;
        params.min_cluster_pixels = 1;
        let mut image = Image::solid(4, 4, [0, 0, 0]).unwrap();
        image.fill_rect(0, 0, 4, 4, [255, 0, 0]);
        assert!(detect(&image, &params).unwrap().is_empty());
    }
}
