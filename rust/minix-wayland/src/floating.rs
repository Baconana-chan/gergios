//! # Floating Window Manager — Drag, Resize, Snapping
//!
//! Implements Phase 4.2 of the GUI architecture: floating window operations.
//! Manages which windows are in floating mode (vs. tiling), handles interactive
//! move and resize via pointer grab, and provides edge/window snapping.
//!
//! ## Architecture
//!
//! ```text
//! FloatingManager
//!   ├── floating_windows: HashSet<u32>  (xdg_toplevel_ids in floating mode)
//!   ├── drag: Option<DragState>         (active move operation)
//!   ├── resize: Option<ResizeState>     (active resize operation)
//!   └── snap_config: SnapConfig         (edge/window snap settings)
//! ```
//!
//! ## Integration with xdg_toplevel protocol
//!
//! - `xdg_toplevel.move(seat, serial)` → `start_drag()`
//! - `xdg_toplevel.resize(seat, serial, edges)` → `start_resize()`
//! - Pointer button release → `end_drag()` / `end_resize()`
//! - Pointer motion → `on_motion()` (updates window position/size)
//!
//! ## Snapping
//!
//! Windows snap to screen edges and other window edges within the threshold.
//! Snapping is disabled during resize; only move supports snapping.

use alloc::vec::Vec;
use core::fmt;

/// Snap threshold in pixels.
pub const DEFAULT_SNAP_THRESHOLD: i32 = 10;

/// Minimum window dimensions (pixels).
pub const MIN_WINDOW_WIDTH: i32 = 100;
pub const MIN_WINDOW_HEIGHT: i32 = 60;

/// Maximum window dimensions (pixels). 0 = no limit.
pub const MAX_WINDOW_WIDTH: i32 = 0;
pub const MAX_WINDOW_HEIGHT: i32 = 0;

/// Resize edge flags matching xdg_toplevel.resize(edges) enum.
///
/// ```text
/// 0 = none
/// 1 = top
/// 2 = bottom
/// 4 = left
/// 8 = right
/// 3 = top|bottom
/// 5 = top|left
/// 6 = bottom|left
/// 9 = top|right
/// 10 = bottom|right
/// 12 = left|right
/// 15 = all
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResizeEdges {
    pub top: bool,
    pub bottom: bool,
    pub left: bool,
    pub right: bool,
}

impl ResizeEdges {
    /// Parse from xdg_toplevel.resize edges bitmask.
    pub fn from_xdg(edges: u32) -> Self {
        Self {
            top: (edges & 1) != 0,    // XDG_TOPLEVEL_RESIZE_EDGE_TOP
            bottom: (edges & 2) != 0, // XDG_TOPLEVEL_RESIZE_EDGE_BOTTOM
            left: (edges & 4) != 0,   // XDG_TOPLEVEL_RESIZE_EDGE_LEFT
            right: (edges & 8) != 0,  // XDG_TOPLEVEL_RESIZE_EDGE_RIGHT
        }
    }

    /// Returns true if any edge is set.
    pub fn is_active(self) -> bool {
        self.top || self.bottom || self.left || self.right
    }

    /// Returns true if multiple edges are set (corner resize).
    pub fn is_corner(self) -> bool {
        (self.top || self.bottom) && (self.left || self.right)
    }
}

/// Active drag (move) operation state.
///
/// Created when a client sends `xdg_toplevel.move()`, active until
/// the next pointer button release.
#[derive(Debug, Clone)]
pub struct DragState {
    /// The xdg_toplevel object ID being dragged.
    pub xdg_toplevel_id: u32,
    /// Cursor position at the start of drag (compositor coordinates).
    pub start_cursor_x: i32,
    pub start_cursor_y: i32,
    /// Window position at the start of drag.
    pub start_window_x: i32,
    pub start_window_y: i32,
    /// Input serial from the MOVE request.
    pub serial: u32,
}

