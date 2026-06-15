use std::collections::HashMap;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

use crate::text::{GlyphKey, ShapedGlyphKey};

/// Default atlas size when the GPU has no stricter limit.
pub const ATLAS_SIZE: u32 = 2048;
/// Atlas size cap in low-memory mode (saves 12 MB VRAM vs 2048).
const ATLAS_SIZE_LOW_MEM: u32 = 1024;
/// Entries not touched in this many frames are candidates for eviction.
const EVICTION_AGE: u64 = 300;
/// Maximum warm-cache entries kept in CPU RAM between frames.
const MAX_WARM_ENTRIES: usize = 2048;

#[derive(Debug, Clone, Copy)]
pub struct UvRect {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
}

#[derive(Debug, Clone)]
struct AtlasEntry {
    uv: UvRect,
    last_frame: u64,
}

pub struct TextureAtlas {
    pub texture: wgpu::Texture,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
    cache: HashMap<GlyphKey, AtlasEntry>,
    shaped_cache: HashMap<ShapedGlyphKey, AtlasEntry>,
    emoji_cache: HashMap<u32, AtlasEntry>,
    x_offset: u32,
    y_offset: u32,
    row_height: u32,
    frame: u64,
    needs_reset: bool,
    warned_90: bool,
    atlas_size: u32,
    // CPU-side pixel mirror for cross-session warm loading.
    warm_glyphs: HashMap<GlyphKey, (Vec<u8>, u32, u32)>,
    warm_shaped: HashMap<ShapedGlyphKey, (Vec<u8>, u32, u32)>,
    warm_emoji: HashMap<u32, (Vec<u8>, u32, u32)>,
    /// When false (low_memory_mode), warm cache storage is skipped entirely.
    warm_enabled: bool,
}

#[derive(Debug)]
pub struct EvictionMetrics {
    pub evicted: usize,
    pub kept: usize,
    pub utilization: f32,
}

impl TextureAtlas {
    pub fn new(device: &wgpu::Device, max_texture_size: u32, low_memory: bool) -> Self {
        let cap = if low_memory { ATLAS_SIZE_LOW_MEM } else { ATLAS_SIZE };
        let atlas_size = max_texture_size.clamp(512, cap);
        let size = wgpu::Extent3d {
            width: atlas_size,
            height: atlas_size,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SYNAPSE_ Glyph Atlas"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SYNAPSE_ Atlas Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SYNAPSE_ Atlas BindGroupLayout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SYNAPSE_ Atlas BindGroup"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            texture,
            bind_group_layout,
            bind_group,
            cache: HashMap::new(),
            shaped_cache: HashMap::new(),
            emoji_cache: HashMap::new(),
            x_offset: 0,
            y_offset: 0,
            row_height: 0,
            frame: 0,
            needs_reset: false,
            warned_90: false,
            atlas_size,
            warm_glyphs: HashMap::new(),
            warm_shaped: HashMap::new(),
            warm_emoji: HashMap::new(),
            warm_enabled: !low_memory,
        }
    }

    pub fn begin_frame(&mut self) {
        self.frame += 1;

        if self.needs_reset {
            let evicted = self.cache.len() + self.shaped_cache.len() + self.emoji_cache.len();
            self.cache.clear();
            self.shaped_cache.clear();
            self.emoji_cache.clear();
            self.x_offset = 0;
            self.y_offset = 0;
            self.row_height = 0;
            self.needs_reset = false;
            self.warned_90 = false;
            tracing::warn!(
                "glyph atlas reset — cleared {} cached entries (frame {})",
                evicted,
                self.frame,
            );
        }
    }

    pub fn utilization(&self) -> f32 {
        (self.y_offset + self.row_height) as f32 / self.atlas_size as f32
    }

    pub fn get_or_insert(
        &mut self,
        key: GlyphKey,
        bitmap_width: u32,
        bitmap_height: u32,
    ) -> Option<(UvRect, bool)> {
        if let Some(entry) = self.cache.get_mut(&key) {
            entry.last_frame = self.frame;
            return Some((entry.uv, false));
        }

        let rect = self.allocate(bitmap_width, bitmap_height)?;
        self.cache.insert(
            key,
            AtlasEntry {
                uv: rect,
                last_frame: self.frame,
            },
        );
        Some((rect, true))
    }

