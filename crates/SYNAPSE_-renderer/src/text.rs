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
    pub font_index: u8,
}

impl GlyphKey {
    pub fn new(ch: char, font_size: f32, bold: bool, italic: bool) -> Self {
        Self {
            ch,
            font_size_bits: font_size.to_bits(),
            bold,
            italic,
            font_index: 0,
        }
    }

    pub fn new_with_index(
        ch: char,
        font_size: f32,
        bold: bool,
        italic: bool,
        font_index: u8,
    ) -> Self {
        Self {
            ch,
            font_size_bits: font_size.to_bits(),
            bold,
            italic,
            font_index,
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
    pub font_index: u8,
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
#[derive(Clone, Copy)]
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
    /// Which font family (0 = primary, 1+ = fallback).
    pub font_index: u8,
}

/// Fast pre-check: does the text contain any right-to-left codepoint?
/// Covers Hebrew, Arabic (incl. Supplement / Extended-A), and Arabic
/// presentation forms. Used to gate the (slower) BiDi reordering path so
/// pure-LTR text — the overwhelming common case — keeps the fast path.
pub fn contains_rtl(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(c,
            '\u{0590}'..='\u{05FF}'   // Hebrew
            | '\u{0600}'..='\u{06FF}' // Arabic
            | '\u{0700}'..='\u{074F}' // Syriac
            | '\u{0750}'..='\u{077F}' // Arabic Supplement
            | '\u{08A0}'..='\u{08FF}' // Arabic Extended-A
            | '\u{FB1D}'..='\u{FDFF}' // Hebrew / Arabic presentation forms A
            | '\u{FE70}'..='\u{FEFF}' // Arabic presentation forms B
        )
    })
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

pub struct FontFamily {
    regular: fontdue::Font,
    bold: fontdue::Font,
    italic: fontdue::Font,
    rb_regular: rustybuzz::Face<'static>,
    rb_bold: rustybuzz::Face<'static>,
    rb_italic: rustybuzz::Face<'static>,
    font_data: &'static [u8],
}

impl FontFamily {
    fn font(&self, bold: bool, italic: bool) -> &fontdue::Font {
        match (bold, italic) {
            (true, _) => &self.bold,
            (_, true) => &self.italic,
            _ => &self.regular,
        }
    }

    fn rb_face(&self, bold: bool, italic: bool) -> &rustybuzz::Face<'static> {
        match (bold, italic) {
            (true, _) => &self.rb_bold,
            (_, true) => &self.rb_italic,
            _ => &self.rb_regular,
        }
    }

    fn has_glyph(&self, ch: char) -> bool {
        self.regular.lookup_glyph_index(ch) != 0
    }

    pub fn has_color_emoji(&self, ch: char) -> bool {
        let face = ttf_parser::Face::parse(self.font_data, 0);
        match face {
            Ok(face) => {
                let gid = face.glyph_index(ch);
                gid.is_some_and(|gid| face.glyph_raster_image(gid, u16::MAX).is_some())
            }
            Err(_) => false,
        }
    }

    pub fn extract_emoji_bitmap(
        &self,
        ch: char,
        target_size_px: f32,
    ) -> Option<(u32, u32, Vec<u8>)> {
        let face = ttf_parser::Face::parse(self.font_data, 0).ok()?;
        let gid = face.glyph_index(ch)?;
        let image_data = face.glyph_raster_image(gid, u16::MAX)?;
        let img = image::load_from_memory(image_data.data).ok()?;
        let target = target_size_px as u32;
        if img.width() == target && img.height() == target {
            Some((target, target, img.to_rgba8().into_raw()))
        } else {
            let resized = image::imageops::resize(
                &img,
                target,
                target,
                image::imageops::FilterType::Lanczos3,
            );
            Some((target, target, resized.into_raw()))
        }
    }
}

pub struct TextShaping {
    families: Vec<FontFamily>,
}

impl TextShaping {
    pub fn new() -> Self {
        Self {
            families: vec![Self::embedded_family()],
        }
    }

    pub fn with_family(family: &str) -> Self {
        Self::with_families(&[family.to_string()])
    }

