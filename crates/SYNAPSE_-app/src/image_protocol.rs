/// Kitty image protocol implementation (APC-based, https://sw.kovidgoyal.net/kitty/graphics-protocol/).
/// Supports: a=T (transmit), a=p (put), a=d (delete), format f=32 (RGBA) and f=100 (PNG).
use std::collections::HashMap;

use base64::Engine;

// ─── Public types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default)]
pub enum KittyAction {
    #[default]
    Transmit,
    TransmitAndPut,
    Query,
    Put,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum KittyFormat {
    #[default]
    Rgba,   // f=32
    Rgb,    // f=24
    Png,    // f=100
}

/// A fully decoded Kitty image stored in the image store.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct StoredImage {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    /// Raw RGBA bytes (width * height * 4).
    pub rgba: Vec<u8>,
}

/// A placement of an image in the terminal grid.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ImagePlacement {
    pub image_id: u32,
    /// Terminal column where the image starts.
    pub col: usize,
    /// Terminal row where the image starts.
    pub row: usize,
    /// How many columns the image spans (0 = auto from image width).
    pub columns: u32,
    /// How many rows the image spans (0 = auto from image height).
    pub rows: u32,
}

/// A command parsed from an APC sequence, as sent from the render thread.
#[derive(Debug)]
pub struct ApcCommand {
    pub action: KittyAction,
    pub format: KittyFormat,
    pub image_id: u32,
    pub width: u32,
    pub height: u32,
    pub columns: u32,
    pub rows: u32,
    pub col: usize,
    pub row: usize,
    /// Base64-encoded image data (may span multiple chunks with m=1).
    pub data: String,
    /// Whether more data chunks follow (m=1).
    pub more: bool,
}

// ─── ImageStore ──────────────────────────────────────────────────────────────

pub struct ImageStore {
    pub images: HashMap<u32, StoredImage>,
    pub placements: Vec<ImagePlacement>,
    /// Accumulate base64 chunks for chunked transmits.
    pending: HashMap<u32, String>,
}

impl ImageStore {
    pub fn new() -> Self {
        Self {
            images: HashMap::new(),
            placements: Vec::new(),
            pending: HashMap::new(),
        }
    }

    /// Process a parsed APC command.
    pub fn process(&mut self, cmd: ApcCommand) {
        match cmd.action {
            KittyAction::Delete => {
                if cmd.image_id == 0 {
                    self.placements.clear();
                } else {
                    self.images.remove(&cmd.image_id);
                    self.placements.retain(|p| p.image_id != cmd.image_id);
                }
                return;
            }
            KittyAction::Query => return,
            _ => {}
        }

        let id = if cmd.image_id == 0 { 1 } else { cmd.image_id };

        // Accumulate chunked data.
        let entry = self.pending.entry(id).or_default();
        entry.push_str(&cmd.data);

        if cmd.more {
            return; // wait for more chunks
        }

        let b64 = self.pending.remove(&id).unwrap_or_default();
        let raw_bytes = match base64::engine::general_purpose::STANDARD.decode(&b64) {
            Ok(b) => b,
            Err(_) => return,
        };

        let (w, h, rgba) = match cmd.format {
            KittyFormat::Png => {
                match decode_png(&raw_bytes) {
                    Some(v) => v,
                    None => return,
                }
            }
            KittyFormat::Rgba => {
                let w = cmd.width;
                let h = cmd.height;
                if w == 0 || h == 0 || raw_bytes.len() < (w * h * 4) as usize {
                    return;
                }
                (w, h, raw_bytes[..(w * h * 4) as usize].to_vec())
            }
            KittyFormat::Rgb => {
                let w = cmd.width;
                let h = cmd.height;
                if w == 0 || h == 0 || raw_bytes.len() < (w * h * 3) as usize {
                    return;
                }
                let mut rgba = Vec::with_capacity((w * h * 4) as usize);
                for chunk in raw_bytes[..(w * h * 3) as usize].chunks(3) {
                    rgba.extend_from_slice(chunk);
                    rgba.push(255);
                }
                (w, h, rgba)
            }
        };

        self.images.insert(id, StoredImage { id, width: w, height: h, rgba });

        if matches!(cmd.action, KittyAction::TransmitAndPut | KittyAction::Put) {
            self.placements.push(ImagePlacement {
                image_id: id,
                col: cmd.col,
                row: cmd.row,
                columns: cmd.columns,
                rows: cmd.rows,
            });
        }
    }
}

// ─── APC sequence extraction ─────────────────────────────────────────────────

