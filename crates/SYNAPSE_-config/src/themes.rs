use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Theme {
    pub bg: [f32; 4],
    pub fg: [f32; 4],
    pub cursor: [f32; 4],
    pub selection: [f32; 4],
    // Tab bar
    pub tab_bar_bg: [f32; 4],
    pub tab_active_bg: [f32; 4],
    pub tab_inactive_bg: [f32; 4],
    pub tab_hover_bg: [f32; 4],
    pub tab_text: [f32; 4],
    pub tab_text_inactive: [f32; 4],
    pub tab_button_text: [f32; 4],
    pub tab_separator: [f32; 4],
    // Panels
    pub panel_active_border: [f32; 4],
    pub panel_inactive_border: [f32; 4],
    pub panel_divider: [f32; 4],
    // Search UI
    pub search_bar_bg: [f32; 4],
    pub search_highlight: [f32; 4],
    pub search_current: [f32; 4],
    pub search_text: [f32; 4],
    pub search_text_dim: [f32; 4],
    // Ghost text (autosuggestion overlay)
    pub ghost_text: [f32; 4],
    // Hyperlink / URL underline color
    pub hyperlink: [f32; 4],
    // Scrollbar
    pub scrollbar_track: [f32; 4],
    pub scrollbar_thumb: [f32; 4],
    // ANSI 16-color palette (indices 0-15)
    // 0=Black, 1=Red, 2=Green, 3=Yellow, 4=Blue, 5=Magenta, 6=Cyan, 7=White
    // 8=BrightBlack, 9=BrightRed, 10=BrightGreen, 11=BrightYellow,
    // 12=BrightBlue, 13=BrightMagenta, 14=BrightCyan, 15=BrightWhite
    pub ansi_colors: [[f32; 4]; 16],
}

fn hex(s: &str) -> [f32; 4] {
    let s = s.trim_start_matches('#');
    let parse = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).unwrap_or(0) as f32 / 255.0;
    if s.len() >= 6 {
        let a = if s.len() >= 8 { parse(6) } else { 1.0 };
        [parse(0), parse(2), parse(4), a]
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::synapse_()
    }
}

impl Theme {
    pub fn synapse_() -> Self {
        Self {
            bg: hex("0A0C14"),
            fg: hex("B0BBC8"),
            cursor: hex("FF003C"),
            selection: hex("FF003C28"),
            tab_bar_bg: hex("070910"),
            tab_active_bg: hex("13182A"),
            tab_inactive_bg: hex("070910"),
            tab_hover_bg: hex("FF003C18"),
            tab_text: hex("D0D8E4"),
            tab_text_inactive: hex("3C4560"),
            tab_button_text: hex("FF003C"),
            tab_separator: hex("FF003C1A"),
            panel_active_border: hex("FF003C"),
            panel_inactive_border: hex("181E30"),
            panel_divider: hex("FF003C22"),
            search_bar_bg: hex("0A0D1AEE"),
            search_highlight: hex("FF003C30"),
            search_current: hex("FF003C70"),
            search_text: hex("D0D8E4"),
            search_text_dim: hex("3C4560"),
            ghost_text: hex("263048"),
            hyperlink: hex("4499FF"),
            scrollbar_track: [0.04, 0.05, 0.10, 0.4],
            scrollbar_thumb: [1.0, 0.0, 0.235, 0.55],
            ansi_colors: [
                hex("0F1220"), // 0  Black        — deep midnight
                hex("FF2040"), // 1  Red           — electric crimson
                hex("00CC44"), // 2  Green         — matrix neon
                hex("FFAA00"), // 3  Yellow        — amber glow
                hex("3388FF"), // 4  Blue          — electric cobalt
                hex("CC00BB"), // 5  Magenta       — neon violet
                hex("00C8D8"), // 6  Cyan          — blade runner teal
                hex("8A98A8"), // 7  White         — muted silver
                hex("252C40"), // 8  BrightBlack   — dark blue-gray
                hex("FF4466"), // 9  BrightRed     — bright crimson
                hex("00FF55"), // 10 BrightGreen   — terminal green
                hex("FFD000"), // 11 BrightYellow  — bright amber
                hex("55AAFF"), // 12 BrightBlue    — electric blue
                hex("FF44DD"), // 13 BrightMagenta — hot pink
                hex("00EEFF"), // 14 BrightCyan    — ice blue
                hex("D4DCE8"), // 15 BrightWhite   — clean white
            ],
        }
    }

