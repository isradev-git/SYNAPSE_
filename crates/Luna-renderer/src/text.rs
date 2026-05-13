use cosmic_text::{Attrs, CacheKey, CacheKeyFlags, Family, FontSystem, SwashCache, SwashImage};

const JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");
const JETBRAINS_MONO_BOLD: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono-Bold.ttf");
const JETBRAINS_MONO_ITALIC: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Italic.ttf");

pub struct TextShaping {
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
}

/// One rasterized glyph from a shaped text run, with its byte range in the source string.
/// A single `ShapedGlyph` covering more than one source char indicates a ligature.
pub struct ShapedGlyph {
    pub image: SwashImage,
    pub cache_key: CacheKey,
    /// Byte offset of the first source char this glyph covers.
    pub src_start: usize,
    /// Byte offset one past the last source char this glyph covers.
    pub src_end: usize,
}

impl TextShaping {
    pub fn new() -> Self {
        let mut font_system = FontSystem::new();

        font_system
            .db_mut()
            .load_font_data(JETBRAINS_MONO_REGULAR.to_vec());

        font_system
            .db_mut()
            .load_font_data(JETBRAINS_MONO_BOLD.to_vec());

        font_system
            .db_mut()
            .load_font_data(JETBRAINS_MONO_ITALIC.to_vec());

        let swash_cache = SwashCache::new();

        Self {
            font_system,
            swash_cache,
        }
    }

    pub fn rasterize_glyph(&mut self, c: char, font_size: f32) -> Option<(SwashImage, CacheKey)> {
        let attrs = Attrs::new().family(Family::Name("JetBrains Mono"));

        let mut buffer = cosmic_text::Buffer::new(
            &mut self.font_system,
            cosmic_text::Metrics::new(font_size, font_size),
        );
        buffer.set_size(&mut self.font_system, Some(font_size), None);
        buffer.set_text(
            &mut self.font_system,
            &c.to_string(),
            attrs,
            cosmic_text::Shaping::Advanced,
        );

        buffer.shape_until_scroll(&mut self.font_system, false);

        for run in buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let (cache_key, _x, _y) = CacheKey::new(
                    glyph.font_id,
                    glyph.glyph_id,
                    glyph.font_size,
                    (0.0, 0.0),
                    CacheKeyFlags::empty(),
                );
                if let Some(image) = self
                    .swash_cache
                    .get_image_uncached(&mut self.font_system, cache_key)
                {
                    return Some((image, cache_key));
                }
            }
        }

        None
    }

    /// Shape an entire text run with ligature support. Returns one `ShapedGlyph` per
    /// output glyph. A glyph whose `src_start..src_end` spans multiple chars is a ligature.
    pub fn shape_run(&mut self, text: &str, font_size: f32) -> Vec<ShapedGlyph> {
        if text.is_empty() {
            return Vec::new();
        }

        let attrs = Attrs::new().family(Family::Name("JetBrains Mono"));
        let mut buffer = cosmic_text::Buffer::new(
            &mut self.font_system,
            cosmic_text::Metrics::new(font_size, font_size),
        );
        buffer.set_size(&mut self.font_system, None, None);
        buffer.set_text(
            &mut self.font_system,
            text,
            attrs,
            cosmic_text::Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut result = Vec::new();
        for run in buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let (cache_key, _x, _y) = CacheKey::new(
                    glyph.font_id,
                    glyph.glyph_id,
                    glyph.font_size,
                    (0.0, 0.0),
                    CacheKeyFlags::empty(),
                );
                if let Some(image) = self
                    .swash_cache
                    .get_image_uncached(&mut self.font_system, cache_key)
                {
                    result.push(ShapedGlyph {
                        image,
                        cache_key,
                        src_start: glyph.start,
                        src_end: glyph.end,
                    });
                }
            }
        }
        result
    }

    pub fn cell_metrics(&mut self, font_size: f32) -> (f32, f32) {
        let attrs = Attrs::new().family(Family::Name("JetBrains Mono"));
        let metrics = cosmic_text::Metrics::new(font_size, font_size * 1.2);
        let mut buffer = cosmic_text::Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(
            &mut self.font_system,
            Some(font_size * 100.0),
            Some(font_size * 2.0),
        );
        buffer.set_text(&mut self.font_system, "W", attrs, cosmic_text::Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut cell_w = font_size * 0.6;
        let mut cell_h = font_size * 1.2;

        for run in buffer.layout_runs() {
            if let Some(glyph) = run.glyphs.first() {
                cell_w = glyph.w;
            }
            cell_h = run.line_height;
            break;
        }

        (cell_w, cell_h)
    }
}

impl Default for TextShaping {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rasterize_a() {
        let mut shaping = TextShaping::new();
        let result = shaping.rasterize_glyph('A', 14.0);
        assert!(result.is_some(), "Should rasterize 'A' without panic");
    }

    #[test]
    fn test_shape_run_single_char() {
        let mut shaping = TextShaping::new();
        let glyphs = shaping.shape_run("A", 14.0);
        assert!(!glyphs.is_empty());
        assert_eq!(glyphs[0].src_start, 0);
        assert_eq!(glyphs[0].src_end, 1);
    }

    #[test]
    fn test_shape_run_two_chars() {
        let mut shaping = TextShaping::new();
        // Two chars — may or may not produce a ligature depending on the font,
        // but must return at least one glyph without panicking.
        let glyphs = shaping.shape_run("->", 14.0);
        assert!(!glyphs.is_empty());
    }
}

#[cfg(test)]
mod debug_tests {
    use super::*;
    use cosmic_text::{Attrs, Family, Metrics, Shaping, Buffer};

    #[test]
    fn debug_cell_metrics() {
        let mut shaping = TextShaping::new();
        let font_size = 14.0f32;
        let attrs = Attrs::new().family(Family::Name("JetBrains Mono"));
        let metrics = Metrics::new(font_size, font_size * 1.2);
        let mut buffer = Buffer::new(&mut shaping.font_system, metrics);
        buffer.set_size(&mut shaping.font_system, Some(font_size * 100.0), Some(font_size * 2.0));
        buffer.set_text(&mut shaping.font_system, "W", attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut shaping.font_system, false);
        let runs: Vec<_> = buffer.layout_runs().collect();
        println!("layout_runs count: {}", runs.len());
        for run in &runs {
            println!("  line_height={}, glyphs={}", run.line_height, run.glyphs.len());
            for g in run.glyphs.iter() {
                println!("    glyph w={}, x={}, font_size={}", g.w, g.x, g.font_size);
            }
        }
        let (w, h) = shaping.cell_metrics(14.0);
        println!("cell_metrics(14.0) -> w={}, h={}", w, h);
    }
}

#[cfg(test)]
mod pixel_tests {
    use super::*;

    #[test]
    fn dump_glyph_pixels() {
        let mut shaping = TextShaping::new();
        if let Some((img, _)) = shaping.rasterize_glyph('A', 14.0) {
            println!("content type: {:?}", img.content);
            println!("size: {}x{}", img.placement.width, img.placement.height);
            println!("placement: left={}, top={}", img.placement.left, img.placement.top);
            println!("data len: {}", img.data.len());
            // Imprime primeras 5 filas como mapa de caracteres
            let w = img.placement.width as usize;
            let h = img.placement.height as usize;
            for row in 0..h.min(10) {
                let row_str: String = (0..w).map(|col| {
                    let alpha = img.data[row * w + col];
                    if alpha > 200 { '#' } else if alpha > 100 { '+' } else if alpha > 0 { '.' } else { ' ' }
                }).collect();
                println!("|{}|", row_str);
            }
        }
    }
}
