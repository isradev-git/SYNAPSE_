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

/// Cache key for a shaped glyph (looked up by glyph ID, not Unicode codepoint).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShapedGlyphKey {
    pub glyph_id: u16,
    pub font_size_bits: u32,
    pub bold: bool,
    pub italic: bool,
}

pub struct GlyphBitmap {
    pub width: u32,
    pub height: u32,
    pub top: i32,
    pub left: i32,
    pub advance_width: f32,
    pub data: Vec<u8>,
}

/// One glyph returned from a rustybuzz shaping run.
pub struct ShapedGlyph {
    pub glyph_id: u16,
    /// Horizontal advance in pixels at the requested font size.
    pub x_advance: f32,
    /// Horizontal offset (kerning/GPOS) in pixels.
    pub x_offset: f32,
    /// Vertical offset in pixels.
    pub y_offset: f32,
    /// Original cluster index (byte offset in input string).
    pub cluster: u32,
}

pub struct TextShaping {
    font_regular: fontdue::Font,
    font_bold: fontdue::Font,
    font_italic: fontdue::Font,
    rb_regular: rustybuzz::Face<'static>,
    rb_bold: rustybuzz::Face<'static>,
    rb_italic: rustybuzz::Face<'static>,
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

        let rb_regular = rustybuzz::Face::from_slice(JETBRAINS_MONO_REGULAR, 0)
            .expect("rustybuzz: JetBrains Mono Regular is invalid");
        let rb_bold = rustybuzz::Face::from_slice(JETBRAINS_MONO_BOLD, 0)
            .expect("rustybuzz: JetBrains Mono Bold is invalid");
        let rb_italic = rustybuzz::Face::from_slice(JETBRAINS_MONO_ITALIC, 0)
            .expect("rustybuzz: JetBrains Mono Italic is invalid");

        Self { font_regular, font_bold, font_italic, rb_regular, rb_bold, rb_italic }
    }

    fn font(&self, bold: bool, italic: bool) -> &fontdue::Font {
        match (bold, italic) {
            (true, _) => &self.font_bold,
            (_, true) => &self.font_italic,
            _ => &self.font_regular,
        }
    }

    fn rb_face(&self, bold: bool, italic: bool) -> &rustybuzz::Face<'static> {
        match (bold, italic) {
            (true, _) => &self.rb_bold,
            (_, true) => &self.rb_italic,
            _ => &self.rb_regular,
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

    /// Rasterize by glyph ID (for ligature glyphs from rustybuzz shaping).
    pub fn rasterize_glyph_id(
        &self,
        glyph_id: u16,
        font_size: f32,
        bold: bool,
        italic: bool,
    ) -> GlyphBitmap {
        let font = self.font(bold, italic);
        let (metrics, data) = font.rasterize_indexed(glyph_id, font_size);
        GlyphBitmap {
            width: metrics.width as u32,
            height: metrics.height as u32,
            top: metrics.ymin,
            left: metrics.xmin,
            advance_width: metrics.advance_width,
            data,
        }
    }

    /// Shape a run of text using HarfBuzz (via rustybuzz). Returns shaped glyphs
    /// with glyph IDs and pixel-accurate advances at the given font size.
    pub fn shape_run(
        &self,
        text: &str,
        font_size: f32,
        bold: bool,
        italic: bool,
    ) -> Vec<ShapedGlyph> {
        if text.is_empty() {
            return Vec::new();
        }
        let face = self.rb_face(bold, italic);
        let units_per_em = face.units_per_em() as f32;
        let scale = font_size / units_per_em;

        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        let output = rustybuzz::shape(face, &[], buffer);

        let info = output.glyph_infos();
        let positions = output.glyph_positions();

        info.iter()
            .zip(positions.iter())
            .map(|(gi, gp)| ShapedGlyph {
                glyph_id: gi.glyph_id as u16,
                x_advance: gp.x_advance as f32 * scale,
                x_offset: gp.x_offset as f32 * scale,
                y_offset: gp.y_offset as f32 * scale,
                cluster: gi.cluster,
            })
            .collect()
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

    #[test]
    fn shape_run_arrow_ligature() {
        let shaping = TextShaping::new();
        // JetBrains Mono has `->` and `=>` ligatures.
        let glyphs = shaping.shape_run("->", 14.0, false, false);
        // Shaped output should have at least 1 glyph (may be 1 ligature or 2 separate).
        assert!(!glyphs.is_empty(), "shaped run must produce glyphs");
        // Total x_advance should be approximately 2 * cell_w.
        let total_adv: f32 = glyphs.iter().map(|g| g.x_advance).sum();
        let (cell_w, _) = shaping.cell_metrics(14.0);
        assert!(
            (total_adv - cell_w * 2.0).abs() < cell_w * 0.5,
            "total advance for '->' should be ~2 cells, got {total_adv} vs {cell_w}*2"
        );
    }

    #[test]
    fn shape_run_single_char() {
        let shaping = TextShaping::new();
        let glyphs = shaping.shape_run("A", 14.0, false, false);
        assert_eq!(glyphs.len(), 1);
        assert!(glyphs[0].glyph_id > 0, "glyph_id for 'A' must be nonzero");
    }

    #[test]
    fn rasterize_glyph_id_matches_char() {
        let shaping = TextShaping::new();
        // Shape 'A' to get its glyph_id, then rasterize by ID.
        let shaped = shaping.shape_run("A", 14.0, false, false);
        assert!(!shaped.is_empty());
        let glyph_id = shaped[0].glyph_id;
        let by_id = shaping.rasterize_glyph_id(glyph_id, 14.0, false, false);
        let by_char = shaping.rasterize(GlyphKey::new('A', 14.0, false, false));
        assert_eq!(by_id.width, by_char.width, "glyph_id and char rasterize to same width");
        assert_eq!(by_id.height, by_char.height, "glyph_id and char rasterize to same height");
    }
}