    pub fn with_families(families: &[String]) -> Self {
        let mut loaded: Vec<FontFamily> = Vec::with_capacity(families.len());

        for (idx, family) in families.iter().enumerate() {
            let key = normalize_family(family);
            if key == "jetbrainsmono" || key == "jetbrains" {
                loaded.push(Self::embedded_family());
                continue;
            }
            if idx == 0 && (key.is_empty() || key == "monospace") {
                tracing::info!("'{family}' → default JetBrains Mono");
                loaded.push(Self::embedded_family());
                continue;
            }
            match find_font_bytes(&key) {
                Some((reg, bold, italic)) => {
                    tracing::info!("Loaded font family '{family}' from system");
                    loaded.push(Self::own_family(reg, bold, italic));
                }
                None => {
                    if idx == 0 {
                        tracing::warn!(
                            "Font family '{family}' not found — using embedded JetBrains Mono"
                        );
                        loaded.push(Self::embedded_family());
                    } else {
                        tracing::warn!("Fallback font family '{family}' not found — skipping");
                    }
                }
            }
        }

        if loaded.is_empty() {
            loaded.push(Self::embedded_family());
        }

        Self { families: loaded }
    }

    fn embedded_family() -> FontFamily {
        let settings = fontdue::FontSettings::default();
        FontFamily {
            regular: fontdue::Font::from_bytes(JETBRAINS_MONO_REGULAR, settings)
                .expect("font Regular bytes invalid"),
            bold: fontdue::Font::from_bytes(JETBRAINS_MONO_BOLD, settings)
                .expect("font Bold bytes invalid"),
            italic: fontdue::Font::from_bytes(JETBRAINS_MONO_ITALIC, settings)
                .expect("font Italic bytes invalid"),
            rb_regular: rustybuzz::Face::from_slice(JETBRAINS_MONO_REGULAR, 0)
                .expect("rustybuzz: font Regular invalid"),
            rb_bold: rustybuzz::Face::from_slice(JETBRAINS_MONO_BOLD, 0)
                .expect("rustybuzz: font Bold invalid"),
            rb_italic: rustybuzz::Face::from_slice(JETBRAINS_MONO_ITALIC, 0)
                .expect("rustybuzz: font Italic invalid"),
            font_data: JETBRAINS_MONO_REGULAR,
        }
    }

    fn own_family(reg: Vec<u8>, bold: Vec<u8>, italic: Vec<u8>) -> FontFamily {
        let reg: &'static [u8] = Box::leak(reg.into_boxed_slice());
        let bold: &'static [u8] = Box::leak(bold.into_boxed_slice());
        let italic: &'static [u8] = Box::leak(italic.into_boxed_slice());
        let settings = fontdue::FontSettings::default();
        FontFamily {
            regular: fontdue::Font::from_bytes(reg, settings).expect("font Regular bytes invalid"),
            bold: fontdue::Font::from_bytes(bold, settings).expect("font Bold bytes invalid"),
            italic: fontdue::Font::from_bytes(italic, settings).expect("font Italic bytes invalid"),
            rb_regular: rustybuzz::Face::from_slice(reg, 0)
                .expect("rustybuzz: font Regular invalid"),
            rb_bold: rustybuzz::Face::from_slice(bold, 0).expect("rustybuzz: font Bold invalid"),
            rb_italic: rustybuzz::Face::from_slice(italic, 0)
                .expect("rustybuzz: font Italic invalid"),
            font_data: reg,
        }
    }

    fn font(&self, family_idx: usize, bold: bool, italic: bool) -> &fontdue::Font {
        self.families[family_idx].font(bold, italic)
    }

