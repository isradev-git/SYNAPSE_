use std::collections::{HashSet, VecDeque};
use std::time::Instant;

use crate::history::CommandHistory;

fn read_git_branch_sync(cwd: &str) -> Option<String> {
    let mut dir = std::path::Path::new(cwd).to_path_buf();
    loop {
        let head = dir.join(".git/HEAD");
        if head.exists() {
            if let Ok(content) = std::fs::read_to_string(&head) {
                let content = content.trim();
                if let Some(branch) = content.strip_prefix("ref: refs/heads/") {
                    return Some(branch.to_string());
                }
                if content.len() >= 7 {
                    return Some(content[..7].to_string());
                }
            }
            return None;
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

pub fn spawn_git_branch_reader(cwd: &str) -> std::sync::mpsc::Receiver<Option<String>> {
    let cwd = cwd.to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(read_git_branch_sync(&cwd));
    });
    rx
}

fn read_k8s_context() -> Option<String> {
    let kube_config = std::env::var("KUBECONFIG").ok().unwrap_or_else(|| {
        std::env::var("HOME")
            .map(|h| format!("{}/.kube/config", h))
            .unwrap_or_default()
    });
    if kube_config.is_empty() {
        return None;
    }
    let content = std::fs::read_to_string(&kube_config).ok()?;
    for line in content.lines() {
        if let Some(ctx) = line.strip_prefix("current-context:") {
            let ctx = ctx.trim().trim_matches('"');
            if !ctx.is_empty() {
                return Some(ctx.to_string());
            }
        }
    }
    None
}

fn read_user_host() -> String {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_default();
    let host = std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "localhost".to_string());
    if user.is_empty() {
        host
    } else {
        format!("{}@{}", user, host)
    }
}

use crate::overlay::OverlayState;
use crate::palette::PaletteState;
use alacritty_terminal::vte::ansi::CursorShape;
use synapse_config::{Config, Keybinds, Theme};
use synapse_suggest::Suggester;
use synapse_ui::pane::PaneId;
use synapse_ui::splitter::{PaneRect, PaneTree, SplitDirection};
use winit::keyboard::ModifiersState;

/// Pixel-space bounding box of a clickable URL in the terminal viewport.
#[derive(Clone)]
pub struct UrlSpan {
    pub url: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub struct DividerDrag {
    pub pane_id: PaneId,
    pub direction: SplitDirection,
    pub parent_rect: PaneRect,
}

#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub col: usize,
    pub row: i32,
}

pub struct SearchState {
    pub active: bool,
    pub term: String,
    pub matches: Vec<SearchMatch>,
    pub current_match: usize,
    pub cursor_pos: usize,
    pub regex_mode: bool,
    pub invalid_regex: bool,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            active: false,
            term: String::new(),
            matches: Vec::new(),
            current_match: 0,
            cursor_pos: 0,
            regex_mode: false,
            invalid_regex: false,
        }
    }

    pub fn toggle(&mut self) {
        self.active = !self.active;
        if !self.active {
            self.term.clear();
            self.matches.clear();
            self.current_match = 0;
            self.cursor_pos = 0;
            self.regex_mode = false;
            self.invalid_regex = false;
        }
    }

    pub fn toggle_regex(&mut self) {
        self.regex_mode = !self.regex_mode;
        self.invalid_regex = false;
    }

    pub fn next_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match = (self.current_match + 1) % self.matches.len();
        }
    }

    pub fn prev_match(&mut self) {
        if !self.matches.is_empty() {
            if self.current_match == 0 {
                self.current_match = self.matches.len() - 1;
            } else {
                self.current_match -= 1;
            }
        }
    }

    pub fn insert_char(&mut self, c: char) {
        let pos = self.cursor_pos.min(self.term.len());
        self.term.insert(pos, c);
        self.cursor_pos = pos + 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.term.remove(self.cursor_pos);
        }
    }

    pub fn delete_forward(&mut self) {
        if self.cursor_pos < self.term.len() {
            self.term.remove(self.cursor_pos);
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor_pos < self.term.len() {
            self.cursor_pos += 1;
        }
    }

    pub fn move_home(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor_pos = self.term.len();
    }
}

pub struct SuggestState {
    pub prefix: String,
    pub ghost: Option<String>,
}

impl SuggestState {
    pub fn new() -> Self {
        Self {
            prefix: String::new(),
            ghost: None,
        }
    }

    pub fn clear(&mut self) {
        self.prefix.clear();
        self.ghost = None;
    }

    pub fn has_ghost(&self) -> bool {
        self.ghost_suffix().is_some()
    }

