use std::collections::HashMap;
use wgpu::{Device, Queue};

use crate::text::{GlyphKey, ShapedGlyphKey};

pub const ATLAS_SIZE: u32 = 2048;

#[derive(Debug, Clone, Copy)]
pub struct UvRect {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
}

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
    x_offset: u32,
    y_offset: u32,
    row_height: u32,
    frame: u64,
    needs_reset: bool,
    warned_90: bool,
}

impl TextureAtlas {
    pub fn new(device: &Device) -> Self {
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
            let evicted = self.cache.len() + self.shaped_cache.len();
            self.cache.clear();
            self.shaped_cache.clear();
            self.x_offset = 0;
            self.y_offset = 0;
            self.row_height = 0;
            self.needs_reset = false;
            self.warned_90 = false;
            tracing::warn!(
                "glyph atlas reset — evicted {} cached entries (frame {})",
                evicted,
                self.frame,
            );
        }

        if !self.warned_90 {
            let fill = self.fill_fraction();
            if fill >= 0.9 {
                tracing::warn!(
                    "glyph atlas at {:.0}% capacity — will reset next frame",
                    fill * 100.0,
                );
                self.warned_90 = true;
                self.needs_reset = true;
            }
        }
    }

    fn fill_fraction(&self) -> f32 {
        (self.y_offset + self.row_height) as f32 / ATLAS_SIZE as f32
    }

    /// Returns `(uv, is_new)`. If `is_new`, caller must call `upload_glyph`.
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
        self.cache.insert(key, AtlasEntry { uv: rect, last_frame: self.frame });
        Some((rect, true))
    }

    /// Same as `get_or_insert` but keyed by shaped glyph ID (for ligatures).
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
        self.shaped_cache.insert(key, AtlasEntry { uv: rect, last_frame: self.frame });
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
            self.needs_reset = true;
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

    /// Upload RGBA bytes to the atlas at the rect returned by `get_or_insert`.
    pub fn upload_glyph(
        &mut self,
        queue: &Queue,
        rect: UvRect,
        rgba_bitmap: &[u8],
        bitmap_width: u32,
        bitmap_height: u32,
    ) {
        let x = (rect.u0 * ATLAS_SIZE as f32) as u32;
        let y = (rect.v0 * ATLAS_SIZE as f32) as u32;

        let raw_bytes_per_row = 4 * bitmap_width;
        let aligned_bytes_per_row =
            raw_bytes_per_row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
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

    #[allow(dead_code)]
    fn make_key(ch: char) -> GlyphKey {
        GlyphKey::new(ch, 14.0, false, false)
    }

    #[test]
    fn test_allocate_no_overlap() {
        let atlas_size = ATLAS_SIZE;
        let mut x_offset: u32 = 0;
        let mut y_offset: u32 = 0;
        let mut row_height: u32 = 0;
        let mut rects = Vec::new();

        for i in 0..100u32 {
            let w = (i % 5 + 1) * 8;
            let h = (i % 3 + 1) * 12;

            if x_offset + w > atlas_size {
                x_offset = 0;
                y_offset += row_height;
                row_height = 0;
            }
            assert!(y_offset + h <= atlas_size, "Atlas overflow at glyph {}", i);

            let u0 = x_offset as f32 / atlas_size as f32;
            let v0 = y_offset as f32 / atlas_size as f32;
            let u1 = (x_offset + w) as f32 / atlas_size as f32;
            let v1 = (y_offset + h) as f32 / atlas_size as f32;

            x_offset += w;
            row_height = row_height.max(h);
            rects.push((u0, v0, u1, v1));

            for (j, &(pu0, pv0, pu1, pv1)) in rects.iter().enumerate().take(rects.len() - 1) {
                let overlap = !(u0 >= pu1 || u1 <= pu0 || v0 >= pv1 || v1 <= pv0);
                assert!(!overlap, "Overlap between glyph {} and {}", i, j);
            }
        }
    }
}
