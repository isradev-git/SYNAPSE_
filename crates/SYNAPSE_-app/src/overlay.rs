use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

use synapse_config::PluginCommand;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverlayStatus {
    Running,
    Completed,
    Failed,
}

pub struct OverlayState {
    pub active: bool,
    pub title: String,
    pub lines: Vec<String>,
    pub scroll: usize,
    pub status: OverlayStatus,
}

impl OverlayState {
    pub fn new() -> Self {
        Self {
            active: false,
            title: String::new(),
            lines: Vec::new(),
            scroll: 0,
            status: OverlayStatus::Completed,
        }
    }

    pub fn close(&mut self) {
        self.active = false;
        self.lines.clear();
        self.title.clear();
        self.scroll = 0;
    }

    pub fn scroll_up(&mut self, visible_lines: usize) {
        if visible_lines < self.lines.len() && self.scroll + visible_lines < self.lines.len() {
            self.scroll += 1;
        }
    }

    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn visible_lines(&self, visible_count: usize) -> &[String] {
        let start = self.scroll.min(self.lines.len().saturating_sub(1));
        let end = (start + visible_count).min(self.lines.len());
        &self.lines[start..end]
    }
}

pub fn expand_variables(
    template: &str,
    cwd: &str,
    selected_text: &str,
    clipboard_text: &str,
) -> String {
    template
        .replace("$CURRENT_PANE_CWD", cwd)
        .replace("$SELECTED_TEXT", selected_text)
        .replace("$CURRENT_FILE", "")
        .replace("$CLIPBOARD", clipboard_text)
}

pub fn resolve_plugin_cwd(
    plugin: &PluginCommand,
    pane_cwd: &str,
    selected_text: &str,
    clipboard_text: &str,
) -> Option<String> {
    if let Some(cwd_tpl) = &plugin.cwd {
        let expanded = expand_variables(cwd_tpl, pane_cwd, selected_text, clipboard_text);
        if expanded.is_empty() {
            None
        } else {
            Some(expanded)
        }
    } else {
        if pane_cwd.is_empty() {
            None
        } else {
            Some(pane_cwd.to_string())
        }
    }
}

pub fn resolve_plugin_command(
    plugin: &PluginCommand,
    cwd: &str,
    selected_text: &str,
    clipboard_text: &str,
) -> String {
    expand_variables(&plugin.command, cwd, selected_text, clipboard_text)
}

pub fn spawn_overlay_command(tx: mpsc::Sender<OverlayEvent>, title: String, shell_cmd: String) {
    thread::Builder::new()
        .name("synapse-plugin-overlay".into())
        .spawn(move || {
            let child = Command::new("sh")
                .arg("-c")
                .arg(&shell_cmd)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn();

            match child {
                Ok(mut child) => {
                    let mut out_buf = String::new();
                    let mut err_buf = String::new();

                    if let Some(ref mut stdout) = child.stdout {
                        let _ = stdout.read_to_string(&mut out_buf);
                    }
                    if let Some(ref mut stderr) = child.stderr {
                        let _ = stderr.read_to_string(&mut err_buf);
                    }

                    let status = child.wait();
                    let exit_ok = status.map(|s| s.success()).unwrap_or(false);

                    let output: Vec<String> = out_buf
                        .lines()
                        .chain(err_buf.lines())
                        .map(|l| l.to_string())
                        .collect();

                    let _ = tx.send(OverlayEvent::Output {
                        title,
                        lines: output,
                        success: exit_ok,
                    });
                }
                Err(e) => {
                    let _ = tx.send(OverlayEvent::Output {
                        title,
                        lines: vec![format!("Failed to spawn: {}", e)],
                        success: false,
                    });
                }
            }
        })
        .ok();
}

#[derive(Debug, Clone)]
pub enum OverlayEvent {
    Output {
        #[allow(dead_code)]
        title: String,
        lines: Vec<String>,
        success: bool,
    },
}

