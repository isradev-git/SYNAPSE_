const JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");
const JETBRAINS_MONO_BOLD: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono-Bold.ttf");
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

fn normalize_family(family: &str) -> String {
    family
        .chars()
        .filter(|c| c.is_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

#[derive(Debug)]
enum FontVariant {
    Regular,
    Bold,
    Italic,
}

fn classify_variant(stem_norm: &str, family_key: &str) -> Option<FontVariant> {
    let remainder = stem_norm.replacen(family_key, "", 1);
    for skip in &[
        "extralight",
        "extrabold",
        "semibold",
        "medium",
        "black",
        "heavy",
        "thin",
        "light",
        "condensed",
        "expanded",
    ] {
        if remainder.contains(skip) {
            return None;
        }
    }
    let is_bold = remainder.contains("bold");
    let is_italic = remainder.contains("italic") || remainder.contains("oblique");
    match (is_bold, is_italic) {
        (false, false) => Some(FontVariant::Regular),
        (true, false) => Some(FontVariant::Bold),
        (false, true) => Some(FontVariant::Italic),
        (true, true) => None,
    }
}

fn collect_font_paths(
    dir: &std::path::Path,
    family_key: &str,
    regular: &mut Option<std::path::PathBuf>,
    bold: &mut Option<std::path::PathBuf>,
    italic: &mut Option<std::path::PathBuf>,
    depth: u8,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && depth > 0 {
            collect_font_paths(&path, family_key, regular, bold, italic, depth - 1);
        } else {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !matches!(ext.to_ascii_lowercase().as_str(), "ttf" | "otf") {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let stem_norm = normalize_family(stem);
            if !stem_norm.contains(family_key) {
                continue;
            }
            match classify_variant(&stem_norm, family_key) {
                Some(FontVariant::Regular) if regular.is_none() => *regular = Some(path),
                Some(FontVariant::Bold) if bold.is_none() => *bold = Some(path),
                Some(FontVariant::Italic) if italic.is_none() => *italic = Some(path),
                _ => {}
            }
        }
    }
}

fn system_font_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            dirs.push(std::path::PathBuf::from(home).join("Library/Fonts"));
        }
        dirs.push(std::path::PathBuf::from("/Library/Fonts"));
        dirs.push(std::path::PathBuf::from("/System/Library/Fonts"));
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            let home = std::path::PathBuf::from(home);
            dirs.push(home.join(".local/share/fonts"));
            dirs.push(home.join(".fonts"));
        }
        dirs.push(std::path::PathBuf::from("/usr/share/fonts"));
        dirs.push(std::path::PathBuf::from("/usr/local/share/fonts"));
    }
    dirs
}

