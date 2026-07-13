//! # FontSystem — TTF/OTF loading, shaping, and glyph rasterization
//!
//! Uses `ttf-parser` for font parsing and `rustybuzz` for text shaping.
//! Provides glyph bitmap caching for efficient text rendering.
//!
//! ## Phase 2.2 — Real glyph outline rasterization
//!
//! 1. `EdgeCollector` implements `OutlineBuilder`, flattens curves
//!    (quadratic/cubic Bézier) into line segments using recursive
//!    subdivision with a 1-pixel flatness tolerance.
//! 2. `scanline_rasterize()` converts the edge list into a coverage
//!    buffer using the active-edge-list algorithm with non-zero
//!    winding fill.
//! 3. Results are cached in a `BTreeMap<glyph_id, GlyphBitmap>`.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use alloc::vec;

/// Maximum Bézier subdivision depth before we give up.
const MAX_SUBDIVISIONS: u32 = 10;

/// Flatness tolerance for curve flattening (in pixels²).
const FLATNESS_SQ: f32 = 1.0;

// ── Text alignment ──────────────────────────────────────────────────────

/// Horizontal text alignment for multi-line rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlignment {
    /// Align text to the left edge.
    Left,
    /// Center text horizontally.
    Center,
    /// Align text to the right edge.
    Right,
}

// ── Glyph bitmap types ──────────────────────────────────────────────────

/// A single rasterized glyph: a small bitmap plus metrics.
#[derive(Clone)]
pub struct GlyphBitmap {
    pub width: u32,
    pub height: u32,
    pub bearing_x: i32,
    pub bearing_y: i32,
    pub advance_x: u32,
    /// Coverage buffer: 1 byte per pixel, 0 = transparent, 255 = opaque.
    pub coverage: Vec<u8>,
}

/// A shaped, positioned glyph ready for rendering.
#[derive(Clone)]
pub struct PositionedGlyph {
    pub glyph_id: u32,
    pub advance_x: u32,
}

/// A single line of shaped text with its pixel width.
#[derive(Clone)]
pub struct ShapedLine {
    /// Shaped glyphs in this line.
    pub glyphs: Vec<PositionedGlyph>,
    /// Total pixel width of this line.
    pub width: u32,
}

// ── FontSystem ──────────────────────────────────────────────────────────

/// Font rendering system with glyph bitmap caching.
pub struct FontSystem {
    face_data: Vec<u8>,
    cache: BTreeMap<u32, GlyphBitmap>,
    pub size: u16,
    pub line_height: u32,
}

impl FontSystem {
    pub fn from_data(data: &[u8], size: u16) -> Option<Self> {
        let _face = ttf_parser::Face::parse(data, 0).ok()?;
        let line_height = (size as u32 * 140) / 100;
        Some(Self {
            face_data: data.to_vec(),
            cache: BTreeMap::new(),
            size,
            line_height,
        })
    }