/// Active resize operation state.
///
/// Created when a client sends `xdg_toplevel.resize()`, active until
/// the next pointer button release.
#[derive(Debug, Clone)]
pub struct ResizeState {
    /// The xdg_toplevel object ID being resized.
    pub xdg_toplevel_id: u32,
    /// Which edges are being dragged.
    pub edges: ResizeEdges,
    /// Cursor position at the start of resize.
    pub start_cursor_x: i32,
    pub start_cursor_y: i32,
    /// Window geometry at the start of resize.
    pub start_window_x: i32,
    pub start_window_y: i32,
    pub start_width: i32,
    pub start_height: i32,
    /// Input serial from the RESIZE request.
    pub serial: u32,
}

/// Snapping configuration.
#[derive(Debug, Clone)]
pub struct SnapConfig {
    /// Snap threshold in pixels.
    pub threshold: i32,
    /// Whether to snap to screen edges.
    pub snap_to_edges: bool,
    /// Whether to snap to other window edges.
    pub snap_to_windows: bool,
}

impl Default for SnapConfig {
    fn default() -> Self {
        Self {
            threshold: DEFAULT_SNAP_THRESHOLD,
            snap_to_edges: true,
            snap_to_windows: false, // disabled by default (complexity)
        }
    }
}

/// Result of a snap computation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapResult {
    /// Adjusted X position after snapping.
    pub x: i32,
    /// Adjusted Y position after snapping.
    pub y: i32,
    /// Whether a horizontal snap was applied.
    pub snapped_h: bool,
    /// Whether a vertical snap was applied.
    pub snapped_v: bool,
}

impl SnapResult {
    /// No snapping applied.
    pub fn none(x: i32, y: i32) -> Self {
        Self { x, y, snapped_h: false, snapped_v: false }
    }
}

/// Floating window manager.
///
/// Tracks which windows are floating (vs. tiled), manages interactive
/// move/resize operations, and provides snapping logic.
///
/// ## Lifecycle
///
/// 1. Client sends `xdg_toplevel.move(seat, serial)` while a button is held.
/// 2. Compositor calls `start_drag()` → `FloatingManager` enters drag mode.
/// 3. On each pointer motion, `on_drag_motion()` updates the window position.
/// 4. On button release, `end_drag()` exits drag mode.
/// 5. Same flow for resize via `start_resize()` / `on_resize_motion()` / `end_resize()`.
pub struct FloatingManager {
    /// Set of xdg_toplevel_ids that are in floating mode.
    pub floating_windows: Vec<u32>,
    /// Active drag (move) operation, if any.
    pub drag: Option<DragState>,
    /// Active resize operation, if any.
    pub resize: Option<ResizeState>,
    /// Snap configuration.
    pub snap_config: SnapConfig,
    /// Screen dimensions (for edge snapping and bounds clamping).
    screen_width: i32,
    screen_height: i32,
}

impl fmt::Debug for FloatingManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FloatingManager")
            .field("floating_count", &self.floating_windows.len())
            .field("drag", &self.drag.is_some())
            .field("resize", &self.resize.is_some())
            .field("snap_threshold", &self.snap_config.threshold)
            .finish()
    }
}

impl FloatingManager {
    /// Create a new floating manager.
    pub fn new(screen_width: i32, screen_height: i32) -> Self {
        Self {
            floating_windows: Vec::new(),
            drag: None,
            resize: None,
            snap_config: SnapConfig::default(),
            screen_width,
            screen_height,
        }
    }

    /// Update screen dimensions (call when output mode changes).
    pub fn set_screen_size(&mut self, width: i32, height: i32) {
        self.screen_width = width;
        self.screen_height = height;
    }

    // ── Floating mode management ─────────────────────────────────

    /// Set a window to floating mode. Returns true if the state changed.
    pub fn set_floating(&mut self, xdg_toplevel_id: u32, floating: bool) -> bool {
        let already = self.floating_windows.contains(&xdg_toplevel_id);
        if floating && !already {
            self.floating_windows.push(xdg_toplevel_id);
            true
        } else if !floating && already {
            self.floating_windows.retain(|&id| id != xdg_toplevel_id);
            true
        } else {
            false
        }
    }

