use winit::event::KeyEvent;
use winit::keyboard::{Key, ModifiersState, NamedKey};

#[derive(Debug)]
#[allow(dead_code)] // Phase 2: ScrollUp/ScrollDown payloads will be reattached to scrollback.
pub enum InputAction {
    Write(Vec<u8>),
    ScrollUp(usize),
    ScrollDown(usize),
    ScrollToTop,
    ScrollToBottom,
    Copy,
    Paste,
    Ignore,
}

impl InputAction {
    pub fn from_key(event: &KeyEvent, modifiers: ModifiersState, application_cursor: bool) -> Self {
        let text = event.text.as_deref();

        if let Some(text) = text {
            let bytes = text.as_bytes();
            let (ctrl, shift) = (modifiers.control_key(), modifiers.shift_key());

            // Ctrl+Shift+C → copy
            if bytes == [3] && ctrl && shift {
                return InputAction::Copy;
            }

            // Ctrl+Shift+V → paste
            if bytes == [22] && ctrl && shift {
                return InputAction::Paste;
            }

            // Ctrl+letter combos already converted by winit (text contains the ctrl char)
            if !text.is_empty() {
                return InputAction::Write(bytes.to_vec());
            }
        }

        let key_ref = event.logical_key.as_ref();
        Self::from_named_key(&key_ref, modifiers, application_cursor)
    }

    /// Kitty keyboard protocol encoding (Level 1+).
    /// Sends CSI u sequences for printable keys with modifiers, disambiguating
    /// Ctrl+letter from control characters and enabling modifier reporting for
    /// named keys. Falls back to legacy sequences for unhandled cases.
    pub fn from_key_kitty(
        event: &KeyEvent,
        modifiers: ModifiersState,
        flags: u8,
        is_release: bool,
    ) -> Self {
        let report_all_keys = flags & 1 != 0;
        let report_releases = flags & 2 != 0;
        let report_all_mods = flags & 4 != 0;

        // Ignore releases unless the app requested them.
        if is_release && !report_releases {
            return InputAction::Ignore;
        }

        let ctrl = modifiers.control_key();
        let shift = modifiers.shift_key();
        let alt = modifiers.alt_key();

        // Modifier value: (shift | alt<<1 | ctrl<<2 | super<<3) + 1
        let mod_bits: u32 =
            (shift as u32) | ((alt as u32) << 1) | ((ctrl as u32) << 2);
        let mods = mod_bits + 1;
        let event_type: u32 = if is_release { 3 } else { 1 };

        // CSI u helper: `ESC [ <cp> ; <mods> ; <event> u`
        // Trailing fields omitted when they equal their defaults (mods=1, event=1).
        let csi_u = |cp: u32| -> Vec<u8> {
            if mods == 1 && event_type == 1 && !report_all_mods {
                if report_all_keys {
                    format!("\x1b[{cp};1u").into_bytes()
                } else {
                    // No modifiers, press only — use legacy path
                    Vec::new()
                }
            } else if event_type == 1 && !report_releases {
                format!("\x1b[{cp};{mods}u").into_bytes()
            } else {
                format!("\x1b[{cp};{mods};{event_type}u").into_bytes()
            }
        };

        use winit::keyboard::{Key, NamedKey};
        match &event.logical_key {
            Key::Named(named) => {
                // KKP codepoints for named keys (from kitty's specification).
                let (cp, legacy): (Option<u32>, Option<InputAction>) = match named {
                    NamedKey::Escape => (Some(27), Some(InputAction::Write(b"\x1b".to_vec()))),
                    NamedKey::Enter => (Some(13), Some(InputAction::Write(b"\r".to_vec()))),
                    NamedKey::Tab => (Some(9), Some(InputAction::Write(b"\t".to_vec()))),
                    NamedKey::Backspace => (Some(127), Some(InputAction::Write(b"\x7f".to_vec()))),
                    NamedKey::Insert => (Some(57348), None),
                    NamedKey::Delete => (Some(57349), None),
                    NamedKey::ArrowLeft => (Some(57350), None),
                    NamedKey::ArrowRight => (Some(57351), None),
                    NamedKey::ArrowUp => (Some(57352), None),
                    NamedKey::ArrowDown => (Some(57353), None),
                    NamedKey::PageUp => (Some(57354), None),
                    NamedKey::PageDown => (Some(57355), None),
                    NamedKey::Home => (Some(57356), None),
                    NamedKey::End => (Some(57357), None),
                    NamedKey::F1 => (Some(57376), None),
                    NamedKey::F2 => (Some(57377), None),
                    NamedKey::F3 => (Some(57378), None),
                    NamedKey::F4 => (Some(57379), None),
                    NamedKey::F5 => (Some(57380), None),
                    NamedKey::F6 => (Some(57381), None),
                    NamedKey::F7 => (Some(57382), None),
                    NamedKey::F8 => (Some(57383), None),
                    NamedKey::F9 => (Some(57384), None),
                    NamedKey::F10 => (Some(57385), None),
                    NamedKey::F11 => (Some(57386), None),
                    NamedKey::F12 => (Some(57387), None),
                    _ => (None, None),
                };

                if let Some(cp) = cp {
                    let bytes = csi_u(cp);
                    if !bytes.is_empty() {
                        return InputAction::Write(bytes);
                    }
                    // No modifiers + not reporting all keys → use legacy for basic named keys.
                    if let Some(leg) = legacy {
                        return leg;
                    }
                }

                // Fall back to legacy for anything unhandled.
                Self::from_key(event, modifiers, false)
            }
            Key::Character(c) => {
                if let Some(ch) = c.chars().next() {
                    let cp = ch as u32;
                    // Only use CSI u when there are modifiers (Ctrl/Alt) that would
                    // otherwise be ambiguous in the legacy encoding.
                    if ctrl || alt || report_all_keys {
                        let bytes = csi_u(cp);
                        if !bytes.is_empty() {
                            return InputAction::Write(bytes);
                        }
                    }
                    // No modifiers: send the character as-is (legacy path is correct).
                    return InputAction::Write(c.as_bytes().to_vec());
                }
                InputAction::Ignore
            }
            _ => Self::from_key(event, modifiers, false),
        }
    }

