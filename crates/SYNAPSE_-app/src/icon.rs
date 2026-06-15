//! Procedural cyberpunk app icon — no external asset required.
//!
//! Draws a neon terminal prompt `>_` in electric red (`#FF003C`) on the void
//! blue-black background (`#0A0C14`) with a thin neon frame. Rendered to an
//! anti-aliased RGBA buffer at any size: used as the runtime window icon
//! (Linux) and exported to PNG for packaging (`--export-icon`).

const BG: [f32; 3] = [
    0x0A as f32 / 255.0,
    0x0C as f32 / 255.0,
    0x14 as f32 / 255.0,
];
const RED: [f32; 3] = [
    0xFF as f32 / 255.0,
    0x00 as f32 / 255.0,
    0x3C as f32 / 255.0,
];

/// Distance from point P to segment AB.
fn dist_to_segment(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let (abx, aby) = (bx - ax, by - ay);
    let (apx, apy) = (px - ax, py - ay);
    let len_sq = abx * abx + aby * aby;
    let t = if len_sq > 0.0 {
        ((apx * abx + apy * aby) / len_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (cx, cy) = (ax + t * abx, ay + t * aby);
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Coverage [0,1] of a stroke of the given half-thickness, anti-aliased.
fn stroke_cov(d: f32, half: f32, aa: f32) -> f32 {
    1.0 - smoothstep(half - aa, half + aa, d)
}

/// Generate the icon as non-premultiplied RGBA8 bytes (`size`×`size`).
pub fn rgba(size: u32) -> Vec<u8> {
    let n = size as f32;
    let aa = (n * 0.004).max(0.75);
    let mut buf = vec![0u8; (size * size * 4) as usize];

    // Geometry (fractions of the canvas).
    let border = n * 0.055;
    let glow = n * 0.045;

    // Chevron ">": arms meeting at a tip pointing right.
    let (a_x, a_y) = (n * 0.34, n * 0.30);
    let (t_x, t_y) = (n * 0.56, n * 0.47);
    let (b_x, b_y) = (n * 0.34, n * 0.64);
    let chev_half = n * 0.040;

    // Underscore "_".
    let (u0_x, u_y) = (n * 0.40, n * 0.72);
    let u1_x = n * 0.67;
    let under_half = n * 0.033;

    for y in 0..size {
        for x in 0..size {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);

            let mut color = BG;

            // Neon frame: a red band inset from the edge.
            let edge_dist = fx.min(fy).min(n - fx).min(n - fy);
            let frame = stroke_cov((edge_dist - border).abs(), border * 0.45, aa);

            // Prompt strokes (chevron + underscore), with a soft outer glow.
            let d_chev = dist_to_segment(fx, fy, a_x, a_y, t_x, t_y)
                .min(dist_to_segment(fx, fy, t_x, t_y, b_x, b_y));
            let d_under = dist_to_segment(fx, fy, u0_x, u_y, u1_x, u_y);
            let d = d_chev.min(d_under);
            let half = if d_chev <= d_under {
                chev_half
            } else {
                under_half
            };

            let sharp = stroke_cov(d, half, aa);
            let glow_cov = (1.0 - smoothstep(half, half + glow, d)) * 0.35;

            let red_cov = frame.max(sharp).max(glow_cov).clamp(0.0, 1.0);
            for c in 0..3 {
                color[c] = color[c] * (1.0 - red_cov) + RED[c] * red_cov;
            }

            let idx = ((y * size + x) * 4) as usize;
            buf[idx] = (color[0] * 255.0).round() as u8;
            buf[idx + 1] = (color[1] * 255.0).round() as u8;
            buf[idx + 2] = (color[2] * 255.0).round() as u8;
            buf[idx + 3] = 255;
        }
    }
    buf
}

/// Build a winit window icon (used on Linux; ignored on macOS).
pub fn window_icon() -> Option<winit::window::Icon> {
    let size = 128;
    winit::window::Icon::from_rgba(rgba(size), size, size).ok()
}

/// Write the icon to a PNG file (for packaging `.ico` / `.icns` / README).
pub fn export_png(path: &str, size: u32) -> Result<(), String> {
    let size = size.clamp(16, 1024);
    let data = rgba(size);
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size, size);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&data).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_has_correct_length_and_opaque() {
        let data = rgba(64);
        assert_eq!(data.len(), 64 * 64 * 4);
        assert!(data.chunks(4).all(|p| p[3] == 255), "icon is fully opaque");
    }

    #[test]
    fn icon_contains_background_and_accent() {
        let data = rgba(128);
        let has_bg = data
            .chunks(4)
            .any(|p| p[0] < 0x20 && p[1] < 0x20 && p[2] < 0x30);
        let has_red = data.chunks(4).any(|p| p[0] > 0xC0 && p[1] < 0x40);
        assert!(has_bg, "must contain void-blue background");
        assert!(has_red, "must contain electric-red accent");
    }

    #[test]
    fn window_icon_builds() {
        assert!(window_icon().is_some());
    }
}