    /// Toggle floating mode for a window. Returns the new state.
    pub fn toggle_floating(&mut self, xdg_toplevel_id: u32) -> bool {
        let is_floating = self.floating_windows.contains(&xdg_toplevel_id);
        self.set_floating(xdg_toplevel_id, !is_floating);
        !is_floating
    }

    /// Check if a window is in floating mode.
    pub fn is_floating(&self, xdg_toplevel_id: u32) -> bool {
        self.floating_windows.contains(&xdg_toplevel_id)
    }

    /// Remove a window from floating tracking (called on window destroy).
    pub fn remove_window(&mut self, xdg_toplevel_id: u32) {
        self.floating_windows.retain(|&id| id != xdg_toplevel_id);
        // Also cancel any active operation on this window
        if self.drag.as_ref().map(|d| d.xdg_toplevel_id) == Some(xdg_toplevel_id) {
            self.drag = None;
        }
        if self.resize.as_ref().map(|r| r.xdg_toplevel_id) == Some(xdg_toplevel_id) {
            self.resize = None;
        }
    }

    /// Get all floating window IDs.
    pub fn floating_ids(&self) -> &[u32] {
        &self.floating_windows
    }

    // ── Drag (move) operations ────────────────────────────────────

    /// Start an interactive drag for the given window.
    ///
    /// Returns true if drag started successfully.
    /// The window is automatically set to floating mode.
    pub fn start_drag(
        &mut self,
        xdg_toplevel_id: u32,
        window_x: i32,
        window_y: i32,
        cursor_x: i32,
        cursor_y: i32,
        serial: u32,
    ) -> bool {
        // Ensure window is floating
        self.set_floating(xdg_toplevel_id, true);

        self.drag = Some(DragState {
            xdg_toplevel_id,
            start_cursor_x: cursor_x,
            start_cursor_y: cursor_y,
            start_window_x: window_x,
            start_window_y: window_y,
            serial,
        });
        true
    }

    /// Process pointer motion during an active drag.
    ///
    /// Returns the new (x, y) position for the window, possibly snapped.
    /// Returns None if no drag is active.
    pub fn on_drag_motion(
        &mut self,
        cursor_x: i32,
        cursor_y: i32,
    ) -> Option<(i32, i32)> {
        let drag = self.drag.as_ref()?;

        let dx = cursor_x - drag.start_cursor_x;
        let dy = cursor_y - drag.start_cursor_y;

        let new_x = drag.start_window_x + dx;
        let new_y = drag.start_window_y + dy;

        // Clamp to screen bounds (keep at least a corner visible)
        let clamped_x = new_x.max(-self.screen_width + MIN_WINDOW_WIDTH)
            .min(self.screen_width - MIN_WINDOW_WIDTH);
        let clamped_y = new_y.max(-self.screen_height + MIN_WINDOW_HEIGHT)
            .min(self.screen_height - MIN_WINDOW_HEIGHT);

        // Apply snapping if enabled
        if self.snap_config.snap_to_edges {
            Some(self.snap_to_edges(clamped_x, clamped_y))
        } else {
            Some((clamped_x, clamped_y))
        }
    }

    /// End the current drag operation.
    pub fn end_drag(&mut self) {
        self.drag = None;
    }

    // ── Resize operations ─────────────────────────────────────────

    /// Start an interactive resize for the given window.
    ///
    /// Returns true if resize started successfully.
    pub fn start_resize(
        &mut self,
        xdg_toplevel_id: u32,
        edges: ResizeEdges,
        window_x: i32,
        window_y: i32,
        window_width: i32,
        window_height: i32,
        cursor_x: i32,
        cursor_y: i32,
        serial: u32,
    ) -> bool {
        self.resize = Some(ResizeState {
            xdg_toplevel_id,
            edges,
            start_cursor_x: cursor_x,
            start_cursor_y: cursor_y,
            start_window_x: window_x,
            start_window_y: window_y,
            start_width: window_width,
            start_height: window_height,
            serial,
        });
        true
    }