    pub fn ghost_suffix(&self) -> Option<&str> {
        let ghost = self.ghost.as_deref()?;
        if ghost.len() > self.prefix.len() {
            Some(&ghost[self.prefix.len()..])
        } else {
            None
        }
    }

    pub fn ghost_suffix_owned(&self) -> Option<String> {
        self.ghost_suffix().map(|s| s.to_string())
    }

    pub fn next_word_owned(&self) -> Option<String> {
        let suffix = self.ghost_suffix()?;
        if suffix.is_empty() {
            return None;
        }
        let end = suffix.find(' ').map(|i| i + 1).unwrap_or(suffix.len());
        Some(suffix[..end].to_string())
    }

    /// Update prefix from raw PTY bytes. Returns true if re-query is needed.
    pub fn update(&mut self, bytes: &[u8]) -> bool {
        match bytes {
            [0x7f] | [0x08] => {
                self.prefix.pop();
                self.ghost = None;
                true
            }
            // Ctrl+W: delete last word (shell word-erase)
            [0x17] => {
                let trimmed = self.prefix.trim_end_matches(' ');
                let cut = trimmed.rfind(' ').map(|i| i + 1).unwrap_or(0);
                self.prefix.truncate(cut);
                self.ghost = None;
                true
            }
            // Enter / Ctrl+C / Ctrl+U: clear state
            [0x0d] | [0x0a] | [0x03] | [0x15] => {
                self.clear();
                false
            }
            // Any escape sequence (cursor keys, function keys, etc.) changes the
            // command line in ways we can't track — reset prefix entirely.
            [0x1b, ..] => {
                self.prefix.clear();
                self.ghost = None;
                false
            }
            bytes => {
                if !bytes.is_empty() {
                    if let Ok(s) = std::str::from_utf8(bytes) {
                        // Accept printable ASCII and valid multi-byte UTF-8.
                        if s.chars().all(|c| !c.is_control()) {
                            self.prefix.push_str(s);
                            self.ghost = None;
                            return true;
                        }
                    }
                }
                false
            }
        }
    }
}

#[derive(Default)]
#[allow(dead_code)]
pub struct ProfilerData {
    pub frame_time_ms: f32,
    pub cell_count: usize,
    pub draw_calls: usize,
    pub pty_bytes_per_sec: f32,
    pub fps: f32,
    pub atlas_used_percent: f32,
    pub frame_cache_hit_rate: f32,
    pty_bytes_buf: Vec<(std::time::Instant, usize)>,
    frame_times: Vec<f32>,
}

#[allow(dead_code)]
impl ProfilerData {
    pub fn new() -> Self {
        Self {
            frame_time_ms: 0.0,
            cell_count: 0,
            draw_calls: 0,
            pty_bytes_per_sec: 0.0,
            fps: 0.0,
            atlas_used_percent: 0.0,
            frame_cache_hit_rate: 0.0,
            pty_bytes_buf: Vec::new(),
            frame_times: Vec::with_capacity(60),
        }
    }

    pub fn add_frame_time(&mut self, dt_ms: f32) {
        self.frame_times.push(dt_ms);
        if self.frame_times.len() > 60 {
            self.frame_times.remove(0);
        }
        self.fps = if self.frame_times.is_empty() {
            0.0
        } else {
            1000.0 / (self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32)
        };
    }

    pub fn add_pty_bytes(&mut self, bytes: usize) {
        let now = std::time::Instant::now();
        self.pty_bytes_buf.push((now, bytes));
        let cutoff = now - std::time::Duration::from_secs(2);
        self.pty_bytes_buf.retain(|(t, _)| *t > cutoff);
        let total: usize = self.pty_bytes_buf.iter().map(|(_, b)| *b).sum();
        let oldest = self.pty_bytes_buf.first().map(|(t, _)| *t).unwrap_or(now);
        let elapsed = now.duration_since(oldest).as_secs_f32().max(0.001);
        self.pty_bytes_per_sec = total as f32 / elapsed;
    }
}

