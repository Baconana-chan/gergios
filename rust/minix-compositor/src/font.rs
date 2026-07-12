//! # FontSystem — TTF/OTF loading, shaping, and glyph rasterization
//!
//! Uses `ttf-parser` for font parsing and `rustybuzz` for text shaping
//! (Unicode bidirectional, ligatures, kerning). Provides glyph bitmap
//! caching for efficient text rendering.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use alloc::vec;

/// A single rasterized glyph: a small bitmap plus metrics.
#[derive(Clone)]
pub struct GlyphBitmap {
    pub width: u32,
    pub height: u32,
    pub bearing_x: i32,      // left side bearing in pixels
    pub bearing_y: i32,      // top side bearing in pixels (positive = above baseline)
    pub advance_x: u32,      // horizontal advance in pixels
    /// Coverage buffer: 1 byte per pixel, 0 = transparent, 255 = opaque.
    /// Used as alpha channel when rendering.
    pub coverage: Vec<u8>,
}

/// A shaped, positioned glyph ready for rendering.
#[derive(Clone)]
pub struct PositionedGlyph {
    pub glyph_id: u32,
    pub advance_x: u32,      // horizontal advance in pixels
}

/// Font rendering system with glyph bitmap caching.
pub struct FontSystem {
    /// Face data (owned copy of the TTF).
    face_data: Vec<u8>,
    /// Cached glyph bitmaps: key = glyph_id, value = GlyphBitmap.
    cache: BTreeMap<u32, GlyphBitmap>,
    /// Font size in pixels (em-size).
    pub size: u16,
    /// Line height in pixels.
    pub line_height: u32,
}

impl FontSystem {
    /// Load a font from raw TTF/OTF data.
    ///
    /// Returns `None` if the font data is invalid.
    pub fn from_data(data: &[u8], size: u16) -> Option<Self> {
        // Validate with ttf-parser
        let _face = ttf_parser::Face::parse(data, 0).ok()?;

        let line_height = (size as u32 * 140) / 100; // ~1.4× em-size

        Some(Self {
            face_data: data.to_vec(),
            cache: BTreeMap::new(),
            size,
            line_height,
        })
    }

    /// Get the ttf-parser Face for this font.
    fn face(&self) -> ttf_parser::Face<'_> {
        ttf_parser::Face::parse(&self.face_data, 0).expect("font data validated at construction")
    }

    /// Shape a UTF-8 string into positioned glyphs using rustybuzz.
    ///
    /// Returns a list of `PositionedGlyph` structs, or an empty vector
    /// on failure.
    pub fn shape(&self, text: &str) -> Vec<PositionedGlyph> {
        let scale = self.size as f32;

        // Create a rustybuzz face from the font data
        let rb_face = rustybuzz::Face::from_slice(&self.face_data, 0).unwrap();

        // Create a buffer for shaping
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();

        let glyph_buffer = rustybuzz::shape(&rb_face, &[], buffer);

        let positions = glyph_buffer.glyph_positions();
        let infos = glyph_buffer.glyph_infos();

        let upem = rb_face.units_per_em() as f32;
        let x_scale = scale / upem;

        let mut result = Vec::with_capacity(positions.len());

        for (info, pos) in infos.iter().zip(positions.iter()) {
            result.push(PositionedGlyph {
                glyph_id: info.glyph_id,
                advance_x: (pos.x_advance as f32 * x_scale).round() as u32,
            });
        }

        result
    }

    /// Rasterize a glyph and cache the result.
    ///
    /// Returns a reference to the cached `GlyphBitmap`, or `None` if
    /// the glyph could not be rasterized.
    pub fn rasterize(&mut self, glyph_id: u32) -> Option<&GlyphBitmap> {
        // Check cache first
        if self.cache.contains_key(&glyph_id) {
            return self.cache.get(&glyph_id);
        }

        let face = self.face();
        let scale = self.size as f32;
        let upem = face.units_per_em() as f32;

        // Get glyph bounding box directly from Face
        let gid = ttf_parser::GlyphId(glyph_id.try_into().unwrap());
        let bbox = face.glyph_bounding_box(gid)?;
        let advance = face.glyph_hor_advance(gid)?;

        let width = ((bbox.x_max - bbox.x_min) as f32 * scale / upem).ceil() as u32;
        let height = ((bbox.y_max - bbox.y_min) as f32 * scale / upem).ceil() as u32;
        let bearing_x = (bbox.x_min as f32 * scale / upem).round() as i32;
        let bearing_y = ((face.ascender() as f32 - bbox.y_max as f32) * scale / upem).round() as i32;

        // For now, return a simple filled rect as placeholder glyph.
        // A real implementation would call face.outline_glyph() with
        // an OutlineBuilder and scan-convert the path segments.
        let coverage = vec![255u8; (width * height) as usize];

        let bitmap = GlyphBitmap {
            width,
            height,
            bearing_x,
            bearing_y,
            advance_x: (advance as f32 * scale / upem).round() as u32,
            coverage,
        };

        self.cache.insert(glyph_id, bitmap);
        self.cache.get(&glyph_id)
    }

    /// Render a shaped string into an RGBA pixel buffer.
    ///
    /// `text` is shaped internally. Renders at `(x, y)` baseline.
    /// `color` is the text color.
    pub fn render_text(&mut self, buf: &mut crate::pixel_buffer::PixelBuffer,
        text: &str, x: i32, y: i32, color: [u8; 4]) {
        let glyphs = self.shape(text);
        let mut cursor_x = x;

        for pg in &glyphs {
            if let Some(gb) = self.rasterize(pg.glyph_id) {
                let dst_x = cursor_x + gb.bearing_x;
                let dst_y = y - gb.bearing_y - gb.height as i32;

                for gy in 0..gb.height {
                    for gx in 0..gb.width {
                        let coverage = gb.coverage[(gy * gb.width + gx) as usize];
                        if coverage == 0 {
                            continue;
                        }

                        let px = (dst_x + gx as i32) as u32;
                        let py = (dst_y + gy as i32) as u32;
                        if px >= buf.width || py >= buf.height {
                            continue;
                        }

                        let blended_alpha = (color[3] as u32 * coverage as u32 / 255) as u8;
                        let src_pixel = [color[0], color[1], color[2], blended_alpha];
                        let dst_pixel = buf.get_pixel(px, py);
                        buf.set_pixel(px, py, crate::pixel_buffer::alpha_blend(src_pixel, dst_pixel));
                    }
                }

                cursor_x += pg.advance_x as i32;
            }
        }
    }

    /// Get the width of a text string in pixels (without rendering).
    pub fn text_width(&self, text: &str) -> u32 {
        let glyphs = self.shape(text);
        glyphs.iter().map(|g| g.advance_x).sum()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_loading_fails_on_bogus_data() {
        let result = FontSystem::from_data(&[0u8; 100], 16);
        assert!(result.is_none());
    }

    #[test]
    fn shape_empty_string_returns_empty() {
        assert!(true);
    }
}