    pub fn get_or_insert_shaped(
        &mut self,
        key: ShapedGlyphKey,
        bitmap_width: u32,
        bitmap_height: u32,
    ) -> Option<(UvRect, bool)> {
        if let Some(entry) = self.shaped_cache.get_mut(&key) {
            entry.last_frame = self.frame;
            return Some((entry.uv, false));
        }
        let rect = self.allocate(bitmap_width, bitmap_height)?;
        self.shaped_cache.insert(
            key,
            AtlasEntry {
                uv: rect,
                last_frame: self.frame,
            },
        );
        Some((rect, true))
    }

    pub fn get_or_insert_emoji(
        &mut self,
        emoji_key: u32,
        width: u32,
        height: u32,
    ) -> Option<(UvRect, bool)> {
        if let Some(entry) = self.emoji_cache.get_mut(&emoji_key) {
            entry.last_frame = self.frame;
            return Some((entry.uv, false));
        }
        let rect = self.allocate(width, height)?;
        self.emoji_cache.insert(
            emoji_key,
            AtlasEntry {
                uv: rect,
                last_frame: self.frame,
            },
        );
        Some((rect, true))
    }

    fn allocate(&mut self, width: u32, height: u32) -> Option<UvRect> {
        if width == 0 || height == 0 {
            return None;
        }

        let sz = self.atlas_size;

        if self.x_offset + width > sz {
            self.x_offset = 0;
            self.y_offset += self.row_height;
            self.row_height = 0;
        }

        if self.y_offset + height > sz {
            match self.try_evict_and_compact() {
                Some(metrics) => {
                    tracing::info!(
                        "glyph atlas compacted: evicted {}, kept {} (util {:.0}%)",
                        metrics.evicted,
                        metrics.kept,
                        metrics.utilization * 100.0,
                    );
                }
                None => {
                    let util = self.utilization() * 100.0;
                    tracing::warn!(
                        "glyph atlas full at {:.0}% — scheduling reset (frame {})",
                        util,
                        self.frame,
                    );
                    self.needs_reset = true;
                    return None;
                }
            }

            // Retry after eviction+compact
            if self.x_offset + width > sz {
                self.x_offset = 0;
                self.y_offset += self.row_height;
                self.row_height = 0;
            }
            if self.y_offset + height > sz {
                self.needs_reset = true;
                return None;
            }
        }

        let u0 = self.x_offset as f32 / sz as f32;
        let v0 = self.y_offset as f32 / sz as f32;
        let u1 = (self.x_offset + width) as f32 / sz as f32;
        let v1 = (self.y_offset + height) as f32 / sz as f32;

        self.x_offset += width;
        self.row_height = self.row_height.max(height);

        if !self.warned_90 && self.utilization() >= 0.9 {
            self.warned_90 = true;
            tracing::warn!(
                "glyph atlas at {:.0}% utilization — LRU eviction will trigger soon (frame {})",
                self.utilization() * 100.0,
                self.frame,
            );
        }

        Some(UvRect { u0, v0, u1, v1 })
    }

    fn try_evict_and_compact(&mut self) -> Option<EvictionMetrics> {
        let total_before = self.cache.len() + self.shaped_cache.len() + self.emoji_cache.len();
        if total_before == 0 {
            return None;
        }

        // Evict stale entries, then clear ALL remaining entries and reset the cursor.
        // Without GPU-side texture copies we cannot compact in-place: resetting the
        // cursor invalidates every stored UV position, so surviving entries must
        // re-rasterize and re-upload on their next use.  This costs one heavier frame
        // but avoids a one-frame rendering glitch (unlike the deferred needs_reset path).
        let evicted = self.evict_lru();
        let kept = total_before - evicted;

        self.cache.clear();
        self.shaped_cache.clear();
        self.emoji_cache.clear();
        self.x_offset = 0;
        self.y_offset = 0;
        self.row_height = 0;
        self.warned_90 = false;

        Some(EvictionMetrics {
            evicted,
            kept,
            utilization: 0.0,
        })
    }