pub struct AppState {
    pub config: Config,
    pub keybinds: Keybinds,
    pub theme: Theme,
    pub modifiers: ModifiersState,
    pub selecting: bool,
    pub cursor_x: f64,
    pub cursor_y: f64,
    pub dragging_divider: Option<DividerDrag>,
    pub scrollbar_drag: Option<PaneId>,
    pub hover_divider: bool,
    pub hover_tab: Option<usize>,
    pub last_click_time: Instant,
    pub click_count: u8,
    pub search: SearchState,
    pub history_search: HistorySearchState,
    pub suggest: SuggestState,
    pub palette: PaletteState,
    pub suggester: Suggester,
    pub font_size: f32,
    pub fullscreen: bool,
    pub tab_scroll_offset: usize,
    pub hovered_url: Option<String>,
    pub effects_enabled: bool,
    pub cursor_trail: VecDeque<(f32, f32)>,
    pub bell_flash_end: Option<std::time::Instant>,
    pub window_focused: bool,
    /// Cursor shape/blink set by the app via DECSCUSR. Overrides TOML when Some.
    pub term_cursor_style: Option<(CursorShape, bool)>,
    pub in_copy_mode: bool,
    pub pane_label_until: Option<std::time::Instant>,
    pub pane_label_id: u32,
    pub status_bar_visible: bool,
    pub git_branch: Option<String>,
    pub git_branch_rx: Option<std::sync::mpsc::Receiver<Option<String>>>,
    pub k8s_context: Option<String>,
    pub user_host: String,
    pub cached_time_sec: u64,
    pub zoomed_pane: Option<PaneId>,
    pub zoom_saved_tree: Option<PaneTree>,
    pub broadcasting: bool,
    pub recording: bool,
    pub profiler_active: bool,
    pub profiler: ProfilerData,
    pub keybinds_open: bool,
    pub keybinds_scroll: usize,
    pub settings_open: bool,
    pub settings_item: usize,
    pub settings_original_config: Option<Config>,
    pub active_workspace: String,
    pub command_history: CommandHistory,
    pub overlay: OverlayState,
    /// Word occurrence highlights (the word under selection)
    pub word_highlight_word: Option<String>,
    pub word_highlight_positions: HashSet<(usize, i32)>,
}

impl AppState {
    pub fn new(config: Config, keybinds: Keybinds, font_size: f32) -> Self {
        let theme = Theme::load(&config.theme, synapse_config::Config::config_dir());
        let mut suggester = synapse_suggest::load_suggester();
        let effects_enabled = config.effects.enabled;
        let status_bar_visible = config.status_bar;
        let k8s_context = if config.status_bar_show_k8s {
            read_k8s_context()
        } else {
            None
        };
        let mut command_history = CommandHistory::new(config.history_max_entries);
        command_history.set_path(
            crate::history::command_history_path()
                .unwrap_or_else(|| std::path::PathBuf::from("history.json")),
        );
        command_history.load();
        // Feed persistent history into the suggester
        if config.persistent_history {
            for cmd in command_history.commands() {
                suggester.insert(cmd);
            }
        }
        Self {
            config,
            keybinds,
            theme,
            modifiers: ModifiersState::empty(),
            selecting: false,
            cursor_x: 0.0,
            cursor_y: 0.0,
            dragging_divider: None,
            scrollbar_drag: None,
            hover_divider: false,
            hover_tab: None,
            last_click_time: Instant::now(),
            click_count: 0,
            search: SearchState::new(),
            history_search: HistorySearchState::new(),
            suggest: SuggestState::new(),
            palette: PaletteState::new(),
            suggester,
            font_size,
            fullscreen: false,
            tab_scroll_offset: 0,
            hovered_url: None,
            effects_enabled,
            cursor_trail: VecDeque::new(),
            bell_flash_end: None,
            window_focused: true,
            term_cursor_style: None,
            in_copy_mode: false,
            pane_label_until: None,
            pane_label_id: 0,
            status_bar_visible,
            git_branch: None,
            git_branch_rx: None,
            k8s_context,
            user_host: read_user_host(),
            cached_time_sec: 0,
            zoomed_pane: None,
            zoom_saved_tree: None,
            broadcasting: false,
            recording: false,
            profiler_active: false,
            profiler: ProfilerData::new(),
            keybinds_open: false,
            keybinds_scroll: 0,
            settings_open: false,
            settings_item: 0,
            settings_original_config: None,
            active_workspace: String::from("default"),
            command_history,
            overlay: OverlayState::new(),
            word_highlight_word: None,
            word_highlight_positions: HashSet::new(),
        }
    }

    pub fn push_cursor_trail(&mut self, x: f32, y: f32, max_len: usize) {
        if max_len == 0 {
            return;
        }
        if self.cursor_trail.front() != Some(&(x, y)) {
            self.cursor_trail.push_front((x, y));
            while self.cursor_trail.len() > max_len {
                self.cursor_trail.pop_back();
            }
        }
    }
}

pub struct HistorySearchState {
    pub active: bool,
    pub term: String,
    pub history: Vec<String>,
    pub matches: Vec<usize>,
    pub current_match: usize,
}

