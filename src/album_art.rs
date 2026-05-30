use std::path::Path;

use image::{Rgb, RgbImage};

use crate::error::ChtmError;

const SIZE: u32 = 512;

/// Write `album.png` for the song.
///
/// If `cover` holds decodable image bytes (an embedded cover extracted from the
/// audio file) it is re-encoded as PNG. Otherwise a deterministic placeholder
/// derived from the title is generated, so there is always an `album.png`.
pub fn write(cover: Option<&[u8]>, title: &str, out: &Path) -> Result<(), ChtmError> {
    if let Some(bytes) = cover
        && let Ok(img) = image::load_from_memory(bytes)
    {
        img.save(out)?;
        return Ok(());
    }

    placeholder(title).save(out)?;
    Ok(())
}

/// Build a 512×512 placeholder: a vertical gradient between two title-derived
/// colors with a centered disc, no text (keeps us font-dependency-free).
fn placeholder(title: &str) -> RgbImage {
    let seed = fnv1a(title.as_bytes());
    let top = color_from(seed);
    let bottom = color_from(seed.rotate_left(17) ^ 0x9E37_79B9_7F4A_7C15);
    let disc = color_from(seed.rotate_left(33).wrapping_mul(0x100_0000_01B3));

    let mut img = RgbImage::new(SIZE, SIZE);
    let cx = SIZE as f32 / 2.0;
    let cy = SIZE as f32 / 2.0;
    let radius = SIZE as f32 * 0.3;
    let radius_sq = radius * radius;

    for y in 0..SIZE {
        let t = y as f32 / (SIZE - 1) as f32;
        let bg = lerp_rgb(top, bottom, t);
        for x in 0..SIZE {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let pixel = if dx * dx + dy * dy <= radius_sq {
                disc
            } else {
                bg
            };
            img.put_pixel(x, y, Rgb(pixel));
        }
    }

    img
}

/// 64-bit FNV-1a hash — small, deterministic, no dependencies.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

/// Map a 64-bit value to a pleasant, reasonably saturated RGB color.
fn color_from(seed: u64) -> [u8; 3] {
    let hue = (seed % 360) as f32;
    hsv_to_rgb(hue, 0.55, 0.75)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
    let c = v * s;
    let h6 = h / 60.0;
    let x = c * (1.0 - (h6 % 2.0 - 1.0).abs());
    let (r, g, b) = match h6 as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    ]
}

fn lerp_rgb(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    [
        lerp(a[0], b[0], t),
        lerp(a[1], b[1], t),
        lerp(a[2], b[2], t),
    ]
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_is_512_square_and_deterministic() {
        let a = placeholder("Some Song");
        let b = placeholder("Some Song");
        assert_eq!(a.dimensions(), (SIZE, SIZE));
        assert_eq!(a.into_raw(), b.into_raw(), "same title -> identical image");
    }

    #[test]
    fn different_titles_produce_different_placeholders() {
        assert_ne!(placeholder("Alpha").into_raw(), placeholder("Beta").into_raw());
    }

    #[test]
    fn invalid_cover_bytes_fall_back_to_placeholder() {
        let out = std::env::temp_dir().join("chtm_album_test.png");
        // Not a decodable image -> must still produce a valid PNG placeholder.
        write(Some(b"not an image"), "Title", &out).expect("should write placeholder");
        let decoded = image::open(&out).expect("output must be a valid image");
        assert_eq!(decoded.width(), SIZE);
        let _ = std::fs::remove_file(&out);
    }
}