    pub fn dracula() -> Self {
        Self {
            bg: hex("282a36"),
            fg: hex("f8f8f2"),
            cursor: hex("ff79c6"),
            selection: [0.68, 0.72, 0.84, 0.4],
            tab_bar_bg: hex("21222c"),
            tab_active_bg: hex("44475a"),
            tab_inactive_bg: hex("21222c"),
            tab_hover_bg: [0.68, 0.72, 0.84, 0.15],
            tab_text: hex("f8f8f2"),
            tab_text_inactive: hex("6272a4"),
            tab_button_text: hex("f8f8f2"),
            tab_separator: hex("44475a"),
            panel_active_border: hex("bd93f9"),
            panel_inactive_border: hex("44475a"),
            panel_divider: hex("44475a"),
            search_bar_bg: [0.13, 0.14, 0.21, 0.97],
            search_highlight: [0.98, 0.97, 0.66, 0.35],
            search_current: [1.0, 0.72, 0.42, 0.55],
            search_text: hex("f8f8f2"),
            search_text_dim: hex("6272a4"),
            ghost_text: hex("6272a4"),
            hyperlink: hex("8be9fd"),
            scrollbar_track: [0.27, 0.28, 0.35, 0.4],
            scrollbar_thumb: [0.74, 0.58, 0.98, 0.5],
            ansi_colors: [
                hex("21222c"), // 0  Black
                hex("ff5555"), // 1  Red
                hex("50fa7b"), // 2  Green
                hex("f1fa8c"), // 3  Yellow
                hex("bd93f9"), // 4  Blue (purple)
                hex("ff79c6"), // 5  Magenta (pink)
                hex("8be9fd"), // 6  Cyan
                hex("f8f8f2"), // 7  White
                hex("6272a4"), // 8  BrightBlack
                hex("ff6e6e"), // 9  BrightRed
                hex("69ff94"), // 10 BrightGreen
                hex("ffffa5"), // 11 BrightYellow
                hex("d6acff"), // 12 BrightBlue
                hex("ff92df"), // 13 BrightMagenta
                hex("a4ffff"), // 14 BrightCyan
                hex("ffffff"), // 15 BrightWhite
            ],
        }
    }

    pub fn catppuccin_mocha() -> Self {
        Self {
            bg: hex("1e1e2e"),
            fg: hex("cdd6f4"),
            cursor: hex("f5c2e7"),
            selection: [0.80, 0.76, 0.91, 0.4],
            tab_bar_bg: hex("181825"),
            tab_active_bg: hex("313244"),
            tab_inactive_bg: hex("181825"),
            tab_hover_bg: [0.96, 0.76, 0.91, 0.15],
            tab_text: hex("cdd6f4"),
            tab_text_inactive: hex("a6adc8"),
            tab_button_text: hex("cdd6f4"),
            tab_separator: hex("313244"),
            panel_active_border: hex("cba6f7"),
            panel_inactive_border: hex("313244"),
            panel_divider: hex("313244"),
            search_bar_bg: [0.12, 0.12, 0.18, 0.97],
            search_highlight: [0.97, 0.76, 0.49, 0.35],
            search_current: [0.96, 0.62, 0.42, 0.55],
            search_text: hex("cdd6f4"),
            search_text_dim: hex("a6adc8"),
            ghost_text: hex("585b70"),
            hyperlink: hex("89b4fa"),
            scrollbar_track: [0.18, 0.18, 0.24, 0.4],
            scrollbar_thumb: [0.80, 0.65, 0.97, 0.5],
            ansi_colors: [
                hex("45475a"), // 0  Black (Surface1)
                hex("f38ba8"), // 1  Red
                hex("a6e3a1"), // 2  Green
                hex("f9e2af"), // 3  Yellow
                hex("89b4fa"), // 4  Blue
                hex("f5c2e7"), // 5  Magenta (Pink)
                hex("94e2d5"), // 6  Cyan (Teal)
                hex("bac2de"), // 7  White (Subtext1)
                hex("585b70"), // 8  BrightBlack (Surface2)
                hex("f38ba8"), // 9  BrightRed
                hex("a6e3a1"), // 10 BrightGreen
                hex("f9e2af"), // 11 BrightYellow
                hex("89b4fa"), // 12 BrightBlue
                hex("f5c2e7"), // 13 BrightMagenta
                hex("94e2d5"), // 14 BrightCyan
                hex("cdd6f4"), // 15 BrightWhite (Text)
            ],
        }
    }