#[allow(dead_code)]
impl HistorySearchState {
    pub fn new() -> Self {
        Self {
            active: false,
            term: String::new(),
            history: Vec::new(),
            matches: Vec::new(),
            current_match: 0,
        }
    }

    pub fn activate(&mut self) {
        self.active = true;
        self.term.clear();
        self.matches.clear();
        self.current_match = 0;
    }

    pub fn deactivate(&mut self) {
        self.active = false;
        self.term.clear();
        self.history.clear();
        self.matches.clear();
        self.current_match = 0;
    }

    pub fn next_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match = (self.current_match + 1) % self.matches.len();
        }
    }

    pub fn current_text(&self) -> Option<&str> {
        if self.matches.is_empty() {
            None
        } else {
            self.history
                .get(self.matches[self.current_match])
                .map(|s| s.as_str())
        }
    }

    pub fn build_history(&mut self, lines: &[Vec<char>]) {
        let mut seen = std::collections::HashSet::new();
        self.history.clear();
        for line in lines.iter().rev() {
            let text: String = line.iter().collect();
            let trimmed = text.trim();
            if !trimmed.is_empty() && trimmed.len() > 1 {
                let owned = trimmed.to_string();
                if seen.insert(owned.clone()) {
                    self.history.push(owned);
                }
            }
        }
    }

    /// Add persistent command history entries (cross-session).
    pub fn add_persistent_commands(&mut self, commands: impl Iterator<Item = impl AsRef<str>>) {
        let mut seen: std::collections::HashSet<String> = self.history.iter().cloned().collect();
        for cmd in commands {
            let trimmed = cmd.as_ref().trim();
            if trimmed.len() > 1 && seen.insert(trimmed.to_string()) {
                self.history.push(trimmed.to_string());
            }
        }
    }

    pub fn update_filter(&mut self) {
        let term_lower = self.term.to_lowercase();
        self.matches.clear();
        self.current_match = 0;
        for (i, line) in self.history.iter().enumerate() {
            if line.to_lowercase().contains(&term_lower) {
                self.matches.push(i);
            }
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.term.push(c);
    }

    pub fn backspace(&mut self) {
        self.term.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use synapse_config::{Config, Keybinds};

    fn make_state() -> AppState {
        AppState::new(Config::default(), Keybinds::default(), 14.0)
    }

    #[test]
    fn test_cursor_trail_push() {
        let mut state = make_state();
        state.push_cursor_trail(10.0, 20.0, 4);
        state.push_cursor_trail(15.0, 20.0, 4);
        state.push_cursor_trail(20.0, 20.0, 4);
        assert_eq!(state.cursor_trail.len(), 3);
        assert_eq!(state.cursor_trail[0], (20.0, 20.0));

        state.push_cursor_trail(25.0, 20.0, 4);
        state.push_cursor_trail(30.0, 20.0, 4);
        assert_eq!(state.cursor_trail.len(), 4);
    }

    #[test]
    fn test_cursor_trail_no_duplicate() {
        let mut state = make_state();
        state.push_cursor_trail(10.0, 10.0, 4);
        state.push_cursor_trail(10.0, 10.0, 4);
        assert_eq!(state.cursor_trail.len(), 1);
    }

    #[test]
    fn test_cursor_trail_max_zero() {
        let mut state = make_state();
        state.push_cursor_trail(10.0, 10.0, 0);
        assert_eq!(state.cursor_trail.len(), 0);
    }

    #[test]
    fn test_term_cursor_style_initial_none() {
        let state = make_state();
        assert!(state.term_cursor_style.is_none());
    }

    #[test]
    fn test_bell_flash_end_initial_none() {
        let state = make_state();
        assert!(state.bell_flash_end.is_none());
    }

    #[test]
    fn test_bell_flash_active() {
        let mut state = make_state();
        state.bell_flash_end =
            Some(std::time::Instant::now() + std::time::Duration::from_millis(200));
        assert!(state
            .bell_flash_end
            .map(|t| t > std::time::Instant::now())
            .unwrap_or(false));
    }

    #[test]
    fn test_in_copy_mode_initial_false() {
        let state = make_state();
        assert!(!state.in_copy_mode);
    }

    #[test]
    fn test_pane_label_initial_none() {
        let state = make_state();
        assert!(state.pane_label_until.is_none());
        assert_eq!(state.pane_label_id, 0);
    }

    #[test]
    fn test_status_bar_initial_state() {
        let state = make_state();
        // Config::default() has status_bar = true
        assert!(state.status_bar_visible);
        assert!(state.git_branch.is_none());
        assert!(state.git_branch_rx.is_none());
        assert_eq!(state.cached_time_sec, 0);
        assert!(!state.user_host.is_empty());
    }
}
