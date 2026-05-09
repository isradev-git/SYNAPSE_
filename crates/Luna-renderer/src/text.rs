use cosmic_text::{Attrs, CacheKey, CacheKeyFlags, Family, FontSystem, SwashCache, SwashImage};

const JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");
const JETBRAINS_MONO_BOLD: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Bold.ttf");
const JETBRAINS_MONO_ITALIC: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Italic.ttf");

pub struct TextShaping {
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
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

    pub fn rasterize_glyph(
        &mut self,
        c: char,
        font_size: f32,
    ) -> Option<(SwashImage, CacheKey)> {
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
                    (glyph.x, glyph.y),
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

    pub fn cell_metrics(&mut self, font_size: f32) -> (f32, f32) {
        if let Some((image, _)) = self.rasterize_glyph('W', font_size) {
            let w = image.placement.width as f32;
            let h = image.placement.height as f32;
            if w > 0.0 && h > 0.0 {
                return (w + 1.0, h + 4.0);
            }
        }
        (font_size * 0.6, font_size * 1.2)
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
}
