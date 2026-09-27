//! Build-time glyph provisioning. This spike cannot display unbaked text.
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[cfg(target_arch = "wasm32")]
use std::io::Cursor;

#[derive(Clone)]
pub(super) struct TextSprite {
    pub width: u32,
    pub height: u32,
    pub advance: f32,
    pub pixels: Vec<u8>,
}

pub(super) struct FontSystem {
    sprites: BTreeMap<String, TextSprite>,
    #[cfg(not(target_arch = "wasm32"))]
    inner: cosmic_text::FontSystem,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    key: String,
    width: u32,
    height: u32,
    advance: f32,
    y: u32,
}

impl FontSystem {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn native() -> Result<Self> {
        let mut inner = cosmic_text::FontSystem::new();
        let font = std::env::var("KINOGRAPH_FONT").unwrap_or_else(|_| {
            format!(
                "{}/Library/Fonts/CommitMono-400-Regular.otf",
                std::env::var("HOME").unwrap()
            )
        });
        inner
            .db_mut()
            .load_font_file(font)
            .context("load native CommitMono for the local comparison")?;
        Ok(Self {
            inner,
            sprites: BTreeMap::new(),
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        let width = self
            .sprites
            .values()
            .map(|s| s.width)
            .max()
            .context("no baked glyphs")?;
        let height = self.sprites.values().map(|s| s.height).sum();
        let mut pixels = vec![0; (width * height * 4) as usize];
        let mut entries = Vec::new();
        let mut y = 0;
        for (key, sprite) in &self.sprites {
            entries.push(Entry {
                key: key.clone(),
                width: sprite.width,
                height: sprite.height,
                advance: sprite.advance,
                y,
            });
            for row in 0..sprite.height {
                let start = ((y + row) * width * 4) as usize;
                let from = (row * sprite.width * 4) as usize;
                pixels[start..start + sprite.width as usize * 4]
                    .copy_from_slice(&sprite.pixels[from..from + sprite.width as usize * 4]);
            }
            y += sprite.height;
        }
        let metadata = serde_json::to_vec(&entries)?;
        let mut bytes = (metadata.len() as u32).to_le_bytes().to_vec();
        bytes.extend(metadata);
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&pixels)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn baked(bytes: &[u8]) -> Result<Self> {
        let length =
            u32::from_le_bytes(bytes.get(..4).context("glyph header")?.try_into()?) as usize;
        let entries: Vec<Entry> =
            serde_json::from_slice(bytes.get(4..4 + length).context("glyph metadata")?)?;
        let mut decoder =
            png::Decoder::new(Cursor::new(bytes.get(4 + length..).context("glyph PNG")?))
                .read_info()?;
        let mut pixels = vec![0; decoder.output_buffer_size().context("glyph size")?];
        let info = decoder.next_frame(&mut pixels)?;
        let mut sprites = BTreeMap::new();
        for entry in entries {
            let mut ink = Vec::new();
            for y in entry.y..entry.y + entry.height {
                let from = (y * info.width * 4) as usize;
                ink.extend_from_slice(
                    pixels
                        .get(from..from + entry.width as usize * 4)
                        .context("glyph bounds")?,
                );
            }
            sprites.insert(
                entry.key,
                TextSprite {
                    width: entry.width,
                    height: entry.height,
                    advance: entry.advance,
                    pixels: ink,
                },
            );
        }
        Ok(Self { sprites })
    }
}

pub(super) fn make_sprite<'a>(
    fonts: &mut FontSystem,
    cache: &mut SwashCache,
    spans: Vec<(&'a str, Attrs<'a>)>,
    base: Attrs<'a>,
    metrics: Metrics,
    width: u32,
    height: u32,
) -> TextSprite {
    let text = spans.iter().map(|(text, _)| *text).collect::<String>();
    let key = format!(
        "{text}/{}/{}/{width}/{height}",
        metrics.font_size.to_bits(),
        metrics.line_height.to_bits()
    );
    if let Some(sprite) = fonts.sprites.get(&key) {
        return sprite.clone();
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (cache, base);
        panic!("unbaked glyph request: {key}");
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let raster=super::text_raster::make_sprite(&mut fonts.inner,cache,spans,base,metrics,width,height);
        let sprite = TextSprite {
            width:raster.width,
            height:raster.height,
            advance:raster.advance,
            pixels:raster.pixels,
        };
        fonts.sprites.insert(key, sprite.clone());
        sprite
    }
}
