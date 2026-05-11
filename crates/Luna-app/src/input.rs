use winit::event::KeyEvent;
use winit::keyboard::{Key, ModifiersState, NamedKey};

use luna_terminal::kitty;

#[derive(Debug)]
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

    /// Encode a key event using the Kitty keyboard protocol.
    /// `flags` are the active Kitty progressive enhancement flags.
    /// `is_release` is true when this is a key release event.
    pub fn from_key_kitty(
        event: &KeyEvent,
        modifiers: ModifiersState,
        flags: u8,
        is_release: bool,
    ) -> Self {
        let key_ref = event.logical_key.as_ref();

        let mod_val =
            kitty::encode_modifiers(
                modifiers.shift_key(),
                modifiers.alt_key(),
                modifiers.control_key(),
                modifiers.super_key(),
            );

        let event_type = if flags & kitty::KITTY_REPORT_EVENTS != 0 {
            if is_release {
                Some(3u8)
            } else if event.repeat {
                Some(2u8)
            } else {
                Some(1u8)
            }
        } else {
            None
        };

        // In report-all mode (flag 8), encode ALL keys as escape codes
        if flags & kitty::KITTY_REPORT_ALL != 0 {
            return Self::encode_kitty_all(&key_ref, mod_val, event_type);
        }

        // Disambiguate mode (flag 1)
        if flags & kitty::KITTY_DISAMBIGUATE != 0 {
            return Self::encode_kitty_disambiguate(&key_ref, event, modifiers, mod_val, event_type);
        }

        // Legacy mode (shouldn't reach here normally)
        InputAction::Ignore
    }

    /// Report-all mode: every key is sent as a CSI escape code.
    fn encode_kitty_all(
        key: &Key<&str>,
        mod_val: u16,
        event_type: Option<u8>,
    ) -> Self {
        let keycode = match key {
            Key::Named(named) => named_to_kitty_keycode(named),
            Key::Character(c) => {
                if let Some(ch) = c.chars().next() {
                    ch as u32
                } else {
                    0
                }
            }
            _ => return InputAction::Ignore,
        };

        if keycode == 0 {
            return InputAction::Ignore;
        }

        let bytes = kitty::encode_key_event(keycode, mod_val, event_type);
        InputAction::Write(bytes)
    }

    /// Disambiguate mode: escape codes for non-text keys + disambiguated ctrl combos.
    /// Enter, Tab, Backspace without ctrl still send legacy bytes (spec exceptions).
    fn encode_kitty_disambiguate(
        key: &Key<&str>,
        event: &KeyEvent,
        modifiers: ModifiersState,
        mod_val: u16,
        event_type: Option<u8>,
    ) -> Self {
        let ctrl = modifiers.control_key();
        let alt = modifiers.alt_key();

        match key {
            Key::Named(named) => {
                // Enter/Tab/Backspace: legacy bytes unless ctrl is active (disambiguated)
                match named {
                    NamedKey::Enter => {
                        if ctrl {
                            return InputAction::Write(kitty::encode_key_event(
                                kitty::keycodes::ENTER, mod_val, event_type,
                            ));
                        }
                        return InputAction::Write(b"\r".to_vec());
                    }
                    NamedKey::Tab => {
                        if ctrl {
                            return InputAction::Write(kitty::encode_key_event(
                                kitty::keycodes::TAB, mod_val, event_type,
                            ));
                        }
                        if modifiers.shift_key() {
                            return InputAction::Write(kitty::encode_key_event(
                                kitty::keycodes::TAB, mod_val, event_type,
                            ));
                        }
                        return InputAction::Write(b"\t".to_vec());
                    }
                    NamedKey::Backspace => {
                        if ctrl {
                            return InputAction::Write(kitty::encode_key_event(
                                kitty::keycodes::BACKSPACE, mod_val, event_type,
                            ));
                        }
                        return InputAction::Write(b"\x7f".to_vec());
                    }
                    NamedKey::Escape => {
                        if ctrl || alt {
                            return InputAction::Write(kitty::encode_key_event(
                                kitty::keycodes::ESCAPE, mod_val, event_type,
                            ));
                        }
                        return InputAction::Write(b"\x1b".to_vec());
                    }
                    NamedKey::Space => {
                        if ctrl {
                            return InputAction::Write(kitty::encode_key_event(
                                32, mod_val, event_type,
                            ));
                        }
                        if alt {
                            return InputAction::Write(kitty::encode_key_event(
                                32, mod_val, event_type,
                            ));
                        }
                        return InputAction::Write(b" ".to_vec());
                    }
                    _ => {
                        let keycode = named_to_kitty_keycode(named);
                        if keycode > 0 {
                            return InputAction::Write(kitty::encode_key_event(
                                keycode, mod_val, event_type,
                            ));
                        }
                    }
                }
                InputAction::Ignore
            }
            Key::Character(c) => {
                if let Some(ch) = c.chars().next() {
                    // Ctrl+letter combos: disambiguate as CSI codepoint;5u
                    if ctrl && !modifiers.shift_key() {
                        let lower = ch.to_ascii_lowercase();
                        return InputAction::Write(kitty::encode_key_event(
                            lower as u32, mod_val, event_type,
                        ));
                    }
                    // Alt+letter or other modified chars: encode as CSI codepoint;mods u
                    if alt || ctrl || modifiers.super_key() {
                        let lower = ch.to_ascii_lowercase();
                        return InputAction::Write(kitty::encode_key_event(
                            lower as u32, mod_val, event_type,
                        ));
                    }
                    // Plain text key: send as normal bytes
                    if let Some(text) = event.text.as_deref() {
                        if !text.is_empty() {
                            return InputAction::Write(text.as_bytes().to_vec());
                        }
                    }
                    return InputAction::Write(c.as_bytes().to_vec());
                }
                InputAction::Ignore
            }
            _ => InputAction::Ignore,
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

/// Map a winit NamedKey to a Kitty functional key code.
/// Returns 0 if the key has no kitty mapping.
fn named_to_kitty_keycode(named: &NamedKey) -> u32 {
    use kitty::keycodes;
    match named {
        NamedKey::Enter => keycodes::ENTER,
        NamedKey::Tab => keycodes::TAB,
        NamedKey::Backspace => keycodes::BACKSPACE,
        NamedKey::Escape => keycodes::ESCAPE,
        NamedKey::Insert => keycodes::INSERT,
        NamedKey::Delete => keycodes::DELETE,
        NamedKey::ArrowLeft => keycodes::LEFT,
        NamedKey::ArrowRight => keycodes::RIGHT,
        NamedKey::ArrowUp => keycodes::UP,
        NamedKey::ArrowDown => keycodes::DOWN,
        NamedKey::PageUp => keycodes::PAGE_UP,
        NamedKey::PageDown => keycodes::PAGE_DOWN,
        NamedKey::Home => keycodes::HOME,
        NamedKey::End => keycodes::END,
        NamedKey::F1 => keycodes::F1,
        NamedKey::F2 => keycodes::F2,
        NamedKey::F3 => keycodes::F3,
        NamedKey::F4 => keycodes::F4,
        NamedKey::F5 => keycodes::F5,
        NamedKey::F6 => keycodes::F6,
        NamedKey::F7 => keycodes::F7,
        NamedKey::F8 => keycodes::F8,
        NamedKey::F9 => keycodes::F9,
        NamedKey::F10 => keycodes::F10,
        NamedKey::F11 => keycodes::F11,
        NamedKey::F12 => keycodes::F12,
        NamedKey::CapsLock => keycodes::CAPS_LOCK,
        NamedKey::ScrollLock => keycodes::SCROLL_LOCK,
        NamedKey::NumLock => keycodes::NUM_LOCK,
        NamedKey::PrintScreen => keycodes::PRINT_SCREEN,
        NamedKey::Pause => keycodes::PAUSE,
        NamedKey::ContextMenu => keycodes::MENU,
        _ => 0,
    }
}
