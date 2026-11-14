//! Minimal PPM (P6) reader and writer for calibration dumps and `--image`.

use crate::frame::Image;
use crate::{Error, Result};
use std::path::Path;

pub fn encode_ppm(image: &Image) -> Vec<u8> {
    let mut out = format!("P6\n{} {}\n255\n", image.width, image.height).into_bytes();
    out.extend_from_slice(&image.rgb);
    out
}

pub fn decode_ppm(data: &[u8]) -> Result<Image> {
    if data.len() < 3 || &data[..2] != b"P6" {
        return Err(Error::Vision(
            "only binary P6 PPM images are supported".into(),
        ));
    }
    let mut i = 2usize;
    let mut tokens = Vec::with_capacity(3);
    while tokens.len() < 3 {
        skip_ws_and_comments(data, &mut i)?;
        let start = i;
        while i < data.len() && !data[i].is_ascii_whitespace() {
            i += 1;
        }
        if start == i {
            return Err(Error::Vision("truncated PPM header".into()));
        }
        let token = std::str::from_utf8(&data[start..i])
            .map_err(|_| Error::Vision("PPM header is not utf-8".into()))?;
        tokens.push(token.to_string());
    }
    if i < data.len() && data[i].is_ascii_whitespace() {
        i += 1;
    }
    let width: usize = tokens[0]
        .parse()
        .map_err(|_| Error::Vision(format!("bad PPM width '{}'", tokens[0])))?;
    let height: usize = tokens[1]
        .parse()
        .map_err(|_| Error::Vision(format!("bad PPM height '{}'", tokens[1])))?;
    let maxval: usize = tokens[2]
        .parse()
        .map_err(|_| Error::Vision(format!("bad PPM maxval '{}'", tokens[2])))?;
    if maxval != 255 {
        return Err(Error::Vision(format!(
            "PPM maxval {maxval} is unsupported; expected 255"
        )));
    }
    let need = width
        .checked_mul(height)
        .and_then(|n| n.checked_mul(3))
        .ok_or_else(|| Error::Vision("PPM dimensions overflow".into()))?;
    if data.len().saturating_sub(i) < need {
        return Err(Error::Vision(
            "PPM pixel data is shorter than the header claims".into(),
        ));
    }
    Image::new(width, height, data[i..i + need].to_vec())
}

fn skip_ws_and_comments(data: &[u8], i: &mut usize) -> Result<()> {
    loop {
        while *i < data.len() && data[*i].is_ascii_whitespace() {
            *i += 1;
        }
        if *i < data.len() && data[*i] == b'#' {
            while *i < data.len() && data[*i] != b'\n' {
                *i += 1;
            }
            continue;
        }
        if *i >= data.len() {
            return Err(Error::Vision("truncated PPM header".into()));
        }
        return Ok(());
    }
}

pub fn read_ppm(path: &Path) -> Result<Image> {
    let bytes = std::fs::read(path).map_err(|source| Error::io(path, source))?;
    decode_ppm(&bytes)
}

pub fn write_ppm(path: &Path, image: &Image) -> Result<()> {
    std::fs::write(path, encode_ppm(image)).map_err(|source| Error::io(path, source))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_preserves_pixels() {
        let mut image = Image::solid(3, 2, [1, 2, 3]).unwrap();
        image.fill_rect(1, 1, 1, 1, [9, 8, 7]);
        let decoded = decode_ppm(&encode_ppm(&image)).unwrap();
        assert_eq!(decoded, image);
    }

    #[test]
    fn header_comments_are_skipped() {
        let mut data = b"P6\n# calibration dump\n2 1\n255\n".to_vec();
        data.extend_from_slice(&[10, 20, 30, 40, 50, 60]);
        let image = decode_ppm(&data).unwrap();
        assert_eq!(image.width, 2);
        assert_eq!(image.pixel(1, 0), Some([40, 50, 60]));
    }
}
