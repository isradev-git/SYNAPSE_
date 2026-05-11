// Kitty Keyboard Protocol — progressive enhancement for terminal key handling
//
// Spec: https://sw.kovidgoyal.net/kitty/keyboard-protocol/
//
// Flags (bitfield):
//   1 = Disambiguate escape codes
//   2 = Report event types (press/repeat/release)
//   4 = Report alternate keys
//   8 = Report all keys as escape codes
//   16 = Report associated text
//
// Modifier bits (encoded as 1 + bitfield in CSI):
//   shift=1, alt=2, ctrl=4, super=8, hyper=16, meta=32, caps_lock=64, num_lock=128

pub const KITTY_DISAMBIGUATE: u8 = 1;
pub const KITTY_REPORT_EVENTS: u8 = 2;
pub const KITTY_REPORT_ALTERNATE: u8 = 4;
pub const KITTY_REPORT_ALL: u8 = 8;
pub const KITTY_REPORT_ASSOCIATED: u8 = 16;

const STACK_MAX: usize = 8;

#[derive(Debug, Clone, Default)]
pub struct KittyKeyboard {
    pub flags: u8,
    stack: Vec<u8>,
}

impl KittyKeyboard {
    pub fn new() -> Self {
        Self {
            flags: 0,
            stack: Vec::with_capacity(STACK_MAX),
        }
    }

    pub fn is_active(&self) -> bool {
        self.flags != 0
    }

    pub fn is_disambiguate(&self) -> bool {
        self.flags & KITTY_DISAMBIGUATE != 0
    }

    pub fn is_report_events(&self) -> bool {
        self.flags & KITTY_REPORT_EVENTS != 0
    }

    pub fn is_report_all(&self) -> bool {
        self.flags & KITTY_REPORT_ALL != 0
    }

    /// Push current flags and set new flags. Returns true on success.
    pub fn push(&mut self, flags: u8) {
        if self.stack.len() >= STACK_MAX {
            self.stack.remove(0);
        }
        self.stack.push(self.flags);
        self.flags = flags;
    }

    /// Pop n entries from the stack. If stack is emptied, reset to 0.
    pub fn pop(&mut self, n: usize) {
        for _ in 0..n {
            match self.stack.pop() {
                Some(prev) => self.flags = prev,
                None => {
                    self.flags = 0;
                    break;
                }
            }
        }
    }

    /// Set flags with mode:
    ///   1 (default) = set all bits as given, clear unset bits
    ///   2 = set given bits, leave unset bits unchanged
    ///   3 = clear given bits, leave unset bits unchanged
    pub fn set_flags(&mut self, flags: u8, mode: u8) {
        match mode {
            2 => self.flags |= flags,
            3 => self.flags &= !flags,
            _ => self.flags = flags,
        }
    }

    pub fn reset(&mut self) {
        self.flags = 0;
        self.stack.clear();
    }
}

/// Encode modifier bits into the kitty modifier value (1 + bitmask).
pub fn encode_modifiers(shift: bool, alt: bool, ctrl: bool, super_key: bool) -> u16 {
    let mut bits: u16 = 0;
    if shift {
        bits |= 1;
    }
    if alt {
        bits |= 2;
    }
    if ctrl {
        bits |= 4;
    }
    if super_key {
        bits |= 8;
    }
    1 + bits
}

/// Functional key codes for keys that don't map to Unicode codepoints.
/// These are in the Unicode Private Use Area (57344-63743).
pub mod keycodes {
    pub const ESCAPE: u32 = 57344;
    pub const ENTER: u32 = 57345;
    pub const TAB: u32 = 57346;
    pub const BACKSPACE: u32 = 57347;
    pub const INSERT: u32 = 57348;
    pub const DELETE: u32 = 57349;
    pub const LEFT: u32 = 57350;
    pub const RIGHT: u32 = 57351;
    pub const UP: u32 = 57352;
    pub const DOWN: u32 = 57353;
    pub const PAGE_UP: u32 = 57354;
    pub const PAGE_DOWN: u32 = 57355;
    pub const HOME: u32 = 57356;
    pub const END: u32 = 57357;
    pub const CAPS_LOCK: u32 = 57358;
    pub const SCROLL_LOCK: u32 = 57359;
    pub const NUM_LOCK: u32 = 57360;
    pub const PRINT_SCREEN: u32 = 57361;
    pub const PAUSE: u32 = 57362;
    pub const MENU: u32 = 57363;
    pub const F1: u32 = 57364;
    pub const F2: u32 = 57365;
    pub const F3: u32 = 57366;
    pub const F4: u32 = 57367;
    pub const F5: u32 = 57368;
    pub const F6: u32 = 57369;
    pub const F7: u32 = 57370;
    pub const F8: u32 = 57371;
    pub const F9: u32 = 57372;
    pub const F10: u32 = 57373;
    pub const F11: u32 = 57374;
    pub const F12: u32 = 57375;
}