    fn evict_lru(&mut self) -> usize {
        let threshold = self.frame.saturating_sub(EVICTION_AGE);
        let before = self.cache.len();
        self.cache.retain(|_, e| e.last_frame >= threshold);
        let c_evicted = before - self.cache.len();

        let before_s = self.shaped_cache.len();
        self.shaped_cache.retain(|_, e| e.last_frame >= threshold);
        let s_evicted = before_s - self.shaped_cache.len();

        let before_e = self.emoji_cache.len();
        self.emoji_cache.retain(|_, e| e.last_frame >= threshold);
        let e_evicted = before_e - self.emoji_cache.len();

        c_evicted + s_evicted + e_evicted
    }

    pub fn upload_glyph(
        &mut self,
        queue: &wgpu::Queue,
        rect: UvRect,
        rgba_bitmap: &[u8],
        bitmap_width: u32,
        bitmap_height: u32,
    ) {
        let x = (rect.u0 * self.atlas_size as f32) as u32;
        let y = (rect.v0 * self.atlas_size as f32) as u32;

        let raw_bytes_per_row = 4 * bitmap_width;
        let aligned_bytes_per_row = raw_bytes_per_row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

        let padded_size = (aligned_bytes_per_row * bitmap_height) as usize;
        let mut padded = vec![0u8; padded_size];
        for row in 0..bitmap_height as usize {
            let src = row * raw_bytes_per_row as usize;
            let dst = row * aligned_bytes_per_row as usize;
            padded[dst..dst + raw_bytes_per_row as usize]
                .copy_from_slice(&rgba_bitmap[src..src + raw_bytes_per_row as usize]);
        }

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &padded,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(aligned_bytes_per_row),
                rows_per_image: Some(bitmap_height),
            },
            wgpu::Extent3d {
                width: bitmap_width,
                height: bitmap_height,
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn store_warm(&mut self, key: GlyphKey, rgba: &[u8], w: u32, h: u32) {
        if !self.warm_enabled || self.warm_glyphs.len() >= MAX_WARM_ENTRIES {
            return;
        }
        self.warm_glyphs.insert(key, (rgba.to_vec(), w, h));
    }

    pub fn store_warm_shaped(&mut self, key: ShapedGlyphKey, rgba: &[u8], w: u32, h: u32) {
        if !self.warm_enabled || self.warm_shaped.len() >= MAX_WARM_ENTRIES {
            return;
        }
        self.warm_shaped.insert(key, (rgba.to_vec(), w, h));
    }

    pub fn store_warm_emoji(&mut self, key: u32, rgba: &[u8], w: u32, h: u32) {
        if !self.warm_enabled || self.warm_emoji.len() >= MAX_WARM_ENTRIES {
            return;
        }
        self.warm_emoji.insert(key, (rgba.to_vec(), w, h));
    }

    /// Persist the warm pixel cache to disk. Called on graceful shutdown.
    pub fn save_warm_cache(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::File::create(path)?;
        let mut w = BufWriter::new(file);

        w.write_all(b"SATL")?;
        w.write_all(&[1u8])?; // version
        w.write_all(&self.atlas_size.to_le_bytes())?;

        w.write_all(&(self.warm_glyphs.len() as u32).to_le_bytes())?;
        for (key, (rgba, gw, gh)) in &self.warm_glyphs {
            w.write_all(&(key.ch as u32).to_le_bytes())?;
            w.write_all(&key.font_size_bits.to_le_bytes())?;
            w.write_all(&[key.bold as u8, key.italic as u8, key.font_index])?;
            w.write_all(&gw.to_le_bytes())?;
            w.write_all(&gh.to_le_bytes())?;
            w.write_all(&(rgba.len() as u32).to_le_bytes())?;
            w.write_all(rgba)?;
        }

        w.write_all(&(self.warm_shaped.len() as u32).to_le_bytes())?;
        for (key, (rgba, gw, gh)) in &self.warm_shaped {
            w.write_all(&key.glyph_id.to_le_bytes())?;
            w.write_all(&key.font_size_bits.to_le_bytes())?;
            w.write_all(&[key.bold as u8, key.italic as u8, key.font_index])?;
            w.write_all(&gw.to_le_bytes())?;
            w.write_all(&gh.to_le_bytes())?;
            w.write_all(&(rgba.len() as u32).to_le_bytes())?;
            w.write_all(rgba)?;
        }

        w.write_all(&(self.warm_emoji.len() as u32).to_le_bytes())?;
        for (key, (rgba, gw, gh)) in &self.warm_emoji {
            w.write_all(&key.to_le_bytes())?;
            w.write_all(&gw.to_le_bytes())?;
            w.write_all(&gh.to_le_bytes())?;
            w.write_all(&(rgba.len() as u32).to_le_bytes())?;
            w.write_all(rgba)?;
        }

        w.flush()?;
        tracing::debug!(
            "warm cache saved: {} glyphs, {} shaped, {} emoji → {}",
            self.warm_glyphs.len(),
            self.warm_shaped.len(),
            self.warm_emoji.len(),
            path.display(),
        );
        Ok(())
    }

    /// Load warm cache from disk and re-upload to GPU. Returns number of glyphs loaded.
    pub fn load_and_warm(&mut self, path: &Path, queue: &wgpu::Queue) -> io::Result<usize> {
        let file = std::fs::File::open(path)?;
        let mut r = BufReader::new(file);

        let mut magic = [0u8; 4];
        r.read_exact(&mut magic)?;
        if &magic != b"SATL" {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "bad magic"));
        }
        let mut ver = [0u8];
        r.read_exact(&mut ver)?;
        if ver[0] != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported version",
            ));
        }
        let saved_size = read_u32(&mut r)?;
        if saved_size != self.atlas_size {
            tracing::debug!(
                "warm cache atlas_size mismatch ({saved_size} vs {}), skipping",
                self.atlas_size
            );
            return Ok(0);
        }

        let mut count = 0usize;

        let glyph_count = read_u32(&mut r)?;
        let mut glyphs = Vec::with_capacity(glyph_count as usize);
        for _ in 0..glyph_count {
            let ch = char::from_u32(read_u32(&mut r)?)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bad char"))?;
            let font_size_bits = read_u32(&mut r)?;
            let mut flags = [0u8; 3];
            r.read_exact(&mut flags)?;
            let key = GlyphKey {
                ch,
                font_size_bits,
                bold: flags[0] != 0,
                italic: flags[1] != 0,
                font_index: flags[2],
            };
            let gw = read_u32(&mut r)?;
            let gh = read_u32(&mut r)?;
            let rgba = read_bytes(&mut r)?;
            glyphs.push((key, rgba, gw, gh));
        }
        for (key, rgba, gw, gh) in glyphs {
            self.warm_glyphs.insert(key, (rgba.clone(), gw, gh));
            if let Some((uv, true)) = self.get_or_insert(key, gw, gh) {
                self.upload_glyph(queue, uv, &rgba, gw, gh);
                count += 1;
            }
        }

        let shaped_count = read_u32(&mut r)?;
        let mut shaped = Vec::with_capacity(shaped_count as usize);
        for _ in 0..shaped_count {
            let glyph_id = read_u16(&mut r)?;
            let font_size_bits = read_u32(&mut r)?;
            let mut flags = [0u8; 3];
            r.read_exact(&mut flags)?;
            let key = ShapedGlyphKey {
                glyph_id,
                font_size_bits,
                bold: flags[0] != 0,
                italic: flags[1] != 0,
                font_index: flags[2],
            };
            let gw = read_u32(&mut r)?;
            let gh = read_u32(&mut r)?;
            let rgba = read_bytes(&mut r)?;
            shaped.push((key, rgba, gw, gh));
        }
        for (key, rgba, gw, gh) in shaped {
            self.warm_shaped.insert(key, (rgba.clone(), gw, gh));
            if let Some((uv, true)) = self.get_or_insert_shaped(key, gw, gh) {
                self.upload_glyph(queue, uv, &rgba, gw, gh);
                count += 1;
            }
        }

        let emoji_count = read_u32(&mut r)?;
        let mut emojis = Vec::with_capacity(emoji_count as usize);
        for _ in 0..emoji_count {
            let key = read_u32(&mut r)?;
            let gw = read_u32(&mut r)?;
            let gh = read_u32(&mut r)?;
            let rgba = read_bytes(&mut r)?;
            emojis.push((key, rgba, gw, gh));
        }
        for (key, rgba, gw, gh) in emojis {
            self.warm_emoji.insert(key, (rgba.clone(), gw, gh));
            if let Some((uv, true)) = self.get_or_insert_emoji(key, gw, gh) {
                self.upload_glyph(queue, uv, &rgba, gw, gh);
                count += 1;
            }
        }

        tracing::debug!("warm cache loaded: {count} glyphs from {}", path.display());
        Ok(count)
    }
}