    pub fn tokyo_night() -> Self {
        Self {
            bg: hex("1a1b26"),
            fg: hex("c0caf5"),
            cursor: hex("bb9af7"),
            selection: [0.12, 0.47, 0.71, 0.4],
            tab_bar_bg: hex("16161e"),
            tab_active_bg: hex("2d3149"),
            tab_inactive_bg: hex("16161e"),
            tab_hover_bg: [0.73, 0.60, 0.97, 0.15],
            tab_text: hex("c0caf5"),
            tab_text_inactive: hex("565f89"),
            tab_button_text: hex("c0caf5"),
            tab_separator: hex("2d3149"),
            panel_active_border: hex("7aa2f7"),
            panel_inactive_border: hex("2d3149"),
            panel_divider: hex("2d3149"),
            search_bar_bg: [0.10, 0.11, 0.15, 0.97],
            search_highlight: [0.73, 0.85, 0.47, 0.35],
            search_current: [0.97, 0.55, 0.27, 0.55],
            search_text: hex("c0caf5"),
            search_text_dim: hex("565f89"),
            ghost_text: hex("414868"),
            hyperlink: hex("7dcfff"),
            scrollbar_track: [0.16, 0.18, 0.26, 0.4],
            scrollbar_thumb: [0.73, 0.63, 0.97, 0.5],
            ansi_colors: [
                hex("15161e"), // 0  Black
                hex("f7768e"), // 1  Red
                hex("9ece6a"), // 2  Green
                hex("e0af68"), // 3  Yellow
                hex("7aa2f7"), // 4  Blue
                hex("bb9af7"), // 5  Magenta (Purple)
                hex("7dcfff"), // 6  Cyan
                hex("a9b1d6"), // 7  White
                hex("414868"), // 8  BrightBlack
                hex("f7768e"), // 9  BrightRed
                hex("9ece6a"), // 10 BrightGreen
                hex("e0af68"), // 11 BrightYellow
                hex("7aa2f7"), // 12 BrightBlue
                hex("bb9af7"), // 13 BrightMagenta
                hex("7dcfff"), // 14 BrightCyan
                hex("c0caf5"), // 15 BrightWhite
            ],
        }
    }

