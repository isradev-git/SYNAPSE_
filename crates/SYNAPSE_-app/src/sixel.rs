use std::collections::HashMap;

pub struct SixelResult {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

const SIXEL_MAX_WIDTH: u32 = 4096;
const SIXEL_MAX_HEIGHT: u32 = 4096;

#[derive(Default)]
pub struct SixelState {
    /// Maps register index (0-255) to RGBA color.
    color_registers: HashMap<u8, [u8; 4]>,
    current_color: [u8; 4],
    /// Pixels: (x, y) -> [r, g, b, a]. Sparse until finalization.
    bitmap: Vec<u8>,
    /// Current drawing cursor in sixel coordinates (not pixels).
    sixel_x: u32,
    sixel_y: u32,
    /// Maximum width/height so far (in sixel coordinates).
    max_x: u32,
    max_y: u32,
    /// Repeat count from last `!N`.
    repeat: u32,
    /// Whether we've had any sixel data (at least one sixel character drawn).
    had_data: bool,
    /// Reached max dimensions limit.
    overflow: bool,
}

impl SixelState {
    pub fn new() -> Self {
        Self {
            color_registers: HashMap::new(),
            current_color: [255, 255, 255, 255],
            bitmap: Vec::new(),
            sixel_x: 0,
            sixel_y: 0,
            max_x: 0,
            max_y: 0,
            repeat: 0,
            had_data: false,
            overflow: false,
        }
    }

    /// Process one byte of sixel data. Returns true if processing should continue.
    pub fn feed(&mut self, byte: u8) -> bool {
        if self.overflow {
            return false;
        }
        match byte {
            b'#' => {
                // Color definition or selection mark - handled on next byte in feed_param.
                // We don't do anything here; the caller already pushed the byte.
            }
            _ => self.feed_value(byte),
        }
        true
    }

    fn feed_value(&mut self, byte: u8) {
        match byte {
            b'!' => {
                // Start repeat count
            }
            b'0'..=b'9' => {
                // Could be part of repeat count or color register number after #
                // Handled by the caller via feed_param context.
            }
            b'$' => {
                // Carriage return: go to x=0, same sixel row
                let x_end = self.sixel_x;
                if x_end > self.max_x {
                    self.max_x = x_end;
                }
                self.sixel_x = 0;
            }
            b'-' => {
                // Next line: increment sixel_y by 1 (6 pixels), reset x
                let x_end = self.sixel_x;
                if x_end > self.max_x {
                    self.max_x = x_end;
                }
                self.sixel_x = 0;
                self.sixel_y += 1;
            }
            b'"' => {
                // Raster attributes: skip until next meaningful byte.
                // Actually we just skip the raster attributes, they're positional.
            }
            0x3f..=0x7e => {
                let sixel_val = byte - 0x3f;
                // Sixel character: 6 vertical pixels at current position
                if !self.had_data {
                    self.had_data = true;
                }
                let rep = if self.repeat > 0 { self.repeat } else { 1 };
                self.repeat = 0;
                for offset in 0..rep {
                    self.draw_sixel(self.sixel_x + offset, sixel_val);
                }
                self.sixel_x += rep;
                // Ensure dimensions cover the full sixel block even if no pixels were lit.
                let x_end = self.sixel_x;
                if x_end > self.max_x {
                    self.max_x = x_end;
                }
                let y_end = (self.sixel_y + 1) * 6;
                if y_end > self.max_y {
                    self.max_y = y_end;
                }
            }
            _ => {
                // Non-sixel characters are ignored (separators, etc.)
            }
        }
    }

    /// Process a parameter byte (after `#` or `!` or between `"` markers).
    /// This is a simplified approach - we parse inline in `decode`.
    pub fn define_color(&mut self, idx: u8, r: u8, g: u8, b: u8) {
        self.color_registers.insert(idx, [r, g, b, 255]);
    }

    pub fn select_color(&mut self, idx: u8) {
        self.current_color = self
            .color_registers
            .get(&idx)
            .copied()
            .unwrap_or([255, 255, 255, 255]);
    }

