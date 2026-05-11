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
        Self::luna()
    }
}

impl Theme {
    pub fn luna() -> Self {
        Self {
            bg: hex("11131a"),
            fg: hex("d2d5db"),
            cursor: hex("7098cc"),
            selection: hex("7098cc40"),
            tab_bar_bg: hex("181b24"),
            tab_active_bg: hex("222739"),
            tab_inactive_bg: hex("181b24"),
            tab_hover_bg: hex("7098cc1a"),
            tab_text: hex("e5e8ee"),
            tab_text_inactive: hex("737a8c"),
            tab_button_text: hex("e5e8ee"),
            tab_separator: hex("222739"),
            panel_active_border: hex("7098cc"),
            panel_inactive_border: hex("222739"),
            panel_divider: hex("222739"),
            search_bar_bg: hex("181b24cc"),
            search_highlight: hex("d4a72c40"),
            search_current: hex("d4734b80"),
            search_text: hex("e5e8ee"),
            search_text_dim: hex("737a8c"),
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
            _ => Self::luna(),
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
    fn luna_theme_smoke() {
        let t = Theme::luna();
        assert!((t.bg[3] - 1.0).abs() < 0.001, "bg alpha should be 1.0");
        assert!(t.tab_text[3] == 1.0, "tab text alpha should be 1.0");
        assert!(t.cursor[3] == 1.0, "cursor alpha should be 1.0");
    }

    #[test]
    fn all_builtin_themes_load() {
        for name in &["luna", "dracula", "catppuccin-mocha", "tokyo-night"] {
            let t = Theme::load(name, None);
            // bg alpha should always be 1.0
            assert!(
                (t.bg[3] - 1.0).abs() < 0.001,
                "theme {} bg alpha != 1",
                name
            );
        }
    }
}