    pub fn high_contrast_dark() -> Self {
        Self {
            bg: hex("000000"),
            fg: hex("FFFFFF"),
            cursor: hex("FFFF00"),
            selection: [1.0, 1.0, 1.0, 0.3],
            tab_bar_bg: hex("000000"),
            tab_active_bg: hex("000000"),
            tab_inactive_bg: hex("000000"),
            tab_hover_bg: [1.0, 1.0, 0.0, 0.2],
            tab_text: hex("FFFFFF"),
            tab_text_inactive: hex("FFFFFF"),
            tab_button_text: hex("FFFF00"),
            tab_separator: hex("FFFFFF"),
            panel_active_border: hex("FFFF00"),
            panel_inactive_border: hex("FFFFFF"),
            panel_divider: hex("FFFFFF"),
            search_bar_bg: hex("000000"),
            search_highlight: [1.0, 1.0, 0.0, 0.4],
            search_current: [1.0, 1.0, 0.0, 0.7],
            search_text: hex("FFFFFF"),
            search_text_dim: hex("FFFFFF"),
            ghost_text: hex("FFFFFF"),
            hyperlink: hex("FFFF00"),
            scrollbar_track: [0.5, 0.5, 0.5, 0.4],
            scrollbar_thumb: [1.0, 1.0, 1.0, 0.7],
            ansi_colors: [
                hex("000000"), // 0  Black
                hex("FF0000"), // 1  Red
                hex("00FF00"), // 2  Green
                hex("FFFF00"), // 3  Yellow
                hex("0000FF"), // 4  Blue
                hex("FF00FF"), // 5  Magenta
                hex("00FFFF"), // 6  Cyan
                hex("FFFFFF"), // 7  White
                hex("808080"), // 8  BrightBlack
                hex("FF4444"), // 9  BrightRed
                hex("44FF44"), // 10 BrightGreen
                hex("FFFF44"), // 11 BrightYellow
                hex("4444FF"), // 12 BrightBlue
                hex("FF44FF"), // 13 BrightMagenta
                hex("44FFFF"), // 14 BrightCyan
                hex("FFFFFF"), // 15 BrightWhite
            ],
        }
    }

    pub fn load(name: &str, config_dir: Option<PathBuf>) -> Self {
        if let Some(dir) = config_dir {
            let path = dir.join("themes").join(format!("{}.toml", name));
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(parsed) = toml::from_str::<ThemeToml>(&content) {
                        let base = Self::builtin(name);
                        return Self::merge(base, &parsed.colors);
                    }
                }
            }
        }
        Self::builtin(name)
    }

    fn builtin(name: &str) -> Self {
        match name {
            "dracula" => Self::dracula(),
            "catppuccin-mocha" => Self::catppuccin_mocha(),
            "tokyo-night" => Self::tokyo_night(),
            "high-contrast" => Self::high_contrast_dark(),
            _ => Self::synapse_(),
        }
    }

    fn merge(mut base: Self, colors: &ColorsToml) -> Self {
        macro_rules! apply {
            ($field:ident) => {
                if let Some(ref s) = colors.$field {
                    base.$field = hex(s);
                }
            };
        }
        apply!(bg);
        apply!(fg);
        apply!(cursor);
        apply!(selection);
        apply!(tab_bar_bg);
        apply!(tab_active_bg);
        apply!(tab_inactive_bg);
        apply!(tab_hover_bg);
        apply!(tab_text);
        apply!(tab_text_inactive);
        apply!(tab_button_text);
        apply!(tab_separator);
        apply!(panel_active_border);
        apply!(panel_inactive_border);
        apply!(panel_divider);
        apply!(search_bar_bg);
        apply!(search_highlight);
        apply!(search_current);
        apply!(search_text);
        apply!(search_text_dim);
        apply!(ghost_text);
        apply!(hyperlink);
        if let Some(ref ansi) = colors.ansi_colors {
            for (i, s) in ansi.iter().enumerate().take(16) {
                base.ansi_colors[i] = hex(s);
            }
        }
        base
    }
}

#[derive(Deserialize)]
struct ThemeToml {
    #[serde(default)]
    colors: ColorsToml,
}

#[derive(Deserialize, Default)]
struct ColorsToml {
    bg: Option<String>,
    fg: Option<String>,
    cursor: Option<String>,
    selection: Option<String>,
    tab_bar_bg: Option<String>,
    tab_active_bg: Option<String>,
    tab_inactive_bg: Option<String>,
    tab_hover_bg: Option<String>,
    tab_text: Option<String>,
    tab_text_inactive: Option<String>,
    tab_button_text: Option<String>,
    tab_separator: Option<String>,
    panel_active_border: Option<String>,
    panel_inactive_border: Option<String>,
    panel_divider: Option<String>,
    search_bar_bg: Option<String>,
    search_highlight: Option<String>,
    search_current: Option<String>,
    search_text: Option<String>,
    search_text_dim: Option<String>,
    ghost_text: Option<String>,
    hyperlink: Option<String>,
    ansi_colors: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_opaque() {
        let c = hex("ff3d94");
        assert!((c[0] - 1.0).abs() < 0.01);
        assert!((c[3] - 1.0).abs() < 0.001);
    }

