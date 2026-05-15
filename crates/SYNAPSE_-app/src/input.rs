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

    /// Kitty keyboard protocol encoding — stubbed for Phase 1.
    /// Phase 2 will reimplement on top of alacritty_terminal's keyboard support.
    #[allow(dead_code)]
    pub fn from_key_kitty(
        _event: &KeyEvent,
        _modifiers: ModifiersState,
        _flags: u8,
        _is_release: bool,
    ) -> Self {
        InputAction::Ignore
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