fn find_font_bytes(family_key: &str) -> Option<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let mut regular_path: Option<std::path::PathBuf> = None;
    let mut bold_path: Option<std::path::PathBuf> = None;
    let mut italic_path: Option<std::path::PathBuf> = None;

    for dir in system_font_dirs() {
        collect_font_paths(
            &dir,
            family_key,
            &mut regular_path,
            &mut bold_path,
            &mut italic_path,
            4,
        );
        if regular_path.is_some() {
            break;
        }
    }

    let reg_bytes = std::fs::read(regular_path?).ok()?;
    let bold_bytes = bold_path
        .and_then(|p| std::fs::read(p).ok())
        .unwrap_or_else(|| reg_bytes.clone());
    let italic_bytes = italic_path
        .and_then(|p| std::fs::read(p).ok())
        .unwrap_or_else(|| reg_bytes.clone());

    Some((reg_bytes, bold_bytes, italic_bytes))
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
        Self::from_static(
            JETBRAINS_MONO_REGULAR,
            JETBRAINS_MONO_BOLD,
            JETBRAINS_MONO_ITALIC,
        )
    }

    /// Load the requested font family from system fonts, falling back to embedded
    /// JetBrains Mono if the family is empty, "monospace", or not found.
    pub fn with_family(family: &str) -> Self {
        let key = normalize_family(family);
        if key.is_empty() || key == "monospace" || key == "jetbrainsmono" {
            return Self::new();
        }
        match find_font_bytes(&key) {
            Some((reg, bold, italic)) => {
                tracing::info!("Loaded font family '{family}' from system");
                Self::from_owned(reg, bold, italic)
            }
            None => {
                tracing::warn!("Font family '{family}' not found — using embedded JetBrains Mono");
                Self::new()
            }
        }
    }

    fn from_static(reg: &'static [u8], bold: &'static [u8], italic: &'static [u8]) -> Self {
        let settings = fontdue::FontSettings::default();
        let font_regular =
            fontdue::Font::from_bytes(reg, settings).expect("font Regular bytes invalid");
        let font_bold = fontdue::Font::from_bytes(bold, settings).expect("font Bold bytes invalid");
        let font_italic =
            fontdue::Font::from_bytes(italic, settings).expect("font Italic bytes invalid");
        let rb_regular =
            rustybuzz::Face::from_slice(reg, 0).expect("rustybuzz: font Regular invalid");
        let rb_bold = rustybuzz::Face::from_slice(bold, 0).expect("rustybuzz: font Bold invalid");
        let rb_italic =
            rustybuzz::Face::from_slice(italic, 0).expect("rustybuzz: font Italic invalid");
        Self {
            font_regular,
            font_bold,
            font_italic,
            rb_regular,
            rb_bold,
            rb_italic,
        }
    }

    fn from_owned(reg: Vec<u8>, bold: Vec<u8>, italic: Vec<u8>) -> Self {
        // Leak so rustybuzz::Face can hold a 'static reference.
        // Each font file is ~200–400 KB; leaking once at startup is acceptable.
        let reg: &'static [u8] = Box::leak(reg.into_boxed_slice());
        let bold: &'static [u8] = Box::leak(bold.into_boxed_slice());
        let italic: &'static [u8] = Box::leak(italic.into_boxed_slice());
        Self::from_static(reg, bold, italic)
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
        buffer.set_direction(rustybuzz::Direction::LeftToRight);
        buffer.push_str(text);

        // Explicitly enable contextual alternates (calt) and standard ligatures (liga).
        // rustybuzz needs these named for JetBrains Mono arrow sequences like -> and =>.
        let features: Vec<rustybuzz::Feature> = ["calt", "liga", "clig"]
            .iter()
            .filter_map(|s| s.parse().ok())
            .collect();
        let output = rustybuzz::shape(face, &features, buffer);

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
        let combined = shaping.shape_run("->", 14.0, false, false);
        let dash = shaping.shape_run("-", 14.0, false, false);
        let gt = shaping.shape_run(">", 14.0, false, false);
        println!(
            "'->' shaped to {} glyph(s): {:?}",
            combined.len(),
            combined.iter().map(|g| g.glyph_id).collect::<Vec<_>>()
        );
        assert!(!combined.is_empty(), "shaped run must produce glyphs");
        // JetBrains Mono uses calt (same count, alternate IDs) or liga (count drops).
        let liga_fired = combined.len() == 1;
        let calt_fired = combined.len() == 2
            && dash
                .first()
                .zip(combined.first())
                .map(|(d, c)| d.glyph_id != c.glyph_id)
                .unwrap_or(false)
            || combined.len() == 2
                && gt
                    .first()
                    .zip(combined.get(1))
                    .map(|(g, c)| g.glyph_id != c.glyph_id)
                    .unwrap_or(false);
        assert!(
            liga_fired || calt_fired,
            "'->' ligature should fire via liga or calt"
        );
        let total_adv: f32 = combined.iter().map(|g| g.x_advance).sum();
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
        assert_eq!(
            by_id.width, by_char.width,
            "glyph_id and char rasterize to same width"
        );
        assert_eq!(
            by_id.height, by_char.height,
            "glyph_id and char rasterize to same height"
        );
    }
}