    /// Process pointer motion during an active resize.
    ///
    /// Returns the new (x, y, width, height) for the window.
    /// Returns None if no resize is active.
    pub fn on_resize_motion(
        &mut self,
        cursor_x: i32,
        cursor_y: i32,
    ) -> Option<(i32, i32, i32, i32)> {
        let resize = self.resize.as_ref()?;

        let dx = cursor_x - resize.start_cursor_x;
        let dy = cursor_y - resize.start_cursor_y;

        let mut new_x = resize.start_window_x;
        let mut new_y = resize.start_window_y;
        let mut new_w = resize.start_width;
        let mut new_h = resize.start_height;

        // Apply edge adjustments
        if resize.edges.left {
            new_x = resize.start_window_x + dx;
            new_w = resize.start_width - dx;
        }
        if resize.edges.right {
            new_w = resize.start_width + dx;
            // X stays the same for right-edge resize
        }
        if resize.edges.top {
            new_y = resize.start_window_y + dy;
            new_h = resize.start_height - dy;
        }
        if resize.edges.bottom {
            new_h = resize.start_height + dy;
            // Y stays the same for bottom-edge resize
        }

        // Enforce minimum size
        if new_w < MIN_WINDOW_WIDTH {
            if resize.edges.left {
                // Adjust X so that the right edge stays in place
                new_x = resize.start_window_x + resize.start_width - MIN_WINDOW_WIDTH;
            }
            new_w = MIN_WINDOW_WIDTH;
        }
        if new_h < MIN_WINDOW_HEIGHT {
            if resize.edges.top {
                // Adjust Y so that the bottom edge stays in place
                new_y = resize.start_window_y + resize.start_height - MIN_WINDOW_HEIGHT;
            }
            new_h = MIN_WINDOW_HEIGHT;
        }

        // Enforce maximum size (if non-zero)
        if MAX_WINDOW_WIDTH > 0 && new_w > MAX_WINDOW_WIDTH {
            if resize.edges.left {
                new_x = resize.start_window_x + resize.start_width - MAX_WINDOW_WIDTH;
            }
            new_w = MAX_WINDOW_WIDTH;
        }
        if MAX_WINDOW_HEIGHT > 0 && new_h > MAX_WINDOW_HEIGHT {
            if resize.edges.top {
                new_y = resize.start_window_y + resize.start_height - MAX_WINDOW_HEIGHT;
            }
            new_h = MAX_WINDOW_HEIGHT;
        }

        Some((new_x, new_y, new_w, new_h))
    }

    /// End the current resize operation.
    pub fn end_resize(&mut self) {
        self.resize = None;
    }

    // ── Snapping logic ────────────────────────────────────────────

    /// Snap a position to screen edges.
    ///
    /// Snaps the window's left/top edges and center to screen edges
    /// when within the threshold distance. Right-edge snap snaps the
    /// window's left edge to the screen's right edge (useful for
    /// docking windows to the right side of the screen).
    fn snap_to_edges(&self, x: i32, y: i32) -> (i32, i32) {
        let t = self.snap_config.threshold;
        let w = self.screen_width;
        let h = self.screen_height;

        let snapped_x = if x.abs() <= t {
            0 // left edge to screen left
        } else if (x - w).abs() <= t {
            w // left edge to screen right
        } else if (x - (w / 2)).abs() <= t {
            w / 2 // left edge to screen center
        } else {
            x
        };

        let snapped_y = if y.abs() <= t {
            0 // top edge to screen top
        } else if (y - h).abs() <= t {
            h // top edge to screen bottom
        } else if (y - (h / 2)).abs() <= t {
            h / 2 // top edge to screen center
        } else {
            y
        };

        (snapped_x, snapped_y)
    }