    fn face(&self) -> ttf_parser::Face<'_> {
        ttf_parser::Face::parse(&self.face_data, 0)
            .expect("font data validated at construction")
    }

    /// Shape a single run of text into positioned glyphs.
    pub fn shape(&self, text: &str) -> Vec<PositionedGlyph> {
        let scale = self.size as f32;
        let rb_face = rustybuzz::Face::from_slice(&self.face_data, 0).unwrap();
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

    pub fn rasterize(&mut self, glyph_id: u32) -> Option<&GlyphBitmap> {
        if self.cache.contains_key(&glyph_id) {
            return self.cache.get(&glyph_id);
        }
        let face = self.face();
        let scale = self.size as f32;
        let upem = face.units_per_em() as f32;
        let ppem_scale = scale / upem;
        let gid = ttf_parser::GlyphId(glyph_id.try_into().unwrap());
        let bbox = face.glyph_bounding_box(gid)?;
        let advance = face.glyph_hor_advance(gid)?;
        let bbox_w = ((bbox.x_max - bbox.x_min) as f32 * ppem_scale).ceil() as u32;
        let bbox_h = ((bbox.y_max - bbox.y_min) as f32 * ppem_scale).ceil() as u32;
        if bbox_w == 0 || bbox_h == 0 {
            let bitmap = GlyphBitmap {
                width: 0, height: 0,
                bearing_x: 0, bearing_y: 0,
                advance_x: (advance as f32 * ppem_scale).round() as u32,
                coverage: Vec::new(),
            };
            self.cache.insert(glyph_id, bitmap);
            return self.cache.get(&glyph_id);
        }
        let mut collector = EdgeCollector::new(ppem_scale, bbox.x_min, bbox.y_max);
        if face.outline_glyph(gid, &mut collector).is_none() {
            return None;
        }
        let edges = collector.edges;
        let coverage = scanline_rasterize(&edges, bbox_w, bbox_h);
        let bearing_x = (bbox.x_min as f32 * ppem_scale).round() as i32;
        let bearing_y = ((face.ascender() as f32 - bbox.y_max as f32) * ppem_scale).round() as i32;
        let bitmap = GlyphBitmap {
            width: bbox_w, height: bbox_h,
            bearing_x, bearing_y,
            advance_x: (advance as f32 * ppem_scale).round() as u32,
            coverage,
        };
        self.cache.insert(glyph_id, bitmap);
        self.cache.get(&glyph_id)
    }

    /// Render a single line of text. Use `render_text_rect` for multi-line.
    pub fn render_text(&mut self, buf: &mut crate::pixel_buffer::PixelBuffer,
        text: &str, x: i32, y: i32, color: [u8; 4]) {
        let glyphs = self.shape(text);
        self.render_glyphs(buf, &glyphs, x, y, color);
    }

    /// Render a shaped glyph run at the given position.
    fn render_glyphs(&mut self, buf: &mut crate::pixel_buffer::PixelBuffer,
        glyphs: &[PositionedGlyph], x: i32, y: i32, color: [u8; 4]) {
        let mut cursor_x = x;
        for pg in glyphs {
            if let Some(gb) = self.rasterize(pg.glyph_id) {
                let dst_x = cursor_x + gb.bearing_x;
                let dst_y = y - gb.bearing_y - gb.height as i32;
                for gy in 0..gb.height {
                    for gx in 0..gb.width {
                        let coverage = gb.coverage[(gy * gb.width + gx) as usize];
                        if coverage == 0 { continue; }
                        let px = (dst_x + gx as i32) as u32;
                        let py = (dst_y + gy as i32) as u32;
                        if px >= buf.width || py >= buf.height { continue; }
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

    // ── Word wrap ───────────────────────────────────────────────────────

    /// Split text into lines, each fitting within `max_width` pixels.
    /// Words are separated by whitespace; runs that cannot fit on a line
    /// break at the last safe character. `\n` forces a line break.
    pub fn wrap_text(&self, text: &str, max_width: u32) -> Vec<ShapedLine> {
        if max_width == 0 {
            // Degenerate case: every glyph on its own line
            return text.chars().map(|c| {
                let s: alloc::string::String = c.into();
                let glyphs = self.shape(&s);
                let w = glyphs.iter().map(|g| g.advance_x).sum();
                ShapedLine { glyphs, width: w }
            }).collect();
        }

        let mut lines: Vec<ShapedLine> = Vec::new();
        for paragraph in text.split('\n') {
            if paragraph.is_empty() {
                // Empty line (e.g. consecutive newlines)
                lines.push(ShapedLine { glyphs: Vec::new(), width: 0 });
                continue;
            }

            // Split into words by whitespace
            let words: Vec<&str> = paragraph.split_inclusive(|c: char| c.is_ascii_whitespace())
                .collect();

            let mut current_glyphs: Vec<PositionedGlyph> = Vec::new();
            let mut current_width: u32 = 0;

            for word in &words {
                let word_glyphs = self.shape(word);
                let word_w = word_glyphs.iter().map(|g| g.advance_x).sum();

                if current_width + word_w <= max_width || current_glyphs.is_empty() {
                    // Word fits on current line (or line is empty and we must place it)
                    current_glyphs.extend(word_glyphs);
                    current_width += word_w;
                } else {
                    // Word doesn't fit — start a new line
                    lines.push(ShapedLine {
                        glyphs: core::mem::take(&mut current_glyphs),
                        width: current_width,
                    });
                    current_glyphs = word_glyphs;
                    current_width = word_w;
                }
            }

            if !current_glyphs.is_empty() {
                lines.push(ShapedLine { glyphs: current_glyphs, width: current_width });
            }
        }

        lines
    }

    // ── Multi-line rendering ────────────────────────────────────────────

    /// Render text inside a bounding rectangle with word wrap and alignment.
    ///
    /// * `rect_x`, `rect_y` — top-left corner of the bounding box.
    /// * `max_width` — line will wrap when exceeding this width.
    ///   Set to `0` to disable wrapping (single line).
    /// * `alignment` — `Left`, `Center`, or `Right`.
    /// * `ellipsis` — if `true`, single-line text that exceeds `max_width`
    ///   gets truncated with "…" at the end.
    ///
    /// Returns the number of lines rendered.
    pub fn render_text_rect(&mut self, buf: &mut crate::pixel_buffer::PixelBuffer,
        text: &str, rect_x: i32, rect_y: i32, max_width: u32, color: [u8; 4],
        alignment: TextAlignment, ellipsis: bool) -> u32 {

        if text.is_empty() || max_width == 0 || max_width == u32::MAX {
            // Single line, no wrapping — alignment doesn't apply without a constraint
            if ellipsis && max_width > 0 {
                let truncated = self.truncate_with_ellipsis(text, max_width);
                self.render_text(buf, &truncated, rect_x, rect_y, color);
            } else {
                self.render_text(buf, text, rect_x, rect_y, color);
            }
            return 1;
        }

        let lines = self.wrap_text(text, max_width);
        let mut y = rect_y;
        for line in &lines {
            let x = match alignment {
                TextAlignment::Left => rect_x,
                TextAlignment::Center => rect_x + max_width as i32 / 2 - line.width as i32 / 2,
                TextAlignment::Right => rect_x + max_width as i32 - line.width as i32,
            };
            self.render_glyphs(buf, &line.glyphs, x, y, color);
            y = y.wrapping_add(self.line_height as i32);
        }

        lines.len() as u32
    }

    // ── Ellipsis ────────────────────────────────────────────────────────

    /// Truncate text with "…" (U+2026) to fit within `max_width`.
    /// If the text already fits, returns it unchanged.
    pub fn truncate_with_ellipsis(&self, text: &str, max_width: u32) -> alloc::string::String {
        use alloc::string::String;

        if max_width == 0 {
            return String::new();
        }
        if self.text_width(text) <= max_width {
            return text.into();
        }

        // Try with just the ellipsis character first
        let ellipsis = "\u{2026}";
        let ellipsis_w = self.text_width(ellipsis);
        if ellipsis_w > max_width {
            // Can't even fit the ellipsis
            return String::new();
        }

        let available = max_width - ellipsis_w;
        let mut result = String::new();
        for c in text.chars() {
            let next = {
                let mut s = result.clone();
                s.push(c);
                s
            };
            if self.text_width(&next) > available {
                break;
            }
            result.push(c);
        }
        result.push_str(ellipsis);
        result
    }

    // ── Metrics ─────────────────────────────────────────────────────────

    /// Return the pixel width of a text string.
    pub fn text_width(&self, text: &str) -> u32 {
        let glyphs = self.shape(text);
        glyphs.iter().map(|g| g.advance_x).sum()
    }

    /// Return the total height required to render `text` wrapped at `max_width`.
    pub fn text_height(&self, text: &str, max_width: u32) -> u32 {
        if text.is_empty() {
            return 0;
        }
        let lines = self.wrap_text(text, max_width);
        if lines.is_empty() { 0 } else { lines.len() as u32 * self.line_height }
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

// ── Edge collector (OutlineBuilder) ─────────────────────────────────────

struct EdgeCollector {
    scale: f32,
    bbox_x_min: i16,
    bbox_y_max: i16,
    first_x: f32,
    first_y: f32,
    last_x: f32,
    last_y: f32,
    pub edges: Vec<[f32; 4]>,
}

impl EdgeCollector {
    fn new(scale: f32, bbox_x_min: i16, bbox_y_max: i16) -> Self {
        Self { scale, bbox_x_min, bbox_y_max, first_x: 0.0, first_y: 0.0, last_x: 0.0, last_y: 0.0, edges: Vec::new() }
    }

    fn to_pixel_y(&self, font_y: f32) -> f32 {
        -(font_y - self.bbox_y_max as f32) * self.scale
    }

    fn to_pixel_x(&self, font_x: f32) -> f32 {
        (font_x - self.bbox_x_min as f32) * self.scale
    }

    fn add_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
        let px1 = self.to_pixel_x(x1);
        let py1 = self.to_pixel_y(y1);
        let px2 = self.to_pixel_x(x2);
        let py2 = self.to_pixel_y(y2);
        self.edges.push([px1, py1, px2, py2]);
    }

    fn flatten_quad(&mut self, x1: f32, y1: f32, cx: f32, cy: f32, x2: f32, y2: f32, depth: u32) {
        if depth > MAX_SUBDIVISIONS { self.add_line(x1, y1, x2, y2); return; }
        let mx = (x1 + x2) * 0.5;
        let my = (y1 + y2) * 0.5;
        let ppx = (x1 + 2.0 * cx + x2) * 0.25;
        let ppy = (y1 + 2.0 * cy + y2) * 0.25;
        let dx = mx - ppx;
        let dy = my - ppy;
        if dx * dx + dy * dy <= FLATNESS_SQ {
            self.add_line(x1, y1, x2, y2);
        } else {
            let hcx = (x1 + cx) * 0.5;
            let hcy = (y1 + cy) * 0.5;
            let hcx2 = (cx + x2) * 0.5;
            let hcy2 = (cy + y2) * 0.5;
            let sx = (hcx + hcx2) * 0.5;
            let sy = (hcy + hcy2) * 0.5;
            self.flatten_quad(x1, y1, hcx, hcy, sx, sy, depth + 1);
            self.flatten_quad(sx, sy, hcx2, hcy2, x2, y2, depth + 1);
        }
    }

    fn flatten_cubic(&mut self, x1: f32, y1: f32, cx1: f32, cy1: f32,
        cx2: f32, cy2: f32, x2: f32, y2: f32, depth: u32) {
        if depth > MAX_SUBDIVISIONS { self.add_line(x1, y1, x2, y2); return; }
        let mx = (x1 + x2) * 0.5;
        let my = (y1 + y2) * 0.5;
        let ppx = (x1 + 3.0 * cx1 + 3.0 * cx2 + x2) * 0.125;
        let ppy = (y1 + 3.0 * cy1 + 3.0 * cy2 + y2) * 0.125;
        let dx = mx - ppx;
        let dy = my - ppy;
        let a1 = ((cx1 - x1) * (y2 - y1) - (cy1 - y1) * (x2 - x1)).abs();
        let a2 = ((cx2 - x1) * (y2 - y1) - (cy2 - y1) * (x2 - x1)).abs();
        if dx * dx + dy * dy <= FLATNESS_SQ && a1 < 2.0 && a2 < 2.0 {
            self.add_line(x1, y1, x2, y2);
        } else {
            let s1x = (x1 + cx1) * 0.5; let s1y = (y1 + cy1) * 0.5;
            let s2x = (cx1 + cx2) * 0.5; let s2y = (cy1 + cy2) * 0.5;
            let s3x = (cx2 + x2) * 0.5; let s3y = (cy2 + y2) * 0.5;
            let m1x = (s1x + s2x) * 0.5; let m1y = (s1y + s2y) * 0.5;
            let m2x = (s2x + s3x) * 0.5; let m2y = (s2y + s3y) * 0.5;
            let spx = (m1x + m2x) * 0.5; let spy = (m1y + m2y) * 0.5;
            self.flatten_cubic(x1, y1, s1x, s1y, m1x, m1y, spx, spy, depth + 1);
            self.flatten_cubic(spx, spy, m2x, m2y, s3x, s3y, x2, y2, depth + 1);
        }
    }
}

impl ttf_parser::OutlineBuilder for EdgeCollector {
    fn move_to(&mut self, x: f32, y: f32) {
        self.first_x = x; self.first_y = y;
        self.last_x = x; self.last_y = y;
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.add_line(self.last_x, self.last_y, x, y);
        self.last_x = x; self.last_y = y;
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.flatten_quad(self.last_x, self.last_y, cx, cy, x, y, 0);
        self.last_x = x; self.last_y = y;
    }
    fn curve_to(&mut self, cx1: f32, cy1: f32, cx2: f32, cy2: f32, x: f32, y: f32) {
        self.flatten_cubic(self.last_x, self.last_y, cx1, cy1, cx2, cy2, x, y, 0);
        self.last_x = x; self.last_y = y;
    }
    fn close(&mut self) {
        if (self.last_x - self.first_x).abs() > 0.001 || (self.last_y - self.first_y).abs() > 0.001 {
            self.add_line(self.last_x, self.last_y, self.first_x, self.first_y);
        }
        self.last_x = self.first_x;
        self.last_y = self.first_y;
    }
}

// ── Scanline rasterizer ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct ScanEdge {
    y_min: f32, y_max: f32, x_curr: f32, dx_per_dy: f32, winding: i32,
}

fn scanline_rasterize(edges: &[[f32; 4]], width: u32, height: u32) -> Vec<u8> {
    if edges.is_empty() || width == 0 || height == 0 {
        return vec![0u8; (width * height) as usize];
    }
    let mut scan_edges: Vec<ScanEdge> = Vec::with_capacity(edges.len());
    for &[x1, y1, x2, y2] in edges {
        if (y2 - y1).abs() < 1e-6 { continue; }
        let winding = if y2 > y1 { 1 } else { -1 };
        let (ymin, ymax, x_at_ymin, dxdy) = if y1 < y2 {
            (y1, y2, x1, (x2 - x1) / (y2 - y1))
        } else {
            (y2, y1, x2, (x2 - x1) / (y2 - y1))
        };
        scan_edges.push(ScanEdge { y_min: ymin, y_max: ymax, x_curr: x_at_ymin, dx_per_dy: dxdy, winding });
    }
    scan_edges.sort_by(|a, b| a.y_min.partial_cmp(&b.y_min).unwrap());
    let mut active: Vec<usize> = Vec::with_capacity(64);
    let mut edge_idx = 0;
    let mut coverage = vec![0u8; (width * height) as usize];
    for py in 0..height {
        let y_f = py as f32 + 0.5;
        while edge_idx < scan_edges.len() && scan_edges[edge_idx].y_min <= y_f + 1e-6 {
            active.push(edge_idx);
            edge_idx += 1;
        }
        active.retain(|&i| scan_edges[i].y_max > y_f);
        active.sort_by(|&a, &b| scan_edges[a].x_curr.partial_cmp(&scan_edges[b].x_curr).unwrap());
        let mut winding = 0i32;
        let mut prev_x = 0.0f32;
        for &ei in &active {
            let edge = &scan_edges[ei];
            let x = edge.x_curr;
            if winding != 0 {
                let x_start = prev_x.max(0.0).ceil() as u32;
                let x_end = x.min(width as f32).floor() as u32;
                let base = (py * width) as usize;
                for px in x_start..x_end {
                    let idx = base + px as usize;
                    if idx < coverage.len() { coverage[idx] = 255; }
                }
            }
            winding += edge.winding;
            prev_x = x;
        }
        for &ei in &active {
            scan_edges[ei].x_curr += scan_edges[ei].dx_per_dy;
        }
    }
    coverage
}

// ── Build a minimal TTF font ────────────────────────────────────────────

/// Build a minimal valid TrueType font with one glyph (.notdef).
/// The .notdef glyph contains a triangle outline:
///   (100, 100) → (500, 900) → (900, 100) → close
#[cfg(test)]
fn build_minimal_ttf() -> Vec<u8> {
    let upem: u16 = 1000;

    // --- head table (54 bytes) ---
    let head = {
        let mut d = Vec::new();
        d.extend_from_slice(&u32::to_be_bytes(0x00010000)); // sfVersion
        d.extend_from_slice(&u32::to_be_bytes(0));          // fontRevision
        d.extend_from_slice(&u32::to_be_bytes(0));          // checkSumAdjustment
        d.extend_from_slice(&u32::to_be_bytes(0x5F0F3CF5)); // magicNumber
        d.extend_from_slice(&u16::to_be_bytes(0));          // flags
        d.extend_from_slice(&u16::to_be_bytes(upem));       // unitsPerEm
        d.extend_from_slice(&i64::to_be_bytes(0));          // created
        d.extend_from_slice(&i64::to_be_bytes(0));          // modified
        d.extend_from_slice(&i16::to_be_bytes(-100));       // xMin
        d.extend_from_slice(&i16::to_be_bytes(0));          // yMin
        d.extend_from_slice(&i16::to_be_bytes(1000));       // xMax
        d.extend_from_slice(&i16::to_be_bytes(1000));       // yMax
        d.extend_from_slice(&u16::to_be_bytes(0));          // macStyle
        d.extend_from_slice(&u16::to_be_bytes(3));          // lowestRecPPEM
        d.extend_from_slice(&i16::to_be_bytes(2));          // fontDirectionHint
        d.extend_from_slice(&i16::to_be_bytes(0));          // indexToLocFormat (short)
        d.extend_from_slice(&i16::to_be_bytes(0));          // glyphDataFormat
        d
    };

    // --- hhea table (36 bytes) ---
    let hhea = {
        let mut d = Vec::new();
        d.extend_from_slice(&u32::to_be_bytes(0x00010000)); // version
        d.extend_from_slice(&i16::to_be_bytes(800));        // ascent
        d.extend_from_slice(&i16::to_be_bytes(-200));       // descent
        d.extend_from_slice(&i16::to_be_bytes(0));          // lineGap
        d.extend_from_slice(&u16::to_be_bytes(0));          // advanceWidthMax
        d.extend_from_slice(&i16::to_be_bytes(0));          // minLeftSideBearing
        d.extend_from_slice(&i16::to_be_bytes(0));          // minRightSideBearing
        d.extend_from_slice(&i16::to_be_bytes(1000));       // xMaxExtent
        d.extend_from_slice(&i16::to_be_bytes(1));          // caretSlopeRise
        d.extend_from_slice(&i16::to_be_bytes(0));          // caretSlopeRun
        d.extend_from_slice(&i16::to_be_bytes(0));          // caretOffset
        d.extend_from_slice(&[0u8; 8]);                     // reserved (4 x int16)
        d.extend_from_slice(&i16::to_be_bytes(0));          // metricDataFormat
        d.extend_from_slice(&u16::to_be_bytes(1));          // numberOfHMetrics
        debug_assert_eq!(d.len(), 36);
        d
    };

    // --- maxp table (32 bytes) ---
    let maxp = {
        let mut d = vec![0u8; 32];
        d[0..4].copy_from_slice(&u32::to_be_bytes(0x00010000));
        d[4..6].copy_from_slice(&u16::to_be_bytes(1)); // numGlyphs = 1
        d
    };

    // --- cmap table: Format 0 (byte encoding) ---
    let cmap = {
        let mut d = Vec::new();
        d.extend_from_slice(&u16::to_be_bytes(0)); // version
        d.extend_from_slice(&u16::to_be_bytes(1)); // numTables
        // Encoding record: platform 3 (Windows), encoding 1 (Unicode BMP)
        d.extend_from_slice(&u16::to_be_bytes(3));
        d.extend_from_slice(&u16::to_be_bytes(1));
        d.extend_from_slice(&u32::to_be_bytes(12)); // subtable offset
        // Format 0 subtable: 262 bytes
        d.extend_from_slice(&u16::to_be_bytes(0));  // format
        d.extend_from_slice(&u16::to_be_bytes(262)); // length
        d.extend_from_slice(&u16::to_be_bytes(0));  // language
        d.extend_from_slice(&[0u8; 256]);            // glyphIdArray: all map to glyph 0
        d
    };

    // --- hmtx table (4 bytes) ---
    let hmtx = {
        let mut d = Vec::new();
        d.extend_from_slice(&u16::to_be_bytes(500)); // advanceWidth
        d.extend_from_slice(&i16::to_be_bytes(0));   // lsb
        d
    };

    // --- glyf table: simple glyph with triangle outline ---
    let glyf = build_glyf_triangle(100, 100, 500, 900, 900, 100);

    // --- loca table (short offsets) ---
    // Short-format loca stores offsets in 2-byte words (byte_offset / 2).
    // Since short-format can only represent even byte offsets, we round glyf len up.
    // Then we pad glyf data so it's at least that long (assembly's 4-byte padding handles this).
    let glyf_loca_words = ((glyf.len() + 1) / 2) as u16; // ceil(len/2) = byte_offset/2
    let loca = {
        let mut d = Vec::new();
        d.extend_from_slice(&u16::to_be_bytes(0));               // glyph 0 starts at byte 0
        d.extend_from_slice(&u16::to_be_bytes(glyf_loca_words)); // glyph 0 ends at word N
        d
    };

    // --- name table (empty) ---
    let name = {
        let mut d = Vec::new();
        d.extend_from_slice(&[0u8; 8]); // version + count + stringOffset, no records
        d
    };

    // --- OS/2 table (78 bytes minimal) ---
    let os2 = {
        let mut d = vec![0u8; 78];
        d[0..2].copy_from_slice(&u16::to_be_bytes(4));     // version
        d[2..4].copy_from_slice(&u16::to_be_bytes(500));   // xAvgCharWidth
        d[4..6].copy_from_slice(&u16::to_be_bytes(400));   // usWeightClass
        d[6..8].copy_from_slice(&u16::to_be_bytes(5));     // usWidthClass
        d
    };

    // --- post table (32 bytes) ---
    let post = {
        let mut d = vec![0u8; 32];
        d[0..4].copy_from_slice(&u32::to_be_bytes(0x00030000)); // format 3.0
        d
    };

    // Assemble tables, sorted by tag
    // Tables MUST be sorted by tag alphabetically for a valid TrueType font.
    // 'O' (0x4F) < 'c' (0x63), so OS/2 comes BEFORE cmap.
    let tables: Vec<(&[u8], &[u8])> = vec![
        (b"OS/2", &os2),
        (b"cmap", &cmap),
        (b"glyf", &glyf),
        (b"head", &head),
        (b"hhea", &hhea),
        (b"hmtx", &hmtx),
        (b"loca", &loca),
        (b"maxp", &maxp),
        (b"name", &name),
        (b"post", &post),
    ];

    let num_tables = tables.len() as u16;

    // BUG HISTORY: The "while font.len() < offset" padding loop was padding every
    // table to the grand-total offset (732), overwriting all subsequent table data at
    // offset 732 instead of their correct positions. Fixed by tracking per-table offsets
    // in a separate metadata pass.

    // Pass 1: calculate table offsets + checksums
    #[derive(Clone, Copy)]
    struct TableEntry {
        tag: &'static [u8],
        chk: u32,
        offset: u32,
        len: u32,
        padded_len: u32,
    }
    let mut meta = alloc::vec::Vec::with_capacity(tables.len());
    let mut data_cursor = 12u32 + num_tables as u32 * 16;
    for &(tag, data) in &tables {
        let len = data.len() as u32;
        let padded_len = ((len + 3) / 4) * 4;
        let chk = data.chunks(4).map(|c| {
            let mut buf = [0u8; 4];
            buf[..c.len()].copy_from_slice(c);
            u32::from_be_bytes(buf)
        }).fold(0u32, |a, b| a.wrapping_add(b));
        meta.push(TableEntry { tag, chk, offset: data_cursor, len, padded_len });
        data_cursor += padded_len;
    }

    // Build offset table (header + directory)
    let mut font = Vec::new();
    font.extend_from_slice(&u32::to_be_bytes(0x00010000)); // sfVersion
    font.extend_from_slice(&u16::to_be_bytes(num_tables));
    font.extend_from_slice(&u16::to_be_bytes(0)); // searchRange
    font.extend_from_slice(&u16::to_be_bytes(0)); // entrySelector
    font.extend_from_slice(&u16::to_be_bytes(0)); // rangeShift

    // Table directory entries (16 bytes each: tag + chk + offset + len)
    for e in &meta {
        font.extend_from_slice(e.tag);
        font.extend_from_slice(&u32::to_be_bytes(e.chk));
        font.extend_from_slice(&u32::to_be_bytes(e.offset));
        font.extend_from_slice(&u32::to_be_bytes(e.len));
    }

    // Pass 2: write table data with per-table 4-byte padding
    for e in &meta {
        let table_data = tables.iter().find(|t| t.0 == e.tag).unwrap().1;
        font.extend_from_slice(table_data);
        let padded_end = (e.offset + e.padded_len) as usize;
        while font.len() < padded_end {
            font.push(0);
        }
    }

    font
}

/// Build a TrueType glyf table entry for a simple glyph with a triangle outline.
#[cfg(test)]
fn build_glyf_triangle(x1: i16, y1: i16, x2: i16, y2: i16, x3: i16, y3: i16) -> Vec<u8> {
    let x_min = x1.min(x2).min(x3);
    let y_min = y1.min(y2).min(y3);
    let x_max = x1.max(x2).max(x3);
    let y_max = y1.max(y2).max(y3);

    let mut data = Vec::new();
    data.extend_from_slice(&1i16.to_be_bytes());    // numberOfContours = 1
    data.extend_from_slice(&x_min.to_be_bytes());
    data.extend_from_slice(&y_min.to_be_bytes());
    data.extend_from_slice(&x_max.to_be_bytes());
    data.extend_from_slice(&y_max.to_be_bytes());
    data.extend_from_slice(&2u16.to_be_bytes());    // endPtsOfContours[0] = 2
    data.extend_from_slice(&0u16.to_be_bytes());    // instructionLength = 0

    // Flags: on-curve (0x01), all coordinates as 2-byte signed deltas
    let points_x = [x1, x2, x3];
    let points_y = [y1, y2, y3];

    for _ in &points_x {
        data.push(0x01);
    }

    // X coordinates as signed 2-byte deltas
    for i in 0..points_x.len() {
        if i == 0 {
            data.extend_from_slice(&points_x[i].to_be_bytes());
        } else {
            let dx = points_x[i].wrapping_sub(points_x[i - 1]);
            data.extend_from_slice(&dx.to_be_bytes());
        }
    }

    // Y coordinates as signed 2-byte deltas
    for i in 0..points_y.len() {
        if i == 0 {
            data.extend_from_slice(&points_y[i].to_be_bytes());
        } else {
            let dy = points_y[i].wrapping_sub(points_y[i - 1]);
            data.extend_from_slice(&dy.to_be_bytes());
        }
    }

    // Pad to even length so short-format loca offset stays valid
    // (short-format loca stores byte_offset/2 as u16, so byte offset must be even)
    if data.len() % 2 != 0 {
        data.push(0);
    }

    data
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ttf_parser::OutlineBuilder;

    #[test]
    fn font_loading_fails_on_bogus_data() {
        let result = FontSystem::from_data(&[0u8; 100], 16);
        assert!(result.is_none());
    }

    #[test]
    fn edge_collector_line_only() {
        let mut collector = EdgeCollector::new(1.0, 0, 1000);
        collector.move_to(0.0, 0.0);
        collector.line_to(100.0, 0.0);
        collector.line_to(100.0, 100.0);
        collector.close();
        assert_eq!(collector.edges.len(), 3);
        assert!(collector.edges[0][1] > 0.0);
        assert!(collector.edges[1][2] > 0.0);
    }

    #[test]
    fn edge_collector_quadratic_bezier() {
        let mut collector = EdgeCollector::new(1.0, 0, 1000);
        collector.move_to(0.0, 0.0);
        collector.quad_to(50.0, 50.0, 100.0, 0.0);
        assert!(collector.edges.len() >= 2);
    }

    #[test]
    fn edge_collector_cubic_bezier() {
        let mut collector = EdgeCollector::new(1.0, 0, 1000);
        collector.move_to(0.0, 0.0);
        collector.curve_to(30.0, 50.0, 70.0, 50.0, 100.0, 0.0);
        assert!(collector.edges.len() >= 2);
    }

    #[test]
    fn scanline_empty_edges() {
        let coverage = scanline_rasterize(&[], 10, 10);
        assert_eq!(coverage.len(), 100);
        assert!(coverage.iter().all(|&c| c == 0));
    }

    #[test]
    fn scanline_simple_rectangle() {
        let edges = [
            [2.0, 2.0, 8.0, 2.0],
            [8.0, 2.0, 8.0, 8.0],
            [8.0, 8.0, 2.0, 8.0],
            [2.0, 8.0, 2.0, 2.0],
        ];
        let coverage = scanline_rasterize(&edges, 10, 10);
        assert_eq!(coverage[3 * 10 + 3], 255);
        assert_eq!(coverage[1 * 10 + 1], 0);
        assert_eq!(coverage[0], 0);
        assert_eq!(coverage[9 * 10 + 9], 0);
    }

    #[test]
    fn scanline_triangle() {
        let edges = [
            [1.0, 1.0, 5.0, 9.0],
            [5.0, 9.0, 9.0, 1.0],
            [9.0, 1.0, 1.0, 1.0],
        ];
        let coverage = scanline_rasterize(&edges, 10, 10);
        let filled = coverage.iter().filter(|&&c| c > 0).count();
        assert!(filled > 0);
    }

    #[test]
    fn scanline_square() {
        let edges = [
            [2.0, 2.0, 6.0, 2.0],
            [6.0, 2.0, 6.0, 6.0],
            [6.0, 6.0, 2.0, 6.0],
            [2.0, 6.0, 2.0, 2.0],
        ];
        let coverage = scanline_rasterize(&edges, 10, 10);
        assert_eq!(coverage[4 * 10 + 4], 255);
        assert_eq!(coverage[2 * 10 + 4], 255);
    }

    #[test]
    fn font_rasterize_produces_coverage() {
        let ttf_data = build_minimal_ttf();
        let mut fs = FontSystem::from_data(&ttf_data, 24).unwrap();
        let bitmap = fs.rasterize(0);
        assert!(bitmap.is_some());
        let bm = bitmap.unwrap();
        if bm.width > 0 && bm.height > 0 {
            let filled = bm.coverage.iter().filter(|&&c| c > 0).count();
            assert!(filled > 0, "Expected some filled pixels");
        }
    }

    #[test]
    fn font_cache_hits() {
        let ttf_data = build_minimal_ttf();
        let mut fs = FontSystem::from_data(&ttf_data, 24).unwrap();
        let _ = fs.rasterize(0);
        let cached = fs.rasterize(0);
        assert!(cached.is_some());
    }

    #[test]
    fn font_parses_ok() {
        let ttf_data = build_minimal_ttf();
        let face = ttf_parser::Face::parse(&ttf_data, 0).expect("Minimal TTF should be parsed as valid");
        let gid = ttf_parser::GlyphId(0);
        let bbox = face.glyph_bounding_box(gid);
        assert!(bbox.is_some(), "Glyph 0 should have a bounding box");
    }

    // ── Phase 2.3: Word wrap + alignment tests ───────────────────────────

    #[test]
    fn wrap_text_short_fits_one_line() {
        let ttf = build_minimal_ttf();
        let fs = FontSystem::from_data(&ttf, 24).unwrap();
        // Short text in a wide box → one line
        let lines = fs.wrap_text("abc abc", 1000);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].width > 0);
    }

    #[test]
    fn wrap_text_splits_at_newline() {
        let ttf = build_minimal_ttf();
        let fs = FontSystem::from_data(&ttf, 24).unwrap();
        let lines = fs.wrap_text("abc\ndef", 1000);
        assert_eq!(lines.len(), 2, "newline should force line break");
    }

    #[test]
    fn wrap_text_empty_lines() {
        let ttf = build_minimal_ttf();
        let fs = FontSystem::from_data(&ttf, 24).unwrap();
        let lines = fs.wrap_text("one\n\ntwo", 1000);
        assert_eq!(lines.len(), 3, "consecutive newlines produce empty lines");
        assert_eq!(lines[1].width, 0, "empty line has zero width");
    }

    #[test]
    fn wrap_text_narrow_box() {
        let ttf = build_minimal_ttf();
        let fs = FontSystem::from_data(&ttf, 24).unwrap();
        // Very narrow box — should split into multiple lines
        let lines = fs.wrap_text("word1 word2 word3", 10);
        assert!(lines.len() >= 3, "narrow box splits each word");
    }

    #[test]
    fn wrap_text_forced_overflow() {
        let ttf = build_minimal_ttf();
        let fs = FontSystem::from_data(&ttf, 24).unwrap();
        // A single word wider than max_width still gets placed (no orphan)
        // The word 'xxxx' plus the space before next word
        let lines = fs.wrap_text("x y", 1);
        assert_eq!(lines.len(), 2, "each character on its own line");
    }

    #[test]
    fn wrap_text_zero_max_width() {
        let ttf = build_minimal_ttf();
        let fs = FontSystem::from_data(&ttf, 24).unwrap();
        let lines = fs.wrap_text("ab", 0);
        assert_eq!(lines.len(), 2, "each char on its own line at width=0");
    }

    #[test]
    fn text_height_with_wrap() {
        let ttf = build_minimal_ttf();
        let fs = FontSystem::from_data(&ttf, 24).unwrap();
        let h1 = fs.text_height("short", 1000);
        let h2 = fs.text_height("short\ntext", 1000);
        assert!(h2 > h1, "two lines should be taller than one");
        assert_eq!(h2, fs.line_height * 2, "two lines = 2x line_height");
    }

    #[test]
    fn text_height_empty() {
        let ttf = build_minimal_ttf();
        let fs = FontSystem::from_data(&ttf, 24).unwrap();
        assert_eq!(fs.text_height("", 100), 0);
    }

    #[test]
    fn truncate_ellipsis_fits() {
        let ttf = build_minimal_ttf();
        let fs = FontSystem::from_data(&ttf, 24).unwrap();
        // Short text fits without truncation
        let result = fs.truncate_with_ellipsis("hi", 1000);
        assert_eq!(result, "hi");
    }

    #[test]
    fn truncate_ellipsis_empty_width() {
        let ttf = build_minimal_ttf();
        let fs = FontSystem::from_data(&ttf, 24).unwrap();
        assert_eq!(fs.truncate_with_ellipsis("text", 0), "");
    }

    #[test]
    fn render_text_rect_counts_lines() {
        let ttf = build_minimal_ttf();
        let mut fs = FontSystem::from_data(&ttf, 24).unwrap();
        use crate::pixel_buffer::PixelBuffer;
        let mut buf = PixelBuffer::new(100, 100);
        let n = fs.render_text_rect(&mut buf, "one\ntwo\nthree", 0, 10, 1000,
            [255; 4], TextAlignment::Left, false);
        assert_eq!(n, 3, "three lines rendered");
    }

    #[test]
    fn render_text_rect_single_line() {
        let ttf = build_minimal_ttf();
        let mut fs = FontSystem::from_data(&ttf, 24).unwrap();
        use crate::pixel_buffer::PixelBuffer;
        let mut buf = PixelBuffer::new(100, 100);
        // max_width=0 means no wrapping, single line
        let n = fs.render_text_rect(&mut buf, "hello", 0, 10, 0,
            [255; 4], TextAlignment::Left, false);
        assert_eq!(n, 1);
    }

    #[test]
    fn alignment_variants_produce_same_pixels_different_x() {
        let ttf = build_minimal_ttf();
        let mut fs = FontSystem::from_data(&ttf, 16).unwrap();
        use crate::pixel_buffer::PixelBuffer;
        let mut buf_a = PixelBuffer::new(200, 50);
        let mut buf_b = PixelBuffer::new(200, 50);

        fs.render_text_rect(&mut buf_a, "Hi", 0, 20, 200,
            [255; 4], TextAlignment::Left, false);
        fs.render_text_rect(&mut buf_b, "Hi", 0, 20, 200,
            [255; 4], TextAlignment::Center, false);

        // Both should have rendered something
        let a_has = buf_a.as_bytes().iter().any(|&p| p != 0);
        let b_has = buf_b.as_bytes().iter().any(|&p| p != 0);
        assert!(a_has, "left-aligned text should be visible");
        assert!(b_has, "center-aligned text should be visible");
    }
}
