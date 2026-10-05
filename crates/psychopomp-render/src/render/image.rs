//! Image decoding: a PNG, JPEG, or WebP decoded once (recognized by its
//! signature, not its name) and halved down to at most twice the width it is
//! shown at, so minification never aliases. Footage draws it.
use std::io::Cursor;

use anyhow::{Context, Result, bail};

/// Decoded straight-alpha RGBA8 pixels.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DecodedImage {
    pub pixels: Vec<u8>,
    pub size: [u32; 2],
}

/// Decode PNG, JPEG, or WebP `bytes` into straight-alpha RGBA8.
pub(crate) fn decode_image(bytes: &[u8]) -> Result<DecodedImage> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        decode_png(bytes)
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        decode_jpeg(bytes)
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        decode_webp(bytes)
    } else {
        bail!("images must be PNG, JPEG, or WebP")
    }
}

fn decode_png(bytes: &[u8]) -> Result<DecodedImage> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().context("read PNG header")?;
    let mut buffer = vec![0; reader.output_buffer_size().context("PNG is too large")?];
    let info = reader.next_frame(&mut buffer).context("decode PNG")?;
    let size = [info.width, info.height];
    let samples = &buffer[..info.buffer_size()];
    let pixels = match info.color_type {
        png::ColorType::Rgba => samples.to_vec(),
        png::ColorType::Rgb => opaque(samples),
        png::ColorType::GrayscaleAlpha => samples
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[value, alpha]| [value, value, value, alpha])
            .collect(),
        png::ColorType::Grayscale => samples.iter().flat_map(|&v| [v, v, v, 255]).collect(),
        png::ColorType::Indexed => bail!("indexed PNG was not expanded"),
    };
    checked(pixels, size)
}

fn decode_jpeg(bytes: &[u8]) -> Result<DecodedImage> {
    use zune_jpeg::zune_core::{colorspace::ColorSpace, options::DecoderOptions};
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    let mut decoder = zune_jpeg::JpegDecoder::new_with_options(Cursor::new(bytes), options);
    decoder
        .decode_headers()
        .map_err(|error| anyhow::anyhow!("read JPEG header: {error:?}"))?;
    if decoder.output_colorspace() != Some(ColorSpace::RGBA) {
        bail!("JPEG cannot be decoded to RGBA");
    }
    let pixels = decoder
        .decode()
        .map_err(|error| anyhow::anyhow!("decode JPEG: {error:?}"))?;
    let info = decoder.info().context("JPEG has no image info")?;
    checked(pixels, [u32::from(info.width), u32::from(info.height)])
}

fn decode_webp(bytes: &[u8]) -> Result<DecodedImage> {
    let mut decoder =
        image_webp::WebPDecoder::new(Cursor::new(bytes)).context("read WebP header")?;
    let mut buffer = vec![0; decoder.output_buffer_size().context("WebP is too large")?];
    decoder.read_image(&mut buffer).context("decode WebP")?;
    let (width, height) = decoder.dimensions();
    let pixels = if decoder.has_alpha() {
        buffer
    } else {
        opaque(&buffer)
    };
    checked(pixels, [width, height])
}

/// RGB samples as opaque RGBA.
fn opaque(rgb: &[u8]) -> Vec<u8> {
    rgb.as_chunks::<3>()
        .0
        .iter()
        .flat_map(|&[r, g, b]| [r, g, b, 255])
        .collect()
}

fn checked(pixels: Vec<u8>, size: [u32; 2]) -> Result<DecodedImage> {
    if size[0] == 0 || size[1] == 0 || pixels.len() != size[0] as usize * size[1] as usize * 4 {
        bail!("decoded image has no pixels or the wrong length");
    }
    Ok(DecodedImage { pixels, size })
}

impl DecodedImage {
    /// Halve (averaging 2×2 blocks with premultiplied alpha) while wider than
    /// twice `width`, so drawing it at `width` samples at most 2:1.
    pub(crate) fn reduced_for(mut self, width: f32) -> Self {
        while self.size[0] as f32 > width * 2.0 && self.size[0] > 1 && self.size[1] > 1 {
            self = self.halved();
        }
        self
    }