/// Build a kitty key event escape sequence.
///
/// `keycode` is the Unicode codepoint or functional key code.
/// `modifiers` is the encoded modifier value (1 + bitmask).
/// `event_type` is 1=press, 2=repeat, 3=release (only used when report_events is active).
pub fn encode_key_event(keycode: u32, modifiers: u16, event_type: Option<u8>) -> Vec<u8> {
    let mut buf = String::with_capacity(32);
    buf.push_str("\x1b[");

    use std::fmt::Write;
    write!(buf, "{}", keycode).unwrap();

    if let Some(et) = event_type {
        write!(buf, ";{}:{}", modifiers, et).unwrap();
    } else if modifiers != 1 {
        write!(buf, ";{}", modifiers).unwrap();
    }

    buf.push('u');
    buf.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kitty_keyboard_default() {
        let k = KittyKeyboard::default();
        assert_eq!(k.flags, 0);
        assert!(!k.is_active());
    }

    #[test]
    fn test_push_pop() {
        let mut k = KittyKeyboard::new();
        assert_eq!(k.flags, 0);

        k.push(KITTY_DISAMBIGUATE);
        assert_eq!(k.flags, KITTY_DISAMBIGUATE);
        assert!(k.is_disambiguate());

        k.push(KITTY_DISAMBIGUATE | KITTY_REPORT_EVENTS);
        assert_eq!(k.flags, KITTY_DISAMBIGUATE | KITTY_REPORT_EVENTS);

        k.pop(1);
        assert_eq!(k.flags, KITTY_DISAMBIGUATE);

        k.pop(1);
        assert_eq!(k.flags, 0);
        assert!(!k.is_active());
    }

    #[test]
    fn test_set_flags() {
        let mut k = KittyKeyboard::new();

        k.set_flags(KITTY_DISAMBIGUATE, 1);
        assert_eq!(k.flags, KITTY_DISAMBIGUATE);

        k.set_flags(KITTY_REPORT_EVENTS, 2);
        assert_eq!(k.flags, KITTY_DISAMBIGUATE | KITTY_REPORT_EVENTS);

        k.set_flags(KITTY_DISAMBIGUATE, 3);
        assert_eq!(k.flags, KITTY_REPORT_EVENTS);
    }

    #[test]
    fn test_pop_empty_stack() {
        let mut k = KittyKeyboard::new();
        k.pop(1);
        assert_eq!(k.flags, 0);
    }

    #[test]
    fn test_encode_modifiers() {
        assert_eq!(encode_modifiers(false, false, false, false), 1); // base = 1
        assert_eq!(encode_modifiers(true, false, false, false), 2); // shift
        assert_eq!(encode_modifiers(false, false, true, false), 5); // ctrl = 4 + 1
        assert_eq!(encode_modifiers(true, false, true, false), 6); // shift + ctrl
        assert_eq!(encode_modifiers(false, true, false, false), 3); // alt
        assert_eq!(encode_modifiers(false, false, false, true), 9); // super
    }

    #[test]
    fn test_encode_key_event_press_no_mods() {
        let bytes = encode_key_event(97, 1, None); // 'a' press, no mods
        assert_eq!(bytes, b"\x1b[97u");
    }

    #[test]
    fn test_encode_key_event_press_with_ctrl() {
        let bytes = encode_key_event(99, 5, None); // 'c' with ctrl, mod_val=5
        assert_eq!(bytes, b"\x1b[99;5u");
    }

    #[test]
    fn test_encode_key_event_with_event_type() {
        let bytes = encode_key_event(13, 1, Some(3)); // Enter release
        assert_eq!(bytes, b"\x1b[13;1:3u");
    }

    #[test]
    fn test_encode_key_event_mods_and_event() {
        let bytes = encode_key_event(97, 6, Some(2)); // 'a' repeat, shift+ctrl
        assert_eq!(bytes, b"\x1b[97;6:2u");
    }

    #[test]
    fn test_reset() {
        let mut k = KittyKeyboard::new();
        k.push(KITTY_DISAMBIGUATE | KITTY_REPORT_ALL);
        assert!(k.is_active());
        k.reset();
        assert_eq!(k.flags, 0);
        assert!(!k.is_active());
    }
}