    fn draw_sixel(&mut self, x: u32, sixel_val: u8) {
        for bit in 0..6 {
            if (sixel_val & (1 << bit)) != 0 {
                let px = x;
                let py = self.sixel_y * 6 + bit;
                self.set_pixel(px, py);
            }
        }
    }

    fn set_pixel(&mut self, x: u32, y: u32) {
        if x >= SIXEL_MAX_WIDTH || y >= SIXEL_MAX_HEIGHT {
            self.overflow = true;
            return;
        }
        if x >= self.max_x {
            self.max_x = x + 1;
        }
        if y >= self.max_y {
            self.max_y = y + 1;
        }
        // Ensure bitmap is large enough.
        let needed = ((y * SIXEL_MAX_WIDTH + x) as usize + 1) * 4;
        if self.bitmap.len() < needed {
            self.bitmap.resize(needed, 0);
        }
        let idx = (y as usize * SIXEL_MAX_WIDTH as usize + x as usize) * 4;
        self.bitmap[idx..idx + 4].copy_from_slice(&self.current_color);
    }

    pub fn finalize(&self) -> Option<SixelResult> {
        if !self.had_data || self.overflow || self.max_x == 0 || self.max_y == 0 {
            return None;
        }
        let w = self.max_x;
        let h = self.max_y;
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let src_idx = (y as usize * SIXEL_MAX_WIDTH as usize + x as usize) * 4;
                let dst_idx = (y as usize * w as usize + x as usize) * 4;
                if src_idx + 4 <= self.bitmap.len() {
                    rgba[dst_idx..dst_idx + 4].copy_from_slice(&self.bitmap[src_idx..src_idx + 4]);
                }
            }
        }
        Some(SixelResult {
            width: w,
            height: h,
            rgba,
        })
    }
}

pub fn decode_sixel(data: &[u8]) -> Option<SixelResult> {
    let mut state = SixelState::new();
    let bytes = data;
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];

        match b {
            b'#' => {
                if i + 1 >= bytes.len() {
                    break;
                }
                i += 1;
                // Parse `#Pc` or `#Pc;Pu;Px;Py;Pz`
                let (idx_bytes, next_i) = parse_number(bytes, i);
                if idx_bytes.is_empty() {
                    continue;
                }
                let idx: u8 = match std::str::from_utf8(idx_bytes).ok()?.parse().ok() {
                    Some(v) => v,
                    None => {
                        i = next_i;
                        continue;
                    }
                };

                if next_i < bytes.len() && bytes[next_i] == b';' {
                    // Color definition: #Pc;Pu;Px;Py;Pz
                    let after_pc = next_i + 1;
                    let (pu_bytes, after_pu) = parse_number(bytes, after_pc);
                    if pu_bytes.is_empty() || after_pu >= bytes.len() || bytes[after_pu] != b';' {
                        i = after_pu;
                        continue;
                    }
                    let pu: u8 = match std::str::from_utf8(pu_bytes).ok()?.parse().ok() {
                        Some(v) => v,
                        None => {
                            i = after_pu;
                            continue;
                        }
                    };
                    let after_pu_semi = after_pu + 1;
                    let (px_bytes, after_px) = parse_number(bytes, after_pu_semi);
                    if px_bytes.is_empty() || after_px >= bytes.len() || bytes[after_px] != b';' {
                        i = after_px;
                        continue;
                    }
                    let after_px_semi = after_px + 1;
                    let (py_bytes, after_py) = parse_number(bytes, after_px_semi);
                    if py_bytes.is_empty() || after_py >= bytes.len() || bytes[after_py] != b';' {
                        i = after_py;
                        continue;
                    }
                    let after_py_semi = after_py + 1;
                    let (pz_bytes, after_pz) = parse_number(bytes, after_py_semi);
                    if pz_bytes.is_empty() {
                        i = after_pz;
                        continue;
                    }

                    let px_val: u8 = match std::str::from_utf8(px_bytes).ok()?.parse().ok() {
                        Some(v) => v,
                        None => {
                            i = after_pz;
                            continue;
                        }
                    };
                    let py_val: u8 = match std::str::from_utf8(py_bytes).ok()?.parse().ok() {
                        Some(v) => v,
                        None => {
                            i = after_pz;
                            continue;
                        }
                    };
                    let pz_val: u8 = match std::str::from_utf8(pz_bytes).ok()?.parse().ok() {
                        Some(v) => v,
                        None => {
                            i = after_pz;
                            continue;
                        }
                    };

                    match pu {
                        2 => {
                            // RGB: values are 0-100 (percentage).
                            let r = ((px_val as f32 / 100.0) * 255.0).min(255.0) as u8;
                            let g = ((py_val as f32 / 100.0) * 255.0).min(255.0) as u8;
                            let b = ((pz_val as f32 / 100.0) * 255.0).min(255.0) as u8;
                            state.define_color(idx, r, g, b);
                        }
                        1 => {
                            // HLS: not supported, treat as gray.
                            let lum = ((pz_val as f32 / 100.0) * 255.0).min(255.0) as u8;
                            state.define_color(idx, lum, lum, lum);
                        }
                        _ => {}
                    }
                    i = after_pz;
                } else {
                    // Just select color register: #Pc
                    state.select_color(idx);
                    i = next_i;
                }
            }

            b'!' => {
                if i + 1 >= bytes.len() {
                    break;
                }
                i += 1;
                let (num_bytes, next_i) = parse_number(bytes, i);
                if let Some(n) = std::str::from_utf8(num_bytes)
                    .ok()
                    .and_then(|s| s.parse::<u32>().ok())
                {
                    state.repeat = n;
                }
                i = next_i;
                // The next byte after `!N` is the sixel character, processed on next iteration.
            }

            b'"' => {
                // Skip raster attributes until the next sixel char or control
                i += 1;
                while i < bytes.len()
                    && bytes[i] != b'$'
                    && bytes[i] != b'-'
                    && !(0x3f..=0x7e).contains(&bytes[i])
                    && bytes[i] != b'#'
                    && bytes[i] != b'!'
                {
                    i += 1;
                }
            }

            _ => {
                state.feed(b);
                i += 1;
            }
        }
    }

    state.finalize()
}

