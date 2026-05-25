/// Kitty image protocol implementation (APC-based, https://sw.kovidgoyal.net/kitty/graphics-protocol/).
/// Supports: a=T (transmit), a=p (put), a=d (delete), format f=32 (RGBA) and f=100 (PNG).
use std::collections::HashMap;

use base64::Engine;

/// Reserved image ID for the per-pane background image.
pub const BG_IMAGE_ID: u32 = u32::MAX - 100;

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
    Rgba, // f=32
    Rgb, // f=24
    Png, // f=100
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
    /// Which pane this placement belongs to.
    pub pane_id: Option<synapse_ui::PaneId>,
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

    pub fn store_bg_image(&mut self, id: u32, width: u32, height: u32, rgba: Vec<u8>) {
        self.images.insert(
            id,
            StoredImage {
                id,
                width,
                height,
                rgba,
            },
        );
    }

    /// Accept a decoded Sixel image into the store and create a placement.
    #[allow(clippy::too_many_arguments)]
    pub fn accept_sixel(
        &mut self,
        id: u32,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
        col: usize,
        row: usize,
        pane_id: Option<synapse_ui::PaneId>,
    ) {
        self.images.insert(
            id,
            StoredImage {
                id,
                width,
                height,
                rgba,
            },
        );
        self.placements.push(ImagePlacement {
            image_id: id,
            col,
            row,
            columns: 0,
            rows: 0,
            pane_id,
        });
    }

    /// Accept a decoded iTerm2 OSC 1337 inline image into the store and create a placement.
    #[allow(clippy::too_many_arguments)]
    pub fn accept_iterm2(
        &mut self,
        id: u32,
        width: u32,
        height: u32,
        rgba: &[u8],
        col: usize,
        row: usize,
        pane_id: Option<synapse_ui::PaneId>,
    ) {
        self.images.insert(
            id,
            StoredImage {
                id,
                width,
                height,
                rgba: rgba.to_vec(),
            },
        );
        self.placements.push(ImagePlacement {
            image_id: id,
            col,
            row,
            columns: 0,
            rows: 0,
            pane_id,
        });
    }

    /// Process a parsed APC command with optional pane context.
    pub fn process(&mut self, cmd: ApcCommand, pane_id: Option<synapse_ui::PaneId>) {
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
            KittyFormat::Png => match decode_png(&raw_bytes) {
                Some(v) => v,
                None => return,
            },
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

        self.images.insert(
            id,
            StoredImage {
                id,
                width: w,
                height: h,
                rgba,
            },
        );

        if matches!(cmd.action, KittyAction::TransmitAndPut | KittyAction::Put) {
            self.placements.push(ImagePlacement {
                image_id: id,
                col: cmd.col,
                row: cmd.row,
                columns: cmd.columns,
                rows: cmd.rows,
                pane_id,
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
    let mut col: usize = 0;
    let mut row: usize = 0;
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
            "C" => col = val.parse().unwrap_or(0),
            "R" => row = val.parse().unwrap_or(0),
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
        col,
        row,
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

// ─── DECRQM scanner ──────────────────────────────────────────────────────────

/// Scan raw PTY bytes for DEC private mode requests (`CSI ? Ps $ p`).
/// Returns a vec of mode numbers to respond to.
pub fn scan_decrqm(bytes: &[u8]) -> Vec<u16> {
    let mut results = Vec::new();
    let mut i = 0;
    while i + 3 < bytes.len() {
        if bytes[i] == 0x1b && bytes[i + 1] == b'[' && bytes[i + 2] == b'?' {
            let rest = &bytes[i + 3..];
            let mut j = 0;
            while j < rest.len() && rest[j].is_ascii_digit() {
                j += 1;
            }
            if j > 0 && j + 1 < rest.len() && rest[j] == b'$' && rest[j + 1] == b'p' {
                if let Ok(s) = std::str::from_utf8(&rest[..j]) {
                    if let Ok(mode_num) = s.parse::<u16>() {
                        results.push(mode_num);
                    }
                }
                i += 3 + j + 2;
                continue;
            }
        }
        i += 1;
    }
    results
}

// ─── Sixel sequence detection ────────────────────────────────────────────────

/// Extract complete Sixel DCS sequences from a byte buffer.
/// Sixel format: ESC P <params> q <data> ST  (where ST is ESC \ or 0x9c).
/// This finds them and strips the full DCS from staging.
/// Returns (extracted data bytes, filtered staging).
pub fn process_sixel_sequences(staging: &mut Vec<u8>) -> Vec<Vec<u8>> {
    let mut results = Vec::new();
    let bytes = staging.as_slice();
    let mut filtered = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let mut had_sixel = false;

    while i < bytes.len() {
        if i + 2 < bytes.len() && bytes[i] == 0x1b && bytes[i + 1] == 0x50 {
            let dcs_start = i;
            // Look for the final character 'q' of the DCS introducer.
            let mut j = i + 2;
            let mut found_q = false;
            while j < bytes.len() && (j - dcs_start) <= 256 {
                if bytes[j] == b'q' {
                    found_q = true;
                    break;
                }
                // Allow digits, semicolons, space, colon, '<', '>', '$' in DCS params.
                if !bytes[j].is_ascii_digit()
                    && bytes[j] != b';'
                    && bytes[j] != b' '
                    && bytes[j] != b':'
                    && bytes[j] != b'<'
                    && bytes[j] != b'>'
                    && bytes[j] != b'$'
                {
                    break;
                }
                j += 1;
            }

            if !found_q {
                // Not a Sixel DCS, keep the bytes.
                filtered.extend_from_slice(&bytes[dcs_start..=i.min(bytes.len() - 1)]);
                i += 1;
                continue;
            }

            let data_start = j + 1;
            // Find ST terminator: ESC \ or 0x9c.
            let mut k = data_start;
            let terminator_found = loop {
                if k >= bytes.len() {
                    break false;
                }
                if bytes[k] == 0x1b && k + 1 < bytes.len() && bytes[k + 1] == 0x5c {
                    break true;
                }
                if bytes[k] == 0x9c {
                    break true;
                }
                k += 1;
            };

            if !terminator_found {
                // Incomplete sequence, keep as-is (may complete next iteration).
                filtered.extend_from_slice(&bytes[dcs_start..]);
                break;
            }

            let data_end = k;
            let sixel_data = bytes[data_start..data_end].to_vec();
            results.push(sixel_data);
            had_sixel = true;

            // Skip past ST
            if bytes.get(data_end) == Some(&0x1b) {
                i = data_end + 2;
            } else {
                i = data_end + 1;
            }
        } else {
            filtered.push(bytes[i]);
            i += 1;
        }
    }

    if had_sixel {
        *staging = filtered;
    }
    results
}

// ─── iTerm2 OSC 1337 inline image detection ──────────────────────────────────

/// A parsed iTerm2 OSC 1337 inline image command.
#[derive(Debug, Clone)]
pub struct Iterm2Image {
    /// Raw base64-encoded image data.
    pub data: String,
    /// Display width in pixels (from `width=` attribute, 0 = auto).
    pub display_width: u32,
    /// Display height in pixels (from `height=` attribute, 0 = auto).
    pub display_height: u32,
    /// Whether to preserve aspect ratio.
    pub preserve_aspect_ratio: bool,
}

/// Extract complete iTerm2 OSC 1337 inline image sequences from staging.
/// Format: `ESC ] 1337 ; File=inline=1 ; key=val : base64data BEL|ST`
/// Strips matched sequences from staging. Returns parsed images.
pub fn process_iterm2_sequences(staging: &mut Vec<u8>) -> Vec<Iterm2Image> {
    let mut results = Vec::new();
    let bytes = staging.as_slice();
    let mut filtered = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let mut had_iterm2 = false;

    while i < bytes.len() {
        // Look for ESC ] 1337
        if i + 8 < bytes.len() && bytes[i] == 0x1b && bytes[i + 1] == b']' {
            let rest = &bytes[i + 2..];
            if rest.starts_with(b"1337") && rest.len() > 4 && (rest[4] == b';' || rest[4] == b' ') {
                let seq_start = i;
                // Find the colon that separates params from base64 data.
                let mut colon_pos = None;
                let mut j = 5; // after "1337"
                while j < rest.len() && j <= 4096 {
                    if rest[j] == b':' {
                        colon_pos = Some(j);
                        break;
                    }
                    // Terminator before colon → malformed, keep bytes.
                    if rest[j] == 0x07
                        || (rest[j] == 0x1b && j + 1 < rest.len() && rest[j + 1] == b'\\')
                        || rest[j] == 0x9c
                    {
                        break;
                    }
                    j += 1;
                }

                let colon = match colon_pos {
                    Some(c) => c,
                    None => {
                        filtered.push(bytes[i]);
                        i += 1;
                        continue;
                    }
                };

                let params_bytes = &rest[..colon];

                // Find terminator after data: BEL (0x07), ST (ESC \), or 0x9c.
                let data_start = colon + 1;
                let mut k = data_start;
                let terminator_found = loop {
                    if k >= rest.len() {
                        break false;
                    }
                    if rest[k] == 0x07 {
                        break true;
                    }
                    if rest[k] == 0x1b && k + 1 < rest.len() && rest[k + 1] == b'\\' {
                        break true;
                    }
                    if rest[k] == 0x9c {
                        break true;
                    }
                    k += 1;
                };

                if !terminator_found {
                    // Incomplete, keep as-is.
                    filtered.extend_from_slice(&bytes[seq_start..]);
                    break;
                }

                let data_bytes = &rest[data_start..k];
                let terminator_len = if rest[k] == 0x1b { 2 } else { 1 };

                // Check if this is an inline image: must contain "inline=1".
                let is_inline = params_bytes.windows(9).any(|w| w == b"inline=1;")
                    || params_bytes.windows(9).any(|w| w == b"inline=1:")
                    || params_bytes.windows(9).any(|w| w == b"inline=1\t")
                    || params_bytes.ends_with(b"inline=1");

                if !is_inline {
                    // Not an inline image; keep bytes.
                    filtered.extend_from_slice(
                        &bytes[seq_start..=seq_start + 2 + k.min(rest.len() - 1)],
                    );
                    i = seq_start + 2 + k + terminator_len;
                    continue;
                }

                // Parse params: display_width, display_height, preserveAspectRatio.
                let mut display_width: u32 = 0;
                let mut display_height: u32 = 0;
                let mut preserve_aspect_ratio = false;

                if let Ok(params_str) = std::str::from_utf8(params_bytes) {
                    for kv in params_str.split(';') {
                        let kv = kv.trim();
                        let mut parts = kv.splitn(2, '=');
                        let key = parts.next().unwrap_or("").trim();
                        let val = parts.next().unwrap_or("").trim();
                        match key {
                            "width" => display_width = val.parse().unwrap_or(0),
                            "height" => display_height = val.parse().unwrap_or(0),
                            "preserveAspectRatio" => {
                                preserve_aspect_ratio = val == "1" || val == "true"
                            }
                            _ => {}
                        }
                    }
                }

                if let Ok(data_str) = std::str::from_utf8(data_bytes) {
                    results.push(Iterm2Image {
                        data: data_str.trim_end().to_string(),
                        display_width,
                        display_height,
                        preserve_aspect_ratio,
                    });
                    had_iterm2 = true;
                }

                i = seq_start + 2 + k + terminator_len;
            } else {
                filtered.push(bytes[i]);
                i += 1;
            }
        } else {
            filtered.push(bytes[i]);
            i += 1;
        }
    }

    if had_iterm2 {
        *staging = filtered;
    }
    results
}

// ─── Image decoding (multi-format) ───────────────────────────────────────────

/// Decode raw image bytes (PNG, JPEG, GIF, BMP) to RGBA using the `image` crate.
/// Returns (width, height, RGBA bytes).
pub fn decode_image_bytes(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    use image::GenericImageView;
    let img = image::load_from_memory(bytes).ok()?;
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8().into_raw();
    Some((w, h, rgba))
}

/// Load a background image from a file path, apply opacity to alpha.
pub fn load_background_image(path: &str, opacity: f32) -> Option<(u32, u32, Vec<u8>)> {
    let bytes = std::fs::read(path).ok()?;
    use image::GenericImageView;
    let img = image::load_from_memory(&bytes).ok()?;
    let (w, h) = img.dimensions();
    let mut rgba = img.to_rgba8().into_raw();
    if (opacity - 1.0).abs() > f32::EPSILON {
        let alpha_u8 = (opacity.clamp(0.0, 1.0) * 255.0) as u8;
        for alpha in rgba.chunks_exact_mut(4).map(|c| &mut c[3]) {
            *alpha = ((*alpha as u16 * alpha_u8 as u16) / 255) as u8;
        }
    }
    Some((w, h, rgba))
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
        let cmd =
            parse_apc("Ga=T,f=32,i=1,s=2,v=2;AAAAAAAAAAAAAAAAAAAAAAAAAA==").expect("parse failed");
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
    fn parse_apc_with_placement_coords() {
        let cmd =
            parse_apc("Ga=T,f=32,i=1,s=2,v=2,c=4,r=3,C=10,R=5;AAAAAAAA").expect("parse failed");
        assert_eq!(cmd.action, KittyAction::Transmit);
        assert_eq!(cmd.columns, 4);
        assert_eq!(cmd.rows, 3);
        assert_eq!(cmd.col, 10);
        assert_eq!(cmd.row, 5);
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
        store.images.insert(
            1,
            StoredImage {
                id: 1,
                width: 1,
                height: 1,
                rgba: vec![0; 4],
            },
        );
        store.placements.push(ImagePlacement {
            image_id: 1,
            col: 0,
            row: 0,
            columns: 1,
            rows: 1,
            pane_id: None,
        });
        store.process(
            ApcCommand {
                action: KittyAction::Delete,
                format: KittyFormat::Rgba,
                image_id: 0,
                width: 0,
                height: 0,
                columns: 0,
                rows: 0,
                col: 0,
                row: 0,
                data: String::new(),
                more: false,
            },
            None,
        );
        assert!(store.placements.is_empty());
    }

    #[test]
    fn image_store_delete_by_id() {
        let mut store = ImageStore::new();
        store.images.insert(
            2,
            StoredImage {
                id: 2,
                width: 1,
                height: 1,
                rgba: vec![0; 4],
            },
        );
        store.placements.push(ImagePlacement {
            image_id: 2,
            col: 0,
            row: 0,
            columns: 1,
            rows: 1,
            pane_id: None,
        });
        store.process(
            ApcCommand {
                action: KittyAction::Delete,
                format: KittyFormat::Rgba,
                image_id: 2,
                width: 0,
                height: 0,
                columns: 0,
                rows: 0,
                col: 0,
                row: 0,
                data: String::new(),
                more: false,
            },
            None,
        );
        assert!(!store.images.contains_key(&2));
        assert!(store.placements.is_empty());
    }

    // ─── iTerm2 OSC 1337 tests ────────────────────────────────────────────

    #[test]
    fn iterm2_basic_inline_bel() {
        let mut staging = b"\x1b]1337;File=inline=1:QUFB\x07".to_vec();
        let imgs = process_iterm2_sequences(&mut staging);
        assert_eq!(imgs.len(), 1);
        assert_eq!(imgs[0].data, "QUFB");
        assert_eq!(imgs[0].display_width, 0);
        assert_eq!(imgs[0].display_height, 0);
        assert!(!imgs[0].preserve_aspect_ratio);
        assert!(staging.is_empty());
    }

    #[test]
    fn iterm2_st_terminator() {
        let mut staging = b"\x1b]1337;File=inline=1:QUFB\x1b\\".to_vec();
        let imgs = process_iterm2_sequences(&mut staging);
        assert_eq!(imgs.len(), 1);
        assert_eq!(imgs[0].data, "QUFB");
        assert!(staging.is_empty());
    }

    #[test]
    fn iterm2_with_dimensions() {
        let mut staging = b"\x1b]1337;File=inline=1;width=800;height=600:QUFB\x07".to_vec();
        let imgs = process_iterm2_sequences(&mut staging);
        assert_eq!(imgs.len(), 1);
        assert_eq!(imgs[0].data, "QUFB");
        assert_eq!(imgs[0].display_width, 800);
        assert_eq!(imgs[0].display_height, 600);
    }

    #[test]
    fn iterm2_preserve_aspect_ratio() {
        let mut staging = b"\x1b]1337;File=inline=1;preserveAspectRatio=1:QUFB\x07".to_vec();
        let imgs = process_iterm2_sequences(&mut staging);
        assert_eq!(imgs.len(), 1);
        assert!(imgs[0].preserve_aspect_ratio);
    }

    #[test]
    fn iterm2_non_inline_ignored() {
        let mut staging = b"\x1b]1337;File=something;size=100:QUFB\x07extra\x07".to_vec();
        let _imgs = process_iterm2_sequences(&mut staging);
        // Non-inline sequences are preserved in staging (not stripped).
        assert!(!staging.is_empty());
    }

    #[test]
    fn iterm2_multiple_images() {
        let mut staging =
            b"\x1b]1337;File=inline=1:QUFB\x07\x1b]1337;File=inline=1:QkJC\x07".to_vec();
        let imgs = process_iterm2_sequences(&mut staging);
        assert_eq!(imgs.len(), 2);
        assert_eq!(imgs[0].data, "QUFB");
        assert_eq!(imgs[1].data, "QkJC");
        assert!(staging.is_empty());
    }

    #[test]
    fn iterm2_incomplete_ignored() {
        let mut staging = b"\x1b]1337;File=inline=1:INCOMPLETE".to_vec();
        let imgs = process_iterm2_sequences(&mut staging);
        assert!(imgs.is_empty());
        assert!(!staging.is_empty());
    }

    #[test]
    fn iterm2_stripping_preserves_surrounding() {
        let mut staging = b"hello\x1b]1337;File=inline=1:QUFB\x07world".to_vec();
        let imgs = process_iterm2_sequences(&mut staging);
        assert_eq!(imgs.len(), 1);
        assert_eq!(staging, b"helloworld");
    }

    #[test]
    fn accept_iterm2_creates_entry() {
        let mut store = ImageStore::new();
        store.accept_iterm2(42, 100, 50, &[0u8; 20000], 10, 5, None);
        assert!(store.images.contains_key(&42));
        assert_eq!(store.placements.len(), 1);
        assert_eq!(store.placements[0].image_id, 42);
        assert_eq!(store.placements[0].col, 10);
        assert_eq!(store.placements[0].row, 5);
    }

    // ─── scan_decrqm tests ────────────────────────────────────────────────────

    #[test]
    fn scan_decrqm_single_mode() {
        // CSI ? 25 $ p → mode 25 (DECTCEM)
        let data = b"\x1b[?25$p";
        let modes = scan_decrqm(data);
        assert_eq!(modes, vec![25]);
    }

    #[test]
    fn scan_decrqm_multiple() {
        let mut data = Vec::new();
        data.extend_from_slice(b"\x1b[?1$p");
        data.extend_from_slice(b"\x1b[?2004$p");
        let modes = scan_decrqm(&data);
        assert_eq!(modes, vec![1, 2004]);
    }

    #[test]
    fn scan_decrqm_no_match_kkp_query() {
        // KKP query ESC [ ? u — should NOT match DECRQM (no $ p suffix)
        let data = b"\x1b[?u";
        let modes = scan_decrqm(data);
        assert!(modes.is_empty());
    }

    #[test]
    fn scan_decrqm_no_match_plain_text() {
        let modes = scan_decrqm(b"hello world");
        assert!(modes.is_empty());
    }

    #[test]
    fn decode_image_bytes_smoke() {
        // Generate a valid 1x1 black RGBA PNG using the png crate.
        let mut png_buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png_buf, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            let pixel: [u8; 4] = [255, 0, 0, 255]; // red, opaque
            writer.write_image_data(&pixel).unwrap();
        }
        let (w, h, rgba) = decode_image_bytes(&png_buf).expect("should decode generated PNG");
        assert_eq!(w, 1);
        assert_eq!(h, 1);
        assert_eq!(rgba.len(), 4);
        assert_eq!(rgba, vec![255, 0, 0, 255]);
    }
}