/// Extract complete APC sequences (`\x1b_G...\x1b\\`) from a raw PTY byte slice.
/// Returns the inner content of each complete APC sequence (everything between
/// `\x1b_` and the ST terminator `\x1b\\`).
pub fn extract_apc_sequences(bytes: &[u8]) -> Vec<String> {
    let mut result = Vec::new();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == 0x1b && bytes[i + 1] == b'_' {
            let start = i + 2;
            let mut j = start;
            while j < bytes.len() {
                // Also accept BEL (0x07) as ST terminator.
                if bytes[j] == 0x07 {
                    if let Ok(s) = std::str::from_utf8(&bytes[start..j]) {
                        result.push(s.to_string());
                    }
                    i = j + 1;
                    break;
                }
                if j + 1 < bytes.len() && bytes[j] == 0x1b && bytes[j + 1] == b'\\' {
                    if let Ok(s) = std::str::from_utf8(&bytes[start..j]) {
                        result.push(s.to_string());
                    }
                    i = j + 2;
                    break;
                }
                j += 1;
            }
            if j >= bytes.len() {
                break;
            }
        } else {
            i += 1;
        }
    }
    result
}

/// Parse an APC inner string into an `ApcCommand`.
/// Kitty format: `G<key>=<val>,...;<base64data>` where the `G` prefix is optional.
pub fn parse_apc(s: &str) -> Option<ApcCommand> {
    // Must start with 'G' for Kitty graphics protocol.
    let s = s.strip_prefix('G')?;

    let (params_str, data) = if let Some(semi) = s.find(';') {
        (&s[..semi], s[semi + 1..].to_string())
    } else {
        (s, String::new())
    };

    let mut action = KittyAction::Transmit;
    let mut format = KittyFormat::Rgba;
    let mut image_id: u32 = 0;
    let mut width: u32 = 0;
    let mut height: u32 = 0;
    let mut columns: u32 = 0;
    let mut rows: u32 = 0;
    let mut more = false;

    for kv in params_str.split(',') {
        let mut parts = kv.splitn(2, '=');
        let key = parts.next()?.trim();
        let val = parts.next().unwrap_or("").trim();
        match key {
            "a" => {
                action = match val {
                    "T" => KittyAction::Transmit,
                    "p" => KittyAction::Put,
                    "t" => KittyAction::TransmitAndPut,
                    "q" => KittyAction::Query,
                    "d" => KittyAction::Delete,
                    _ => KittyAction::Transmit,
                }
            }
            "f" => {
                format = match val {
                    "24" => KittyFormat::Rgb,
                    "32" => KittyFormat::Rgba,
                    "100" => KittyFormat::Png,
                    _ => KittyFormat::Rgba,
                }
            }
            "i" => image_id = val.parse().unwrap_or(0),
            "s" => width = val.parse().unwrap_or(0),
            "v" => height = val.parse().unwrap_or(0),
            "c" => columns = val.parse().unwrap_or(0),
            "r" => rows = val.parse().unwrap_or(0),
            "m" => more = val == "1",
            _ => {}
        }
    }

    Some(ApcCommand {
        action,
        format,
        image_id,
        width,
        height,
        columns,
        rows,
        col: 0,
        row: 0,
        data,
        more,
    })
}

// ─── KKP CSI scanner ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum KkpScan {
    /// App queried KKP support (`ESC [ ? u`).
    Query,
    /// App pushed flags (`ESC [ = N u`).
    Push(u8),
    /// App popped flags (`ESC [ < N > u` with no `=` prefix).
    Pop,
}

