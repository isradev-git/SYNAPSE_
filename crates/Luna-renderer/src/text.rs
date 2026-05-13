const JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");
const JETBRAINS_MONO_BOLD: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Bold.ttf");
const JETBRAINS_MONO_ITALIC: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Italic.ttf");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub ch: char,
    pub font_size_bits: u32,
    pub bold: bool,
    pub italic: bool,
}

impl GlyphKey {
    pub fn new(ch: char, font_size: f32, bold: bool, italic: bool) -> Self {
        Self {
            ch,
            font_size_bits: font_size.to_bits(),
            bold,
            italic,
        }
    }
}

pub struct GlyphBitmap {
    pub width: u32,
    pub height: u32,
    pub top: i32,
    pub left: i32,
    pub advance_width: f32,
    pub data: Vec<u8>,
}

pub struct TextShaping {
    font_regular: fontdue::Font,
    font_bold: fontdue::Font,
    font_italic: fontdue::Font,
}

impl TextShaping {
    pub fn new() -> Self {
        let settings = fontdue::FontSettings::default();
        let font_regular = fontdue::Font::from_bytes(JETBRAINS_MONO_REGULAR, settings)
            .expect("embedded JetBrains Mono Regular is invalid");
        let font_bold = fontdue::Font::from_bytes(JETBRAINS_MONO_BOLD, settings)
            .expect("embedded JetBrains Mono Bold is invalid");
        let font_italic = fontdue::Font::from_bytes(JETBRAINS_MONO_ITALIC, settings)
            .expect("embedded JetBrains Mono Italic is invalid");
        Self { font_regular, font_bold, font_italic }
    }

    fn font(&self, bold: bool, italic: bool) -> &fontdue::Font {
        match (bold, italic) {
            (true, _) => &self.font_bold,
            (_, true) => &self.font_italic,
            _ => &self.font_regular,
        }
    }

    pub fn rasterize(&self, key: GlyphKey) -> GlyphBitmap {
        let font_size = f32::from_bits(key.font_size_bits);
        let font = self.font(key.bold, key.italic);
        let (metrics, data) = font.rasterize(key.ch, font_size);
        GlyphBitmap {
            width: metrics.width as u32,
            height: metrics.height as u32,
            top: metrics.ymin,
            left: metrics.xmin,
            advance_width: metrics.advance_width,
            data,
        }
    }

    pub fn cell_metrics(&self, font_size: f32) -> (f32, f32) {
        let metrics = self.font_regular.metrics('M', font_size);
        let cell_w = metrics.advance_width;
        let cell_h = font_size * 1.2;
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
    fn rasterize_ascii_produces_bitmap() {
        let shaping = TextShaping::new();
        let key = GlyphKey::new('A', 14.0, false, false);
        let glyph = shaping.rasterize(key);
        assert!(glyph.width > 0, "width must be > 0 for 'A'");
        assert!(glyph.height > 0, "height must be > 0 for 'A'");
        assert_eq!(
            glyph.data.len(),
            (glyph.width * glyph.height) as usize,
            "bitmap length must equal width*height (grayscale)"
        );
    }

    #[test]
    fn rasterize_space_is_empty() {
        let shaping = TextShaping::new();
        let key = GlyphKey::new(' ', 14.0, false, false);
        let glyph = shaping.rasterize(key);
        assert_eq!(glyph.data.len(), 0, "space produces empty bitmap");
    }

    #[test]
    fn cell_metrics_are_positive() {
        let shaping = TextShaping::new();
        let (w, h) = shaping.cell_metrics(14.0);
        assert!(w > 0.0, "cell width must be > 0");
        assert!(h > 0.0, "cell height must be > 0");
    }

    #[test]
    fn bold_differs_from_regular() {
        let shaping = TextShaping::new();
        let regular = shaping.rasterize(GlyphKey::new('B', 14.0, false, false));
        let bold = shaping.rasterize(GlyphKey::new('B', 14.0, true, false));
        assert!(
            regular.data != bold.data || regular.width != bold.width,
            "bold and regular bitmaps should differ"
        );
    }
}