pub fn run_replace_selection(
    command: &PluginCommand,
    cwd: &str,
    selected_text: &str,
    clipboard_text: &str,
) -> Result<String, String> {
    let cmd_str = resolve_plugin_command(command, cwd, selected_text, clipboard_text);

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(&cmd_str)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn failed: {}", e))?;

    if let Some(ref mut stdin) = child.stdin {
        use std::io::Write;
        let _ = stdin.write_all(selected_text.as_bytes());
    }

    let mut out_buf = String::new();
    if let Some(ref mut stdout) = child.stdout {
        stdout
            .read_to_string(&mut out_buf)
            .map_err(|e| format!("read stdout failed: {}", e))?;
    }

    let status = child.wait().map_err(|e| format!("wait failed: {}", e))?;
    if !status.success() {
        return Err(format!("command exited with {}", status));
    }

    Ok(out_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_state_new() {
        let state = OverlayState::new();
        assert!(!state.active);
        assert!(state.title.is_empty());
        assert!(state.lines.is_empty());
        assert_eq!(state.scroll, 0);
        assert_eq!(state.status, OverlayStatus::Completed);
    }

    #[test]
    fn test_overlay_close() {
        let mut state = OverlayState::new();
        state.active = true;
        state.title = "test".into();
        state.lines = vec!["a".into()];
        state.close();
        assert!(!state.active);
        assert!(state.title.is_empty());
        assert!(state.lines.is_empty());
    }

    #[test]
    fn test_overlay_scroll() {
        let mut state = OverlayState::new();
        state.lines = vec![
            "line1".into(),
            "line2".into(),
            "line3".into(),
            "line4".into(),
            "line5".into(),
        ];
        state.scroll_up(2);
        assert_eq!(state.scroll, 1);
        state.scroll_up(2);
        assert_eq!(state.scroll, 2);
        state.scroll_down();
        assert_eq!(state.scroll, 1);
        state.scroll_down();
        state.scroll_down();
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn test_visible_lines() {
        let mut state = OverlayState::new();
        state.lines = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        let vis = state.visible_lines(2);
        assert_eq!(vis.len(), 2);
        assert_eq!(vis[0], "a");
        assert_eq!(vis[1], "b");

        state.scroll = 2;
        let vis = state.visible_lines(3);
        assert_eq!(vis.len(), 2);
        assert_eq!(vis[0], "c");
        assert_eq!(vis[1], "d");
    }

    #[test]
    fn test_expand_variables() {
        let result = expand_variables(
            "cd $CURRENT_PANE_CWD && echo $SELECTED_TEXT",
            "/home",
            "hello",
            "",
        );
        assert_eq!(result, "cd /home && echo hello");
    }

    #[test]
    fn test_expand_variables_clipboard() {
        let result = expand_variables("echo $CLIPBOARD", "", "", "clip-data");
        assert_eq!(result, "echo clip-data");
    }

    #[test]
    fn test_resolve_plugin_cwd_defaults_to_pane() {
        let plugin = PluginCommand {
            name: "test".into(),
            keybind: None,
            command: "echo hi".into(),
            cwd: None,
            split: "tab".into(),
            replace_selection: false,
        };
        let result = resolve_plugin_cwd(&plugin, "/home/user", "", "");
        assert_eq!(result, Some("/home/user".into()));
    }

    #[test]
    fn test_resolve_plugin_cwd_template() {
        let plugin = PluginCommand {
            name: "test".into(),
            keybind: None,
            command: "echo hi".into(),
            cwd: Some("$CURRENT_PANE_CWD/project".into()),
            split: "tab".into(),
            replace_selection: false,
        };
        let result = resolve_plugin_cwd(&plugin, "/home/user", "", "");
        assert_eq!(result, Some("/home/user/project".into()));
    }

    #[test]
    fn test_resolve_plugin_command() {
        let plugin = PluginCommand {
            name: "test".into(),
            keybind: None,
            command: "python3 -m json.tool $SELECTED_TEXT".into(),
            cwd: None,
            split: "tab".into(),
            replace_selection: false,
        };
        let result = resolve_plugin_command(&plugin, "", "{\"a\":1}", "");
        assert_eq!(result, "python3 -m json.tool {\"a\":1}");
    }
}