/// Scan raw PTY bytes for Kitty keyboard protocol control sequences.
/// Returns a vec of detected KKP commands.  The caller is responsible for
/// responding to queries by writing `ESC [ ? 1 u` back to the PTY.
pub fn scan_kkp(bytes: &[u8]) -> Vec<KkpScan> {
    let mut results = Vec::new();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == 0x1b && bytes[i + 1] == b'[' {
            let rest = &bytes[i + 2..];

            // ESC [ ? u → query
            if rest.len() >= 2 && rest[0] == b'?' && rest[1] == b'u' {
                results.push(KkpScan::Query);
                i += 4;
                continue;
            }

            // ESC [ = N u → push flags
            if !rest.is_empty() && rest[0] == b'=' {
                let mut j = 1;
                while j < rest.len() && rest[j].is_ascii_digit() {
                    j += 1;
                }
                if j < rest.len() && rest[j] == b'u' && j > 1 {
                    let flags: u8 = std::str::from_utf8(&rest[1..j])
                        .ok()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    results.push(KkpScan::Push(flags));
                    i += 2 + j + 1;
                    continue;
                }
            }

            // ESC [ < N > u → pop (number followed by u, no = prefix, digits only)
            if !rest.is_empty() && rest[0].is_ascii_digit() {
                let mut j = 0;
                while j < rest.len() && rest[j].is_ascii_digit() {
                    j += 1;
                }
                if j < rest.len() && rest[j] == b'u' {
                    results.push(KkpScan::Pop);
                    i += 2 + j + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    results
}

// ─── Image decoding ──────────────────────────────────────────────────────────

fn decode_png(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let w = info.width;
    let h = info.height;
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => {
            let rgb = &buf[..info.buffer_size()];
            let mut out = Vec::with_capacity(w as usize * h as usize * 4);
            for chunk in rgb.chunks(3) {
                out.extend_from_slice(chunk);
                out.push(255);
            }
            out
        }
        png::ColorType::Grayscale => {
            let gray = &buf[..info.buffer_size()];
            let mut out = Vec::with_capacity(w as usize * h as usize * 4);
            for &g in gray {
                out.extend_from_slice(&[g, g, g, 255]);
            }
            out
        }
        png::ColorType::GrayscaleAlpha => {
            let ga = &buf[..info.buffer_size()];
            let mut out = Vec::with_capacity(w as usize * h as usize * 4);
            for chunk in ga.chunks(2) {
                let (g, a) = (chunk[0], chunk.get(1).copied().unwrap_or(255));
                out.extend_from_slice(&[g, g, g, a]);
            }
            out
        }
        _ => return None,
    };
    Some((w, h, rgba))
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_apc_simple() {
        let data = b"\x1b_Ga=T,f=32;AAAA\x1b\\";
        let seqs = extract_apc_sequences(data);
        assert_eq!(seqs, vec!["Ga=T,f=32;AAAA"]);
    }

    #[test]
    fn extract_apc_bel_terminator() {
        let data = b"\x1b_Ga=T;AAAA\x07";
        let seqs = extract_apc_sequences(data);
        assert_eq!(seqs, vec!["Ga=T;AAAA"]);
    }

    #[test]
    fn extract_apc_multiple() {
        let data = b"\x1b_Ga=T;AA\x1b\\\x1b_Ga=p;BB\x1b\\";
        let seqs = extract_apc_sequences(data);
        assert_eq!(seqs.len(), 2);
    }

    #[test]
    fn extract_apc_incomplete_ignored() {
        let data = b"\x1b_Ga=T;INCOMPLETE";
        let seqs = extract_apc_sequences(data);
        assert!(seqs.is_empty());
    }

    #[test]
    fn parse_apc_transmit() {
        let cmd = parse_apc("Ga=T,f=32,i=1,s=2,v=2;AAAAAAAAAAAAAAAAAAAAAAAAAA==")
            .expect("parse failed");
        assert_eq!(cmd.action, KittyAction::Transmit);
        assert_eq!(cmd.format, KittyFormat::Rgba);
        assert_eq!(cmd.image_id, 1);
        assert_eq!(cmd.width, 2);
        assert_eq!(cmd.height, 2);
        assert!(!cmd.data.is_empty());
    }

    #[test]
    fn parse_apc_delete() {
        let cmd = parse_apc("Ga=d,i=5;").expect("parse failed");
        assert_eq!(cmd.action, KittyAction::Delete);
        assert_eq!(cmd.image_id, 5);
    }

    #[test]
    fn parse_apc_png_format() {
        let cmd = parse_apc("Ga=T,f=100,i=2;AAAA").expect("parse failed");
        assert_eq!(cmd.format, KittyFormat::Png);
    }

    #[test]
    fn parse_apc_chunked() {
        let cmd = parse_apc("Ga=T,f=32,i=3,m=1;AAAA").expect("parse failed");
        assert!(cmd.more);
    }

    #[test]
    fn scan_kkp_query() {
        let data = b"\x1b[?u";
        let cmds = scan_kkp(data);
        assert!(matches!(cmds[0], KkpScan::Query));
    }

    #[test]
    fn scan_kkp_push() {
        let data = b"\x1b[=1u";
        let cmds = scan_kkp(data);
        assert!(matches!(cmds[0], KkpScan::Push(1)));
    }

    #[test]
    fn scan_kkp_push_flags_31() {
        let data = b"\x1b[=31u";
        let cmds = scan_kkp(data);
        assert!(matches!(cmds[0], KkpScan::Push(31)));
    }

    #[test]
    fn image_store_delete_all() {
        let mut store = ImageStore::new();
        store.images.insert(1, StoredImage { id: 1, width: 1, height: 1, rgba: vec![0; 4] });
        store.placements.push(ImagePlacement { image_id: 1, col: 0, row: 0, columns: 1, rows: 1 });
        store.process(ApcCommand {
            action: KittyAction::Delete,
            format: KittyFormat::Rgba,
            image_id: 0,
            width: 0, height: 0, columns: 0, rows: 0, col: 0, row: 0,
            data: String::new(),
            more: false,
        });
        assert!(store.placements.is_empty());
    }

    #[test]
    fn image_store_delete_by_id() {
        let mut store = ImageStore::new();
        store.images.insert(2, StoredImage { id: 2, width: 1, height: 1, rgba: vec![0; 4] });
        store.placements.push(ImagePlacement { image_id: 2, col: 0, row: 0, columns: 1, rows: 1 });
        store.process(ApcCommand {
            action: KittyAction::Delete,
            format: KittyFormat::Rgba,
            image_id: 2,
            width: 0, height: 0, columns: 0, rows: 0, col: 0, row: 0,
            data: String::new(),
            more: false,
        });
        assert!(!store.images.contains_key(&2));
        assert!(store.placements.is_empty());
    }
}