    /// Compute a snap result with metadata (for testing).
    pub fn snap_to_edges_with_result(&self, x: i32, y: i32) -> SnapResult {
        let t = self.snap_config.threshold;
        let w = self.screen_width;
        let h = self.screen_height;

        let (sx, snapped_h) = if x.abs() <= t {
            (0, true)
        } else if (x - w).abs() <= t {
            (w, true)
        } else if (x - (w / 2)).abs() <= t {
            (w / 2, true)
        } else {
            (x, false)
        };

        let (sy, snapped_v) = if y.abs() <= t {
            (0, true)
        } else if (y - h).abs() <= t {
            (h, true)
        } else if (y - (h / 2)).abs() <= t {
            (h / 2, true)
        } else {
            (y, false)
        };

        SnapResult { x: sx, y: sy, snapped_h, snapped_v }
    }
}

impl Default for FloatingManager {
    fn default() -> Self {
        Self::new(800, 600)
    }
}

// ════════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Floating mode tests ──────────────────────────────────────

    #[test]
    fn new_manager_has_no_floating_windows() {
        let mgr = FloatingManager::new(800, 600);
        assert!(mgr.floating_windows.is_empty());
        assert!(mgr.drag.is_none());
        assert!(mgr.resize.is_none());
    }

    #[test]
    fn set_floating_adds_window() {
        let mut mgr = FloatingManager::new(800, 600);
        assert!(mgr.set_floating(10, true));
        assert!(mgr.is_floating(10));
        assert_eq!(mgr.floating_ids().len(), 1);
    }

    #[test]
    fn set_floating_idempotent() {
        let mut mgr = FloatingManager::new(800, 600);
        mgr.set_floating(10, true);
        assert!(!mgr.set_floating(10, true)); // already floating
        assert_eq!(mgr.floating_ids().len(), 1);
    }

    #[test]
    fn remove_from_floating() {
        let mut mgr = FloatingManager::new(800, 600);
        mgr.set_floating(10, true);
        mgr.set_floating(20, true);
        assert_eq!(mgr.floating_ids().len(), 2);

        assert!(mgr.set_floating(10, false));
        assert!(!mgr.is_floating(10));
        assert!(mgr.is_floating(20));
        assert_eq!(mgr.floating_ids().len(), 1);
    }

    #[test]
    fn toggle_floating() {
        let mut mgr = FloatingManager::new(800, 600);
        assert!(mgr.toggle_floating(10)); // becomes floating
        assert!(mgr.is_floating(10));
        assert!(!mgr.toggle_floating(10)); // becomes tiled
        assert!(!mgr.is_floating(10));
    }

    #[test]
    fn remove_window_cleans_up() {
        let mut mgr = FloatingManager::new(800, 600);
        mgr.set_floating(10, true);
        mgr.start_drag(10, 100, 100, 200, 200, 1);
        assert!(mgr.drag.is_some());

        mgr.remove_window(10);
        assert!(!mgr.is_floating(10));
        assert!(mgr.drag.is_none());
    }

    // ── Drag tests ───────────────────────────────────────────────

    #[test]
    fn start_drag_sets_window_floating() {
        let mut mgr = FloatingManager::new(800, 600);
        mgr.start_drag(42, 100, 100, 200, 200, 1);
        assert!(mgr.is_floating(42));
        assert!(mgr.drag.is_some());
    }

    #[test]
    fn drag_motion_moves_window() {
        let mut mgr = FloatingManager::new(800, 600);
        mgr.start_drag(42, 100, 100, 200, 200, 1);

        // Move cursor to (300, 300) — delta is (100, 100)
        let pos = mgr.on_drag_motion(300, 300);
        assert!(pos.is_some());
        let (x, y) = pos.unwrap();
        // Window started at (100, 100), moved by (100, 100) → (200, 200)
        // But snapping may adjust (200 = center of 800, so likely snaps to center)
        // Let's use a non-center position
        assert_eq!(x, 200);
        assert_eq!(y, 200);
    }

    #[test]
    fn drag_motion_without_snap() {
        let mut mgr = FloatingManager::new(800, 600);
        mgr.snap_config.snap_to_edges = false;
        mgr.start_drag(42, 100, 100, 200, 200, 1);

        let pos = mgr.on_drag_motion(300, 300);
        assert!(pos.is_some());
        let (x, y) = pos.unwrap();
        assert_eq!(x, 200); // 100 + (300 - 200) = 200
        assert_eq!(y, 200); // 100 + (300 - 200) = 200
    }

    #[test]
    fn drag_clamps_to_screen() {
        let mut mgr = FloatingManager::new(800, 600);
        mgr.snap_config.snap_to_edges = false;
        mgr.start_drag(42, 100, 100, 200, 200, 1);

        // Move far offscreen
        let pos = mgr.on_drag_motion(-2000, 2000);
        assert!(pos.is_some());
        let (x, y) = pos.unwrap();
        // X clamped: start 100 + delta(-2000-200=-2200) = -2100, but min is -800+100 = -700?
        // Actually: new_x = 100 + (-2000 - 200) = -2100
        // clamped to max(-2100, -700) = -700, min(800-100=700) = -700
        // Wait, let me recalculate: -2100.max(-800+100) = -2100.max(-700) = -700
        // .min(800-100) = 700 → -700
        assert_eq!(x, -700);
        // Y clamped: start 100 + delta(2000-200=1800) = 1900
        // clamped to max(1900, -800+60=-740) = 1900, min(600-60=540) = 540
        assert_eq!(y, 540);
    }

    #[test]
    fn end_drag_clears_state() {
        let mut mgr = FloatingManager::new(800, 600);
        mgr.start_drag(42, 100, 100, 200, 200, 1);
        assert!(mgr.drag.is_some());
        mgr.end_drag();
        assert!(mgr.drag.is_none());
    }

    #[test]
    fn drag_motion_no_drag_returns_none() {
        let mut mgr = FloatingManager::new(800, 600);
        assert!(mgr.on_drag_motion(300, 300).is_none());
    }

    // ── Resize tests ─────────────────────────────────────────────

    #[test]
    fn start_resize_creates_state() {
        let mut mgr = FloatingManager::new(800, 600);
        let edges = ResizeEdges { top: false, bottom: true, left: false, right: true };
        mgr.start_resize(42, edges, 100, 100, 200, 200, 300, 300, 1);
        assert!(mgr.resize.is_some());
    }

    #[test]
    fn resize_motion_bottom_right() {
        let mut mgr = FloatingManager::new(800, 600);
        let edges = ResizeEdges { top: false, bottom: true, left: false, right: true };
        mgr.start_resize(42, edges, 100, 100, 200, 200, 300, 300, 1);

        // Move cursor right and down by 50px
        let result = mgr.on_resize_motion(350, 350);
        assert!(result.is_some());
        let (x, y, w, h) = result.unwrap();
        assert_eq!(x, 100); // no left/top edge → stays
        assert_eq!(y, 100);
        assert_eq!(w, 250); // 200 + (350 - 300) = 250
        assert_eq!(h, 250); // 200 + (350 - 300) = 250
    }

    #[test]
    fn resize_motion_top_left() {
        let mut mgr = FloatingManager::new(800, 600);
        let edges = ResizeEdges { top: true, bottom: false, left: true, right: false };
        mgr.start_resize(42, edges, 100, 100, 200, 200, 300, 300, 1);

        // Move cursor left and up by 30px (delta = -30, -30)
        let result = mgr.on_resize_motion(270, 270);
        assert!(result.is_some());
        let (x, y, w, h) = result.unwrap();
        assert_eq!(x, 70);  // 100 + (-30) = 70
        assert_eq!(y, 70);  // 100 + (-30) = 70
        assert_eq!(w, 230); // 200 - (-30) = 230
        assert_eq!(h, 230); // 200 - (-30) = 230
    }

    #[test]
    fn resize_enforces_minimum_size() {
        let mut mgr = FloatingManager::new(800, 600);
        let edges = ResizeEdges { top: true, bottom: true, left: true, right: true };
        mgr.start_resize(42, edges, 200, 200, 200, 200, 300, 300, 1);

        // Try to shrink far beyond minimum
        let result = mgr.on_resize_motion(600, 600);
        assert!(result.is_some());
        let (x, y, w, h) = result.unwrap();
        assert!(w >= MIN_WINDOW_WIDTH);
        assert!(h >= MIN_WINDOW_HEIGHT);
    }

    #[test]
    fn end_resize_clears_state() {
        let mut mgr = FloatingManager::new(800, 600);
        let edges = ResizeEdges::from_xdg(2 | 8); // bottom | right
        mgr.start_resize(42, edges, 100, 100, 200, 200, 300, 300, 1);
        assert!(mgr.resize.is_some());
        mgr.end_resize();
        assert!(mgr.resize.is_none());
    }

    #[test]
    fn resize_motion_no_resize_returns_none() {
        let mut mgr = FloatingManager::new(800, 600);
        assert!(mgr.on_resize_motion(400, 400).is_none());
    }

    // ── ResizeEdges tests ────────────────────────────────────────

    #[test]
    fn resize_edges_parsing() {
        let e = ResizeEdges::from_xdg(0);
        assert!(!e.is_active());

        let e = ResizeEdges::from_xdg(1);
        assert!(e.top);
        assert!(!e.bottom);
        assert!(!e.left);
        assert!(!e.right);

        let e = ResizeEdges::from_xdg(8);
        assert!(!e.top);
        assert!(!e.bottom);
        assert!(!e.left);
        assert!(e.right);

        let e = ResizeEdges::from_xdg(10); // bottom(2) | right(8)
        assert!(!e.top);
        assert!(e.bottom);
        assert!(!e.left);
        assert!(e.right);
        assert!(e.is_corner());
    }

    #[test]
    fn resize_edges_all() {
        let e = ResizeEdges::from_xdg(15); // 1|2|4|8
        assert!(e.top);
        assert!(e.bottom);
        assert!(e.left);
        assert!(e.right);
        assert!(e.is_corner());
    }

    // ── Snapping tests ───────────────────────────────────────────

    #[test]
    fn snap_to_left_edge() {
        let mgr = FloatingManager::new(800, 600);
        let r = mgr.snap_to_edges_with_result(5, 100);
        assert!(r.snapped_h);
        assert!(!r.snapped_v);
        assert_eq!(r.x, 0);
        assert_eq!(r.y, 100);
    }

    #[test]
    fn snap_to_right_edge() {
        let mgr = FloatingManager::new(800, 600);
        // Window left edge near screen right edge (800)
        let r = mgr.snap_to_edges_with_result(795, 100);
        assert!(r.snapped_h);
        assert_eq!(r.x, 800);
    }

    #[test]
    fn snap_to_top_edge() {
        let mgr = FloatingManager::new(800, 600);
        let r = mgr.snap_to_edges_with_result(100, 3);
        assert!(r.snapped_v);
        assert_eq!(r.y, 0);
    }

    #[test]
    fn snap_to_bottom_edge() {
        let mgr = FloatingManager::new(800, 600);
        // Window top edge near screen bottom edge (600)
        let r = mgr.snap_to_edges_with_result(100, 595);
        assert!(r.snapped_v);
        assert_eq!(r.y, 600);
    }

    #[test]
    fn snap_to_center() {
        let mgr = FloatingManager::new(800, 600);
        let r = mgr.snap_to_edges_with_result(405, 305);
        assert!(r.snapped_h);
        assert!(r.snapped_v);
        assert_eq!(r.x, 400);
        assert_eq!(r.y, 300);
    }

    #[test]
    fn no_snap_outside_threshold() {
        let mgr = FloatingManager::new(800, 600);
        let r = mgr.snap_to_edges_with_result(50, 50);
        assert!(!r.snapped_h);
        assert!(!r.snapped_v);
        assert_eq!(r.x, 50);
        assert_eq!(r.y, 50);
    }
}