fn parse_number(bytes: &[u8], start: usize) -> (&[u8], usize) {
    let mut end = start;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    (&bytes[start..end], end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sixel_basic_color_definition() {
        let data = b"#0;2;100;50;25";
        let result = decode_sixel(data);
        // No sixel drawn (no chars), so result is None.
        assert!(result.is_none());
    }

    #[test]
    fn test_sixel_empty_returns_none() {
        let result = decode_sixel(b"");
        assert!(result.is_none());
    }

    #[test]
    fn test_sixel_simple_dots() {
        // Default color register 0 = white (or [255,255,255,255]).
        // Draw a 1x6 vertical line with `?`~`~` (all bits set = 63 = 0b111111).
        // `~` = 0x7E, value 63 = all 6 bits on.
        let data = b"~~";
        let result = decode_sixel(data).expect("should decode");
        // Two sixel chars adjacent: width 2, height 6
        assert_eq!(result.width, 2);
        assert_eq!(result.height, 6);
        // Check that all pixels in the 2x6 region are white (RGBA = 255,255,255,255).
        for y in 0..6 {
            for x in 0..2 {
                let idx = (y as usize * 2 + x as usize) * 4;
                assert_eq!(
                    &result.rgba[idx..idx + 4],
                    &[255u8, 255, 255, 255],
                    "pixel ({},{}) should be white",
                    x,
                    y
                );
            }
        }
    }

    #[test]
    fn test_sixel_basic_pattern() {
        // `?` = val 0, `@` = val 1 (bottom pixel only), `A` = val 2 (bit 1 only)
        // First sixel ? is all off, second @ has only bit 0 (bottom pixel) on, third A has bit 1 on.
        let data = b"?@A";
        let result = decode_sixel(data).expect("should decode");
        assert_eq!(result.width, 3);
        assert_eq!(result.height, 6);

        // For @ (val=1): bottom pixel = row 0 (since bit 0 set). Wait...
        // Actually bit 0 = LSB = top, bit 5 = MSB = bottom.
        // Val=1 = 0b000001 = only top pixel.
        // Val=2 = 0b000010 = second pixel.
        // Let me verify with the test actually running.

        // Pixel @ (idx=1, val=1): row 0 (top) = (0*3+1)*4 = 4
        assert_eq!(result.rgba[4..8], [255, 255, 255, 255]);
        // Pixel @ row 1: should be 0, (1*3+1)*4 = 16
        assert_eq!(result.rgba[16..20], [0, 0, 0, 0]);

        // Pixel A (idx=2, val=2): row 1 (second from top) = (1*3+2)*4 = 20
        assert_eq!(result.rgba[20..24], [255, 255, 255, 255]);
    }

    #[test]
    fn test_sixel_repeat() {
        // `!3?` draws 3 blank sixels, then `~` draws one full.
        let data = b"!3?~";
        let result = decode_sixel(data).expect("should decode");
        // 4 sixels total: 3 blanks + 1 full
        assert_eq!(result.width, 4);
        assert_eq!(result.height, 6);
        // First 3 columns should be all 0 (from ? = blank).
        // (0*4+0)*4=0, (0*4+1)*4=4, (0*4+2)*4=8
        assert_eq!(result.rgba[0..4], [0, 0, 0, 0]);
        assert_eq!(result.rgba[4..8], [0, 0, 0, 0]);
        assert_eq!(result.rgba[8..12], [0, 0, 0, 0]);
        // 4th column (x=3) should be white from ~: (0*4+3)*4=12
        assert_eq!(result.rgba[12..16], [255, 255, 255, 255]);
    }

    #[test]
    fn test_sixel_newline() {
        // Draw at (0,0), move to next line, draw at (0,1)
        let data = b"?-~";
        let result = decode_sixel(data).expect("should decode");
        // Width = 1 (only x=0 has pixels), height = 12 (2 sixel rows = 12 pixels)
        assert_eq!(result.width, 1);
        assert_eq!(result.height, 12);
        // First sixel row (y=0-5): all black (?)
        for y in 0..6 {
            let idx = y * 4;
            assert_eq!(result.rgba[idx..idx + 4], [0, 0, 0, 0]);
        }
        // Second sixel row (y=6-11): white (~)
        for y in 6..12 {
            let idx = y * 4;
            assert_eq!(result.rgba[idx..idx + 4], [255, 255, 255, 255]);
        }
    }

    #[test]
    fn test_sixel_carriage_return() {
        // `~` draws at x=0, then `$~` should draw at x=0 again (overwrite)
        // Actually $ resets cursor to x=0, then ~ draws there.
        // The max_x should be 1 (since both draws are at x=0)
        let data = b"~$~";
        let result = decode_sixel(data).expect("should decode");
        assert_eq!(result.width, 1);
        assert_eq!(result.height, 6);
    }

    #[test]
    fn test_sixel_with_color() {
        // Define register 1 as red (100,0,0), select it, draw one sixel
        let data = b"#1;2;100;0;0#1~";
        let result = decode_sixel(data).expect("should decode");
        // Pixel should be red (255,0,0,255)
        let idx = 0;
        assert_eq!(result.rgba[idx..idx + 4], [255, 0, 0, 255]);
    }

    #[test]
    fn test_sixel_decode_from_spec_example() {
        // Classic test: printf '\ePq#0;2;100;100;100#1;2;100;0;0#2;2;100;100;0#1~~@@#2~~@@#0$$$$$\e\\'
        // We test just the data part (what decode_sixel receives).
        let data = b"#0;2;100;100;100#1;2;100;0;0#2;2;100;100;0#1~~@@#2~~@@#0";
        let result = decode_sixel(data);
        // Should produce some pixels
        assert!(result.is_some());
        let result = result.unwrap();
        // At least some pixels drawn
        assert!(result.width > 0);
        assert!(result.height > 0);
    }
}