fn read_u16(r: &mut impl Read) -> io::Result<u16> {
    let mut b = [0u8; 2];
    r.read_exact(&mut b)?;
    Ok(u16::from_le_bytes(b))
}

fn read_u32(r: &mut impl Read) -> io::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn read_bytes(r: &mut impl Read) -> io::Result<Vec<u8>> {
    let len = read_u32(r)? as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::GlyphKey;

    fn make_key(ch: char) -> GlyphKey {
        GlyphKey::new(ch, 14.0, false, false)
    }

    #[test]
    fn test_utilization() {
        // Test allocation logic without GPU
        let mut x_off: u32 = 0;
        let mut y_off: u32 = 0;
        let mut row_h: u32 = 0;

        for i in 0..100u32 {
            let w = (i % 5 + 1) * 8;
            let h = (i % 3 + 1) * 12;
            if x_off + w > ATLAS_SIZE {
                x_off = 0;
                y_off += row_h;
                row_h = 0;
            }
            let util = (y_off + row_h) as f32 / ATLAS_SIZE as f32;
            if i == 50 {
                assert!(util > 0.0, "util should increase by glyph 50");
            }
            x_off += w;
            row_h = row_h.max(h);
        }
    }

    #[test]
    fn test_allocate_no_overlap() {
        let mut x_off: u32 = 0;
        let mut y_off: u32 = 0;
        let mut row_h: u32 = 0;
        let mut rects = Vec::new();

        for i in 0..100u32 {
            let w = (i % 5 + 1) * 8;
            let h = (i % 3 + 1) * 12;

            if x_off + w > ATLAS_SIZE {
                x_off = 0;
                y_off += row_h;
                row_h = 0;
            }
            assert!(y_off + h <= ATLAS_SIZE, "Atlas overflow at glyph {}", i);

            let u0 = x_off as f32 / ATLAS_SIZE as f32;
            let v0 = y_off as f32 / ATLAS_SIZE as f32;
            let u1 = (x_off + w) as f32 / ATLAS_SIZE as f32;
            let v1 = (y_off + h) as f32 / ATLAS_SIZE as f32;

            x_off += w;
            row_h = row_h.max(h);
            rects.push((u0, v0, u1, v1));

            for (j, &(pu0, pv0, pu1, pv1)) in rects.iter().enumerate().take(rects.len() - 1) {
                let overlap = !(u0 >= pu1 || u1 <= pu0 || v0 >= pv1 || v1 <= pv0);
                assert!(!overlap, "Overlap between glyph {} and {}", i, j);
            }
        }
    }

    #[test]
    fn test_lru_eviction_preserves_recent() {
        let mut atlas = TextureAtlas::dummy();
        atlas.frame = 1;

        let key_a = make_key('A');
        let key_b = make_key('B');
        assert!(atlas.get_or_insert(key_a, 32, 32).unwrap().1);
        assert!(atlas.get_or_insert(key_b, 32, 32).unwrap().1);

        // Fast-forward past eviction threshold
        atlas.frame = EVICTION_AGE + 10;
        // Touch key_b so it stays alive
        let (_, is_new) = atlas.get_or_insert(key_b, 32, 32).unwrap();
        assert!(!is_new);

        atlas.evict_lru();
        // key_a should be gone (wasn't touched)
        let (_, is_new_a) = atlas.get_or_insert(key_a, 32, 32).unwrap();
        assert!(is_new_a, "key_a should have been evicted");
        // key_b should survive
        let (_, is_new_b) = atlas.get_or_insert(key_b, 32, 32).unwrap();
        assert!(!is_new_b, "key_b should still be cached");
    }

    #[test]
    fn test_evict_lru_empty() {
        let mut atlas = TextureAtlas::dummy();
        assert_eq!(atlas.evict_lru(), 0);
    }

    #[test]
    fn test_eviction_metrics_empty() {
        let mut atlas = TextureAtlas::dummy();
        atlas.frame = 1;
        let result = atlas.try_evict_and_compact();
        assert!(result.is_none(), "empty atlas should return None");
    }

    #[test]
    fn test_compact_clears_all_entries_and_resets_cursor() {
        let mut atlas = TextureAtlas::dummy();
        atlas.frame = 1;

        // Insert two glyphs
        let key_a = make_key('A');
        let key_b = make_key('B');
        assert!(atlas.get_or_insert(key_a, 32, 32).unwrap().1);
        assert!(atlas.get_or_insert(key_b, 32, 32).unwrap().1);
        assert!(atlas.x_offset > 0 || atlas.y_offset > 0 || atlas.row_height > 0);

        // Age both past eviction threshold then compact
        atlas.frame = EVICTION_AGE + 10;
        let metrics = atlas
            .try_evict_and_compact()
            .expect("should compact non-empty atlas");

        // Cursor reset
        assert_eq!(atlas.x_offset, 0);
        assert_eq!(atlas.y_offset, 0);
        assert_eq!(atlas.row_height, 0);

        // All entries cleared (surviving entries must re-upload; corrupted UVs avoided)
        assert_eq!(atlas.cache.len(), 0);
        assert_eq!(atlas.shaped_cache.len(), 0);
        assert_eq!(atlas.emoji_cache.len(), 0);

        // Both were evicted (stale)
        assert_eq!(metrics.evicted, 2);

        // After compact, both keys are treated as new on next access
        let (_, is_new_a) = atlas.get_or_insert(key_a, 32, 32).unwrap();
        assert!(is_new_a, "key_a must re-upload after compact");
        let (_, is_new_b) = atlas.get_or_insert(key_b, 32, 32).unwrap();
        assert!(is_new_b, "key_b must re-upload after compact");
    }

    #[test]
    fn test_compact_recent_entries_still_cleared() {
        // Even entries that were freshly used must be cleared after compact
        // (their GPU pixels would be at old positions after cursor reset).
        let mut atlas = TextureAtlas::dummy();
        atlas.frame = 1;

        let key_a = make_key('A');
        assert!(atlas.get_or_insert(key_a, 32, 32).unwrap().1);

        // Touch key_a to keep it "recent" relative to EVICTION_AGE
        atlas.frame = 5;
        let (_, is_new) = atlas.get_or_insert(key_a, 32, 32).unwrap();
        assert!(!is_new);

        // Compact while entry is still recent
        let metrics = atlas.try_evict_and_compact().unwrap();
        assert_eq!(metrics.evicted, 0, "nothing stale enough to evict");
        assert_eq!(metrics.kept, 1, "one entry was alive before compact");

        // Cache must still be cleared to prevent UV corruption
        assert_eq!(atlas.cache.len(), 0);
        let (_, is_new_after) = atlas.get_or_insert(key_a, 32, 32).unwrap();
        assert!(
            is_new_after,
            "key_a needs re-upload even though it was recent"
        );
    }

    #[test]
    fn test_utilization_zero_initially() {
        let atlas = TextureAtlas::dummy();
        assert_eq!(atlas.utilization(), 0.0);
    }

    impl TextureAtlas {
        #[cfg(test)]
        fn dummy() -> Self {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
            let adapter = pollster::block_on(
                instance.request_adapter(&wgpu::RequestAdapterOptions::default()),
            )
            .expect("no adapter");
            let (device, _queue) = pollster::block_on(adapter.request_device(
                &wgpu::DeviceDescriptor {
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    ..Default::default()
                },
                None,
            ))
            .expect("no device");

            let size = wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            };

            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("test atlas"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

            let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

            let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("test layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("test bg"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });

            Self {
                texture,
                bind_group_layout,
                bind_group,
                cache: HashMap::new(),
                shaped_cache: HashMap::new(),
                emoji_cache: HashMap::new(),
                x_offset: 0,
                y_offset: 0,
                row_height: 0,
                frame: 0,
                needs_reset: false,
                warned_90: false,
                atlas_size: ATLAS_SIZE,
                warm_glyphs: HashMap::new(),
                warm_shaped: HashMap::new(),
                warm_emoji: HashMap::new(),
                warm_enabled: true,
            }
        }
    }
}