    fn halved(&self) -> Self {
        let [width, height] = self.size;
        let size = [width.div_ceil(2), height.div_ceil(2)];
        let pixel = |x: u32, y: u32| {
            let index =
                (y.min(height - 1) as usize * width as usize + x.min(width - 1) as usize) * 4;
            &self.pixels[index..index + 4]
        };
        let mut pixels = Vec::with_capacity(size[0] as usize * size[1] as usize * 4);
        for y in 0..size[1] {
            for x in 0..size[0] {
                let mut alpha = 0.0;
                let mut color = [0.0_f32; 3];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let p = pixel(x * 2 + dx, y * 2 + dy);
                    let a = f32::from(p[3]);
                    alpha += a;
                    for (sum, channel) in color.iter_mut().zip(p) {
                        *sum += f32::from(*channel) * a;
                    }
                }
                let straight = |sum: f32| {
                    if alpha > 0.0 {
                        (sum / alpha).round() as u8
                    } else {
                        0
                    }
                };
                pixels.extend([
                    straight(color[0]),
                    straight(color[1]),
                    straight(color[2]),
                    (alpha / 4.0).round() as u8,
                ]);
            }
        }
        Self { pixels, size }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{DecodedImage, decode_image};

    fn encode_png(pixels: &[u8], size: [u32; 2], color: png::ColorType) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(Cursor::new(&mut bytes), size[0], size[1]);
            encoder.set_color(color);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(pixels).unwrap();
        }
        bytes
    }

    #[test]
    fn pngs_decode_to_straight_rgba_by_signature() {
        let rgba = [255, 0, 0, 255, 0, 255, 0, 128];
        let decoded = decode_image(&encode_png(&rgba, [2, 1], png::ColorType::Rgba)).unwrap();
        assert_eq!(decoded.size, [2, 1]);
        assert_eq!(decoded.pixels, rgba);
        let rgb = encode_png(&[10, 20, 30, 40, 50, 60], [2, 1], png::ColorType::Rgb);
        assert_eq!(
            decode_image(&rgb).unwrap().pixels,
            [10, 20, 30, 255, 40, 50, 60, 255]
        );
        let gray = encode_png(&[7, 9], [1, 1], png::ColorType::GrayscaleAlpha);
        assert_eq!(decode_image(&gray).unwrap().pixels, [7, 7, 7, 9]);
        assert!(decode_image(b"GIF89a....").is_err());
        assert!(decode_image(b"\x89PNG\r\n\x1a\ntruncated").is_err());
    }

    #[test]
    fn checked_in_jpeg_and_webp_fixtures_decode() {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/anchors");
        for name in ["swatch.jpg", "swatch.webp"] {
            let decoded = decode_image(&std::fs::read(assets.join(name)).unwrap()).unwrap();
            assert_eq!(decoded.size, [64, 32], "{name}");
            // The left half is a saturated orange, the right a deep blue.
            let left = &decoded.pixels[(16 * 64 + 8) * 4..][..4];
            let right = &decoded.pixels[(16 * 64 + 56) * 4..][..4];
            assert!(
                left[0] > 200 && left[2] < 80 && left[3] == 255,
                "{name}: {left:?}"
            );
            assert!(right[2] > 150 && right[0] < 80, "{name}: {right:?}");
        }
    }

    #[test]
    fn reduction_halves_with_premultiplied_alpha_until_twice_the_width() {
        // A transparent pixel beside an opaque one must not darken the mix.
        let image = DecodedImage {
            pixels: vec![
                0, 0, 0, 0, 200, 100, 50, 255, //
                0, 0, 0, 0, 200, 100, 50, 255,
            ],
            size: [2, 2],
        };
        let half = image.reduced_for(0.5);
        assert_eq!(half.size, [1, 1]);
        assert_eq!(half.pixels, [200, 100, 50, 128]);
        let wide = DecodedImage {
            pixels: vec![255; 1000 * 10 * 4],
            size: [1000, 10],
        };
        let reduced = wide.reduced_for(120.0);
        assert_eq!(reduced.size, [125, 2]);
        assert!(reduced.pixels.iter().all(|&v| v == 255));
    }
}