    #[test]
    fn hex_with_hash() {
        let a = hex("210b4b");
        let b = hex("#210b4b");
        assert_eq!(a, b);
    }

    #[test]
    fn hex_with_alpha() {
        let c = hex("ff3d9422");
        assert!((c[3] - (0x22 as f32 / 255.0)).abs() < 0.001);
    }

    #[test]
    fn synapse_theme_smoke() {
        let t = Theme::synapse_();
        assert!((t.bg[3] - 1.0).abs() < 0.001, "bg alpha should be 1.0");
        assert!(t.tab_text[3] == 1.0, "tab text alpha should be 1.0");
        assert!(t.cursor[3] == 1.0, "cursor alpha should be 1.0");
    }

    #[test]
    fn all_builtin_themes_load() {
        for name in &["synapse_", "dracula", "catppuccin-mocha", "tokyo-night", "high-contrast"] {
            let t = Theme::load(name, None);
            // bg alpha should always be 1.0
            assert!(
                (t.bg[3] - 1.0).abs() < 0.001,
                "theme {} bg alpha != 1",
                name
            );
        }
    }

    #[test]
    fn merge_overrides_ansi_colors() {
        let base = Theme::synapse_();
        let toml: ThemeToml =
            toml::from_str("[colors]\nansi_colors = [\"#000000\", \"#ff0000\"]\n").unwrap();
        let merged = Theme::merge(base, &toml.colors);
        // Index 0: override to black
        assert!((merged.ansi_colors[0][0] - 0.0).abs() < 0.01);
        // Index 1: override to red
        assert!((merged.ansi_colors[1][0] - 1.0).abs() < 0.01);
        // Index 2..15 remain unchanged from base
        assert_eq!(merged.ansi_colors[2], Theme::synapse_().ansi_colors[2]);
        // Did NOT clobber non-ansi fields
        assert!((merged.fg[0] - Theme::synapse_().fg[0]).abs() < 0.001);
    }

    #[test]
    fn merge_preserves_ansi_when_not_in_toml() {
        let base = Theme::synapse_();
        let toml: ThemeToml = toml::from_str("[colors]\nbg = \"#ffffff\"\n").unwrap();
        let merged = Theme::merge(base, &toml.colors);
        // bg got overridden
        assert!((merged.bg[0] - 1.0).abs() < 0.01);
        // ansi_colors are untouched
        assert_eq!(merged.ansi_colors, Theme::synapse_().ansi_colors);
    }

    #[test]
    fn custom_theme_from_file_with_ansi() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("synapse_test_themes");
        let themes_dir = dir.join("themes");
        let _ = std::fs::create_dir_all(&themes_dir);
        let theme_path = themes_dir.join("custom.toml");
        let mut f = std::fs::File::create(&theme_path).unwrap();
        writeln!(
            f,
            "[colors]\nbg = \"#111111\"\nfg = \"#eeeeee\"\nansi_colors = [\"#000000\", \"#ff0000\", \"#00ff00\", \"#0000ff\"]"
        )
        .unwrap();

        let t = Theme::load("custom", Some(dir.clone()));
        assert!((t.bg[0] - 0.0666).abs() < 0.01);
        assert!((t.fg[0] - 0.9333).abs() < 0.01);
        // ansi[0] = black, ansi[1] = red
        assert!((t.ansi_colors[0][0] - 0.0).abs() < 0.01);
        assert!((t.ansi_colors[1][0] - 1.0).abs() < 0.01);
        // ansi[2] = green (from file), not from builtin
        assert!((t.ansi_colors[2][1] - 1.0).abs() < 0.01);
        // ansi[3] = blue (from file), not from builtin
        assert!((t.ansi_colors[3][2] - 1.0).abs() < 0.01);
        // ansi[4..15] should still be from base (not overridden)
        assert_eq!(t.ansi_colors[4], Theme::synapse_().ansi_colors[4]);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