    fn from_named_key(
        key: &Key<&str>,
        modifiers: ModifiersState,
        application_cursor: bool,
    ) -> Self {
        use Key::Named;
        let named = match key {
            Named(n) => n,
            Key::Character(c) => {
                if c.is_empty() {
                    return InputAction::Ignore;
                }
                let ch = c.chars().next().unwrap();
                if modifiers.control_key() && !modifiers.shift_key() {
                    let ctrl_byte = match ch {
                        'a'..='z' => (ch as u8) - b'a' + 1,
                        'A'..='Z' => (ch as u8) - b'A' + 1,
                        '[' => 27,
                        '\\' => 28,
                        ']' => 29,
                        '^' => 30,
                        '_' => 31,
                        '2' | '@' => 0,
                        '6' => 30,
                        '-' => 31,
                        _ => return InputAction::Ignore,
                    };
                    return InputAction::Write(vec![ctrl_byte]);
                }
                return InputAction::Write(c.as_bytes().to_vec());
            }
            _ => return InputAction::Ignore,
        };

        let ctrl = modifiers.control_key();
        let shift = modifiers.shift_key();

        match named {
            NamedKey::Enter => InputAction::Write(b"\r".to_vec()),
            NamedKey::Backspace => InputAction::Write(b"\x7f".to_vec()),
            NamedKey::Tab => {
                if shift {
                    InputAction::Write(b"\x1b[Z".to_vec())
                } else {
                    InputAction::Write(b"\t".to_vec())
                }
            }
            NamedKey::Escape => InputAction::Write(b"\x1b".to_vec()),
            NamedKey::ArrowUp => {
                if shift {
                    InputAction::Write(b"\x1b[1;2A".to_vec())
                } else if ctrl {
                    InputAction::Write(b"\x1b[1;5A".to_vec())
                } else if application_cursor {
                    InputAction::Write(b"\x1bOA".to_vec())
                } else {
                    InputAction::Write(b"\x1b[A".to_vec())
                }
            }
            NamedKey::ArrowDown => {
                if shift {
                    InputAction::Write(b"\x1b[1;2B".to_vec())
                } else if ctrl {
                    InputAction::Write(b"\x1b[1;5B".to_vec())
                } else if application_cursor {
                    InputAction::Write(b"\x1bOB".to_vec())
                } else {
                    InputAction::Write(b"\x1b[B".to_vec())
                }
            }
            NamedKey::ArrowRight => {
                if shift {
                    InputAction::Write(b"\x1b[1;2C".to_vec())
                } else if ctrl {
                    InputAction::Write(b"\x1b[1;5C".to_vec())
                } else if application_cursor {
                    InputAction::Write(b"\x1bOC".to_vec())
                } else {
                    InputAction::Write(b"\x1b[C".to_vec())
                }
            }
            NamedKey::ArrowLeft => {
                if shift {
                    InputAction::Write(b"\x1b[1;2D".to_vec())
                } else if ctrl {
                    InputAction::Write(b"\x1b[1;5D".to_vec())
                } else if application_cursor {
                    InputAction::Write(b"\x1bOD".to_vec())
                } else {
                    InputAction::Write(b"\x1b[D".to_vec())
                }
            }
            NamedKey::Home => {
                if shift {
                    InputAction::Write(b"\x1b[1;2H".to_vec())
                } else if ctrl {
                    InputAction::Write(b"\x1b[1;5H".to_vec())
                } else {
                    InputAction::Write(b"\x1b[H".to_vec())
                }
            }
            NamedKey::End => {
                if shift {
                    InputAction::Write(b"\x1b[1;2F".to_vec())
                } else if ctrl {
                    InputAction::Write(b"\x1b[1;5F".to_vec())
                } else {
                    InputAction::Write(b"\x1b[F".to_vec())
                }
            }
            NamedKey::Delete => {
                if shift {
                    InputAction::Write(b"\x1b[3;2~".to_vec())
                } else if ctrl {
                    InputAction::Write(b"\x1b[3;5~".to_vec())
                } else {
                    InputAction::Write(b"\x1b[3~".to_vec())
                }
            }
            NamedKey::Insert => InputAction::Write(b"\x1b[2~".to_vec()),
            NamedKey::PageUp => {
                if ctrl && shift {
                    InputAction::ScrollToTop
                } else if shift {
                    InputAction::ScrollUp(24)
                } else {
                    InputAction::Write(b"\x1b[5~".to_vec())
                }
            }
            NamedKey::PageDown => {
                if ctrl && shift {
                    InputAction::ScrollToBottom
                } else if shift {
                    InputAction::ScrollDown(24)
                } else {
                    InputAction::Write(b"\x1b[6~".to_vec())
                }
            }
            NamedKey::F1 => InputAction::Write(b"\x1bOP".to_vec()),
            NamedKey::F2 => InputAction::Write(b"\x1bOQ".to_vec()),
            NamedKey::F3 => InputAction::Write(b"\x1bOR".to_vec()),
            NamedKey::F4 => InputAction::Write(b"\x1bOS".to_vec()),
            NamedKey::F5 => InputAction::Write(b"\x1b[15~".to_vec()),
            NamedKey::F6 => InputAction::Write(b"\x1b[17~".to_vec()),
            NamedKey::F7 => InputAction::Write(b"\x1b[18~".to_vec()),
            NamedKey::F8 => InputAction::Write(b"\x1b[19~".to_vec()),
            NamedKey::F9 => InputAction::Write(b"\x1b[20~".to_vec()),
            NamedKey::F10 => InputAction::Write(b"\x1b[21~".to_vec()),
            NamedKey::F11 => InputAction::Write(b"\x1b[23~".to_vec()),
            NamedKey::F12 => InputAction::Write(b"\x1b[24~".to_vec()),
            _ => InputAction::Ignore,
        }
    }
}