    fn rb_face(&self, family_idx: usize, bold: bool, italic: bool) -> &rustybuzz::Face<'static> {
        self.families[family_idx].rb_face(bold, italic)
    }

    pub fn family_count(&self) -> usize {
        self.families.len()
    }

    pub fn glyph_font_index(&self, ch: char) -> u8 {
        for (idx, family) in self.families.iter().enumerate() {
            if family.has_glyph(ch) {
                return idx as u8;
            }
        }
        0
    }

    /// Returns false when no loaded font has a glyph for `ch`.
    /// Used to skip rendering .notdef boxes for unsupported codepoints (e.g. Nerd Font icons).
    pub fn has_any_glyph(&self, ch: char) -> bool {
        self.families.iter().any(|f| f.has_glyph(ch))
    }

    pub fn rasterize(&self, key: GlyphKey) -> GlyphBitmap {
        let font_size = f32::from_bits(key.font_size_bits);
        let family_idx = key.font_index as usize;
        let font = self.font(family_idx, key.bold, key.italic);
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

    pub fn rasterize_glyph_id(
        &self,
        glyph_id: u16,
        font_size: f32,
        bold: bool,
        italic: bool,
        font_index: u8,
    ) -> GlyphBitmap {
        let family_idx = font_index as usize;
        let font = self.font(family_idx, bold, italic);
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

    pub fn has_color_emoji_in_family(&self, family_idx: usize, ch: char) -> bool {
        self.families
            .get(family_idx)
            .is_some_and(|f| f.has_color_emoji(ch))
    }

    pub fn extract_emoji_bitmap(
        &self,
        family_idx: usize,
        ch: char,
        target_px: u32,
    ) -> Option<(u32, u32, Vec<u8>)> {
        self.families
            .get(family_idx)?
            .extract_emoji_bitmap(ch, target_px as f32)
    }

    fn shape_run_single_dir(
        &self,
        text: &str,
        font_size: f32,
        bold: bool,
        italic: bool,
        family_idx: u8,
        rtl: bool,
    ) -> Vec<ShapedGlyph> {
        if text.is_empty() {
            return Vec::new();
        }
        let face = self.rb_face(family_idx as usize, bold, italic);
        let units_per_em = face.units_per_em() as f32;
        let scale = font_size / units_per_em;

        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        if rtl {
            // Derive script/direction/language from the text so Arabic gets its
            // joining forms and Hebrew/Arabic glyphs come back in visual order.
            buffer.guess_segment_properties();
        } else {
            buffer.set_direction(rustybuzz::Direction::LeftToRight);
        }

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
                font_index: family_idx,
            })
            .collect()
    }

    pub fn shape_run(
        &self,
        text: &str,
        font_size: f32,
        bold: bool,
        italic: bool,
    ) -> Vec<ShapedGlyph> {
        self.shape_run_dir(text, font_size, bold, italic, false)
    }

    /// Shape a run with an explicit direction. `rtl = true` enables Arabic/Hebrew
    /// joining and returns glyphs in visual (right-to-left) order. The font
    /// fallback chain is honoured for both directions.
    pub fn shape_run_dir(
        &self,
        text: &str,
        font_size: f32,
        bold: bool,
        italic: bool,
        rtl: bool,
    ) -> Vec<ShapedGlyph> {
        if text.is_empty() {
            return Vec::new();
        }

        if self.families.len() == 1 {
            return self.shape_run_single_dir(text, font_size, bold, italic, 0, rtl);
        }

        let primary = self.shape_run_single_dir(text, font_size, bold, italic, 0, rtl);
        let num_primary = primary.len();

        let char_count = text.chars().count();
        if char_count == 0 {
            return primary;
        }

        let mut char_indices: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
        char_indices.push(text.len());

        let cluster_to_char = |cluster: u32| -> Option<usize> {
            if cluster as usize >= char_indices.len() {
                return None;
            }
            Some(char_indices[cluster as usize])
        };

        let mut result: Vec<ShapedGlyph> = Vec::with_capacity(num_primary);
        let mut i = 0;

        while i < num_primary {
            let gi = primary[i].glyph_id;
            let cluster = primary[i].cluster;

            if gi != 0
                || self.families[0].has_glyph(
                    text[cluster_to_char(cluster)
                        .unwrap_or(cluster as usize)
                        .min(text.len().saturating_sub(1))..]
                        .chars()
                        .next()
                        .unwrap_or('\0'),
                )
            {
                result.push(primary[i]);
                i += 1;
                continue;
            }

            let start_cluster = cluster;
            let start_byte = cluster_to_char(start_cluster)
                .unwrap_or(start_cluster as usize)
                .min(text.len());
            let mut end_cluster = start_cluster;
            let mut j = i;

            while j < num_primary {
                let gj = primary[j].glyph_id;
                let cj = primary[j].cluster;
                if gj != 0
                    && self.families[0].has_glyph(
                        text[cluster_to_char(cj)
                            .unwrap_or(cj as usize)
                            .min(text.len().saturating_sub(1))..]
                            .chars()
                            .next()
                            .unwrap_or('\0'),
                    )
                {
                    break;
                }
                end_cluster = cj;
                j += 1;
            }

            let end_byte = cluster_to_char(end_cluster)
                .map(|b| {
                    let next_idx = char_indices
                        .iter()
                        .position(|&x| x > b)
                        .unwrap_or(char_indices.len());
                    if next_idx < char_indices.len() {
                        char_indices[next_idx]
                    } else {
                        text.len()
                    }
                })
                .unwrap_or(text.len());

            let sub = &text[start_byte..end_byte.min(text.len())];

            let mut found = false;
            for fb_idx in 1..self.families.len() {
                let first_ch = sub.chars().next().unwrap_or('\0');
                if !self.families[fb_idx].has_glyph(first_ch) {
                    continue;
                }
                let fb_glyphs =
                    self.shape_run_single_dir(sub, font_size, bold, italic, fb_idx as u8, rtl);
                for g in &fb_glyphs {
                    result.push(ShapedGlyph {
                        glyph_id: g.glyph_id,
                        x_advance: g.x_advance,
                        x_offset: g.x_offset,
                        y_offset: g.y_offset,
                        cluster: start_cluster + g.cluster,
                        font_index: fb_idx as u8,
                    });
                }
                found = true;
                break;
            }

            if !found {
                for item in primary.iter().take(j).skip(i) {
                    result.push(*item);
                }
            }

            i = j;
        }

        result
    }

    pub fn cell_metrics(&self, font_size: f32) -> (f32, f32) {
        let metrics = self.families[0].regular.metrics('M', font_size);
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
    fn contains_rtl_detects_hebrew_and_arabic() {
        assert!(contains_rtl("\u{05E9}\u{05DC}\u{05D5}\u{05DD}")); // שלום (Hebrew)
        assert!(contains_rtl("\u{0645}\u{0631}\u{062D}\u{0628}\u{0627}")); // مرحبا (Arabic)
        assert!(contains_rtl("hi \u{05E9}")); // mixed
        assert!(!contains_rtl("hello world"));
        assert!(!contains_rtl("ls -la | grep foo"));
        assert!(!contains_rtl(""));
    }

    #[test]
    fn shape_run_dir_rtl_does_not_panic() {
        // Even without an Arabic-capable fallback font, RTL shaping must not
        // panic and must return one entry per input cluster (likely .notdef).
        let shaping = TextShaping::new();
        let glyphs =
            shaping.shape_run_dir("\u{05E9}\u{05DC}\u{05D5}\u{05DD}", 14.0, false, false, true);
        assert!(!glyphs.is_empty());
    }

    #[test]
    fn bidi_reorders_rtl_visually() {
        // Sanity-check the BiDi engine itself: a pure-RTL string yields a single
        // RTL level run covering the whole text.
        let text = "\u{05E9}\u{05DC}\u{05D5}\u{05DD}";
        let bidi = unicode_bidi::BidiInfo::new(text, None);
        let para = &bidi.paragraphs[0];
        let (_levels, runs) = bidi.visual_runs(para, para.range.clone());
        assert_eq!(runs.len(), 1);
        assert!(bidi.levels[runs[0].start].is_rtl());
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
    fn rasterize_glyph_id_matches_char() {
        let shaping = TextShaping::new();
        let shaped = shaping.shape_run("A", 14.0, false, false);
        assert!(!shaped.is_empty());
        let glyph_id = shaped[0].glyph_id;
        let by_id = shaping.rasterize_glyph_id(glyph_id, 14.0, false, false, 0);
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

    #[test]
    fn glyph_key_font_index_defaults_to_zero() {
        let key = GlyphKey::new('A', 14.0, false, false);
        assert_eq!(key.font_index, 0);
    }

    #[test]
    fn glyph_key_with_index() {
        let key = GlyphKey::new_with_index('A', 14.0, false, false, 2);
        assert_eq!(key.font_index, 2);
        assert_eq!(key.ch, 'A');
    }

    #[test]
    fn shaped_glyph_has_font_index_zero_for_primary() {
        let shaping = TextShaping::new();
        let glyphs = shaping.shape_run("Hello", 14.0, false, false);
        for g in &glyphs {
            assert_eq!(
                g.font_index, 0,
                "all glyphs should use primary font (index 0)"
            );
        }
    }

    #[test]
    fn glyph_font_index_ascii_is_zero() {
        let shaping = TextShaping::new();
        assert_eq!(shaping.glyph_font_index('A'), 0);
        assert_eq!(shaping.glyph_font_index('z'), 0);
        assert_eq!(shaping.glyph_font_index('9'), 0);
        assert_eq!(shaping.glyph_font_index(' '), 0);
    }

    #[test]
    fn family_count_is_one_for_default() {
        let shaping = TextShaping::new();
        assert_eq!(shaping.family_count(), 1);
    }

    #[test]
    fn shape_run_primary_font_index() {
        let shaping = TextShaping::new();
        let glyphs = shaping.shape_run("Abc", 14.0, false, false);
        for g in &glyphs {
            assert_eq!(g.font_index, 0);
        }
    }
}
