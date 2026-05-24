use std::collections::HashMap;

use crate::text::{GlyphKey, ShapedGlyphKey};

pub const ATLAS_SIZE: u32 = 2048;

/// Entries not touched in this many frames are candidates for eviction.
const EVICTION_AGE: u64 = 300;

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
    width: u32,
    height: u32,
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
}

#[derive(Debug)]
pub struct EvictionMetrics {
    pub evicted: usize,
    pub kept: usize,
    pub utilization: f32,
}

impl TextureAtlas {
    pub fn new(device: &wgpu::Device) -> Self {
        let size = wgpu::Extent3d {
            width: ATLAS_SIZE,
            height: ATLAS_SIZE,
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
        (self.y_offset + self.row_height) as f32 / ATLAS_SIZE as f32
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
                width: bitmap_width,
                height: bitmap_height,
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
                width: bitmap_width,
                height: bitmap_height,
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
                width,
                height,
                last_frame: self.frame,
            },
        );
        Some((rect, true))
    }

    fn allocate(&mut self, width: u32, height: u32) -> Option<UvRect> {
        if width == 0 || height == 0 {
            return None;
        }

        if self.x_offset + width > ATLAS_SIZE {
            self.x_offset = 0;
            self.y_offset += self.row_height;
            self.row_height = 0;
        }

        if self.y_offset + height > ATLAS_SIZE {
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
            if self.x_offset + width > ATLAS_SIZE {
                self.x_offset = 0;
                self.y_offset += self.row_height;
                self.row_height = 0;
            }
            if self.y_offset + height > ATLAS_SIZE {
                self.needs_reset = true;
                return None;
            }
        }

        let u0 = self.x_offset as f32 / ATLAS_SIZE as f32;
        let v0 = self.y_offset as f32 / ATLAS_SIZE as f32;
        let u1 = (self.x_offset + width) as f32 / ATLAS_SIZE as f32;
        let v1 = (self.y_offset + height) as f32 / ATLAS_SIZE as f32;

        self.x_offset += width;
        self.row_height = self.row_height.max(height);

        Some(UvRect { u0, v0, u1, v1 })
    }

    fn try_evict_and_compact(&mut self) -> Option<EvictionMetrics> {
        let before = self.cache.len() + self.shaped_cache.len() + self.emoji_cache.len();
        let evicted = self.evict_lru();
        if before == 0 {
            return None;
        }

        let mut items: Vec<(u32, u32)> =
            Vec::with_capacity(self.cache.len() + self.shaped_cache.len() + self.emoji_cache.len());
        for entry in self.cache.values() {
            items.push((entry.width, entry.height));
        }
        for entry in self.shaped_cache.values() {
            items.push((entry.width, entry.height));
        }
        for entry in self.emoji_cache.values() {
            items.push((entry.width, entry.height));
        }

        self.x_offset = 0;
        self.y_offset = 0;
        self.row_height = 0;

        let mut new_rects: Vec<Option<UvRect>> = Vec::with_capacity(items.len());
        for (w, h) in &items {
            new_rects.push(self.raw_allocate(*w, *h));
        }

        let mut idx = 0;
        for entry in self.cache.values_mut() {
            if let Some(Some(rect)) = new_rects.get(idx) {
                entry.uv = *rect;
            }
            idx += 1;
        }
        for entry in self.shaped_cache.values_mut() {
            if let Some(Some(rect)) = new_rects.get(idx) {
                entry.uv = *rect;
            }
            idx += 1;
        }
        for entry in self.emoji_cache.values_mut() {
            if let Some(Some(rect)) = new_rects.get(idx) {
                entry.uv = *rect;
            }
            idx += 1;
        }

        Some(EvictionMetrics {
            evicted,
            kept: self.cache.len() + self.shaped_cache.len() + self.emoji_cache.len(),
            utilization: self.utilization(),
        })
    }

    fn raw_allocate(&mut self, width: u32, height: u32) -> Option<UvRect> {
        if width == 0 || height == 0 {
            return None;
        }
        if self.x_offset + width > ATLAS_SIZE {
            self.x_offset = 0;
            self.y_offset += self.row_height;
            self.row_height = 0;
        }
        if self.y_offset + height > ATLAS_SIZE {
            return None;
        }
        let u0 = self.x_offset as f32 / ATLAS_SIZE as f32;
        let v0 = self.y_offset as f32 / ATLAS_SIZE as f32;
        let u1 = (self.x_offset + width) as f32 / ATLAS_SIZE as f32;
        let v1 = (self.y_offset + height) as f32 / ATLAS_SIZE as f32;
        self.x_offset += width;
        self.row_height = self.row_height.max(height);
        Some(UvRect { u0, v0, u1, v1 })
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
        let x = (rect.u0 * ATLAS_SIZE as f32) as u32;
        let y = (rect.v0 * ATLAS_SIZE as f32) as u32;

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
            }
        }
    }
}
