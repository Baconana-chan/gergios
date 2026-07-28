//! # Desktop Shell — Window Management
//!
//! The shell manages windows created by xdg_toplevel clients, provides
//! window list, z-order tracking, and operations like close, focus, raise.
//!
//! ## Architecture
//!
//! ```text
//! Shell
//!   ├── windows: Vec<WindowInfo>
//!   │     └── each window has: title, app_id, position, size, z_order, state
//!   ├── next_z_order: i32 (incrementing counter for new windows)
//!   └── Rc<RefCell<>> for shared access from handler closures
//! ```
//!
//! ## Integration with WaylandServer
//!
//! - `WaylandServer.shell` field holds `Rc<RefCell<Shell>>`
//! - xdg_toplevel handlers update Shell via cloned Rc
//! - `WaylandServer.shell_close_window()` sends CLOSE event to client
//! - `WaylandServer.shell_raise_window()` updates z-order

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::fmt;

/// Information about a single window managed by the shell.
#[derive(Clone, Debug)]
pub struct WindowInfo {
    /// Index into WaylandServer::connections of the client owning this window.
    pub conn_idx: usize,
    /// The xdg_toplevel object ID (used for sending events like CLOSE).
    pub xdg_toplevel_id: u32,
    /// The wl_surface object ID that this toplevel wraps.
    pub surface_id: u32,
    /// Window title (set by xdg_toplevel.set_title).
    pub title: String,
    /// Application ID (set by xdg_toplevel.set_app_id).
    pub app_id: String,
    /// X position on the compositor output.
    pub x: i32,
    /// Y position on the compositor output.
    pub y: i32,
    /// Window width in pixels.
    pub width: i32,
    /// Window height in pixels.
    pub height: i32,
    /// Z-order (higher = on top). Assigned on creation.
    pub z_order: i32,
    /// Whether the window is minimized.
    pub minimized: bool,
    /// Whether the window is maximized.
    pub maximized: bool,
    /// Whether the window is fullscreen.
    pub fullscreen: bool,
    /// Whether the window is visible.
    pub visible: bool,
}

impl fmt::Display for WindowInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Window[{}] \"{}\" ({}) @ ({}, {}) {}x{} z={}",
            self.surface_id,
            self.title,
            self.app_id,
            self.x,
            self.y,
            self.width,
            self.height,
            self.z_order,
        )
    }
}

/// Desktop shell: manages windows, z-order, and provides operations.
///
/// The shell holds a list of all toplevel windows and provides methods
/// to add, remove, query, and manipulate them. It is stored in
/// `WaylandServer` as `Rc<RefCell<Shell>>` so that protocol handlers
/// can access it without owning the server.
pub struct Shell {
    windows: Vec<WindowInfo>,
    next_z_order: i32,
}

impl fmt::Debug for Shell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Shell")
            .field("window_count", &self.windows.len())
            .field("next_z_order", &self.next_z_order)
            .finish()
    }
}

impl Shell {
    /// Create a new, empty shell.
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            next_z_order: 1,
        }
    }

    /// Register a new window from an xdg_toplevel creation.
    ///
    /// Returns the assigned z_order.
    pub fn add_window(
        &mut self,
        conn_idx: usize,
        xdg_toplevel_id: u32,
        surface_id: u32,
        title: String,
        app_id: String,
        width: i32,
        height: i32,
    ) -> i32 {
        let z_order = self.next_z_order;
        self.next_z_order += 1;

        self.windows.push(WindowInfo {
            conn_idx,
            xdg_toplevel_id,
            surface_id,
            title,
            app_id,
            x: 0,
            y: 0,
            width,
            height,
            z_order,
            minimized: false,
            maximized: false,
            fullscreen: false,
            visible: true,
        });

        z_order
    }

    /// Remove a window by its xdg_toplevel_id.
    pub fn remove_window(&mut self, xdg_toplevel_id: u32) -> Option<WindowInfo> {
        let idx = self.windows.iter().position(|w| w.xdg_toplevel_id == xdg_toplevel_id);
        idx.map(|i| self.windows.remove(i))
    }

    /// Update a window's title (called from xdg_toplevel.set_title).
    pub fn set_title(&mut self, xdg_toplevel_id: u32, title: String) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.xdg_toplevel_id == xdg_toplevel_id) {
            w.title = title;
        }
    }

    /// Update a window's app_id (called from xdg_toplevel.set_app_id).
    pub fn set_app_id(&mut self, xdg_toplevel_id: u32, app_id: String) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.xdg_toplevel_id == xdg_toplevel_id) {
            w.app_id = app_id;
        }
    }

    /// Update a window's minimized state.
    pub fn set_minimized(&mut self, xdg_toplevel_id: u32, minimized: bool) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.xdg_toplevel_id == xdg_toplevel_id) {
            w.minimized = minimized;
            if minimized {
                w.visible = false;
            }
        }
    }

    /// Set a window's visibility directly (used by workspace switching).
    /// Does NOT change minimized state.
    pub fn set_visible(&mut self, xdg_toplevel_id: u32, visible: bool) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.xdg_toplevel_id == xdg_toplevel_id) {
            w.visible = visible;
        }
    }

    /// Update a window's maximized state.
    pub fn set_maximized(&mut self, xdg_toplevel_id: u32, maximized: bool) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.xdg_toplevel_id == xdg_toplevel_id) {
            w.maximized = maximized;
        }
    }

    /// Set a window's position on the compositor output.
    pub fn set_position(&mut self, xdg_toplevel_id: u32, x: i32, y: i32) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.xdg_toplevel_id == xdg_toplevel_id) {
            w.x = x;
            w.y = y;
        }
    }

    /// Set a window's size.
    pub fn set_size(&mut self, xdg_toplevel_id: u32, width: i32, height: i32) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.xdg_toplevel_id == xdg_toplevel_id) {
            w.width = width;
            w.height = height;
        }
    }

    /// Update a window's fullscreen state.
    pub fn set_fullscreen(&mut self, xdg_toplevel_id: u32, fullscreen: bool) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.xdg_toplevel_id == xdg_toplevel_id) {
            w.fullscreen = fullscreen;
        }
    }

    /// Raise a window to the top of the z-order.
    ///
    /// Returns the new z_order value.
    pub fn raise_window(&mut self, xdg_toplevel_id: u32) -> i32 {
        let z = self.next_z_order;
        self.next_z_order += 1;

        if let Some(w) = self.windows.iter_mut().find(|w| w.xdg_toplevel_id == xdg_toplevel_id) {
            w.z_order = z;
            w.visible = true;
            if w.minimized {
                w.minimized = false;
                w.visible = true;
            }
        }

        z
    }

    /// Find a window by xdg_toplevel_id.
    pub fn find_by_toplevel(&self, xdg_toplevel_id: u32) -> Option<&WindowInfo> {
        self.windows.iter().find(|w| w.xdg_toplevel_id == xdg_toplevel_id)
    }

    /// Find a window by surface_id.
    pub fn find_by_surface(&self, surface_id: u32) -> Option<&WindowInfo> {
        self.windows.iter().find(|w| w.surface_id == surface_id)
    }

    /// Find a window by connection index and surface_id.
    pub fn find_by_conn_and_surface(&self, conn_idx: usize, surface_id: u32) -> Option<&WindowInfo> {
        self.windows.iter().find(|w| w.conn_idx == conn_idx && w.surface_id == surface_id)
    }

    /// Get all windows sorted by z-order (topmost last).
    pub fn windows_by_z(&self) -> Vec<&WindowInfo> {
        let mut sorted: Vec<&WindowInfo> = self.windows.iter().filter(|w| w.visible).collect();
        sorted.sort_by_key(|w| w.z_order);
        sorted
    }

    /// Get the topmost visible window.
    pub fn topmost(&self) -> Option<&WindowInfo> {
        self.windows.iter()
            .filter(|w| w.visible)
            .max_by_key(|w| w.z_order)
    }

    /// Get the topmost visible window at a specific position.
    pub fn topmost_at(&self, x: i32, y: i32) -> Option<&WindowInfo> {
        self.windows.iter()
            .filter(|w| w.visible && x >= w.x && x < w.x + w.width && y >= w.y && y < w.y + w.height)
            .max_by_key(|w| w.z_order)
    }

    /// Count visible windows.
    pub fn visible_count(&self) -> usize {
        self.windows.iter().filter(|w| w.visible).count()
    }

    /// Total number of windows (including minimized).
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Get a reference to all windows.
    pub fn all_windows(&self) -> &[WindowInfo] {
        &self.windows
    }

    /// Get the next available z_order.
    pub fn current_z(&self) -> i32 {
        self.next_z_order
    }
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

// ════════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_creation() {
        let shell = Shell::new();
        assert_eq!(shell.window_count(), 0);
        assert_eq!(shell.visible_count(), 0);
        assert!(shell.topmost().is_none());
    }

    #[test]
    fn add_and_remove_window() {
        let mut shell = Shell::new();
        let z = shell.add_window(0, 10, 100, "Test".into(), "app".into(), 800, 600);
        assert_eq!(z, 1);
        assert_eq!(shell.window_count(), 1);
        assert_eq!(shell.visible_count(), 1);

        // Find by toplevel_id
        assert!(shell.find_by_toplevel(10).is_some());
        let w = shell.find_by_toplevel(10).unwrap();
        assert_eq!(w.title, "Test");
        assert_eq!(w.app_id, "app");
        assert_eq!(w.width, 800);
        assert_eq!(w.height, 600);

        // Find by surface_id
        assert!(shell.find_by_surface(100).is_some());

        // Remove
        assert!(shell.remove_window(10).is_some());
        assert_eq!(shell.window_count(), 0);
    }

    #[test]
    fn window_state_updates() {
        let mut shell = Shell::new();
        shell.add_window(0, 10, 100, "Test".into(), "app".into(), 800, 600);

        shell.set_title(10, "New Title".into());
        assert_eq!(shell.find_by_toplevel(10).unwrap().title, "New Title");

        shell.set_app_id(10, "new-app".into());
        assert_eq!(shell.find_by_toplevel(10).unwrap().app_id, "new-app");

        shell.set_minimized(10, true);
        assert!(shell.find_by_toplevel(10).unwrap().minimized);
        assert!(!shell.find_by_toplevel(10).unwrap().visible);

        shell.set_maximized(10, true);
        assert!(shell.find_by_toplevel(10).unwrap().maximized);

        shell.set_fullscreen(10, true);
        assert!(shell.find_by_toplevel(10).unwrap().fullscreen);
    }

    #[test]
    fn raise_window() {
        let mut shell = Shell::new();
        let z1 = shell.add_window(0, 1, 100, "A".into(), "a".into(), 100, 100);
        let z2 = shell.add_window(1, 2, 200, "B".into(), "b".into(), 100, 100);
        assert!(z2 > z1);

        // Raise window 1 to top
        let z1_raised = shell.raise_window(1);
        assert!(z1_raised > z2);
        assert_eq!(shell.topmost().unwrap().xdg_toplevel_id, 1);
    }

    #[test]
    fn topmost_at_position() {
        let mut shell = Shell::new();
        // Window A at (0, 0) 200x200
        shell.add_window(0, 1, 100, "A".into(), "a".into(), 200, 200);
        shell.set_position(1, 0, 0);
        // Window B at (50, 50) 200x200 — overlaps A, above A
        shell.add_window(0, 2, 200, "B".into(), "b".into(), 200, 200);
        shell.set_position(2, 50, 50);
        // Window C at (100, 0) 100x100 — not overlapping with others
        shell.add_window(0, 3, 300, "C".into(), "c".into(), 100, 100);
        shell.set_position(3, 100, 0);

        // Point (60, 60) should be in both window A (z=1, covers 0..200) and
        // window B (z=2, covers 50..250) — B is topmost
        let top = shell.topmost_at(60, 60);
        assert!(top.is_some());
        assert_eq!(top.unwrap().xdg_toplevel_id, 2);

        // Point (110, 10) should be in window C only
        let top = shell.topmost_at(110, 10);
        assert!(top.is_some());
        assert_eq!(top.unwrap().xdg_toplevel_id, 3);

        // Point (300, 300) should be nowhere
        assert!(shell.topmost_at(300, 300).is_none());
    }

    #[test]
    fn multiple_windows() {
        let mut shell = Shell::new();
        shell.add_window(0, 1, 100, "Alpha".into(), "alpha".into(), 400, 300);
        shell.add_window(1, 2, 200, "Beta".into(), "beta".into(), 640, 480);
        shell.add_window(2, 3, 300, "Gamma".into(), "gamma".into(), 1024, 768);

        assert_eq!(shell.window_count(), 3);
        assert_eq!(shell.visible_count(), 3);

        // Minimize one window
        shell.set_minimized(2, true);
        assert_eq!(shell.visible_count(), 2);

        // Topmost should be the last added (z=3)
        assert_eq!(shell.topmost().unwrap().xdg_toplevel_id, 3);

        // Remove one
        shell.remove_window(1);
        assert_eq!(shell.window_count(), 2);
    }

    #[test]
    fn windows_by_z_order() {
        let mut shell = Shell::new();
        shell.add_window(0, 1, 100, "A".into(), "a".into(), 100, 100);
        shell.add_window(1, 2, 200, "B".into(), "b".into(), 100, 100);
        shell.add_window(2, 3, 300, "C".into(), "c".into(), 100, 100);

        let by_z = shell.windows_by_z();
        assert_eq!(by_z.len(), 3);
        // Should be sorted by z_order ascending
        assert!(by_z[0].z_order <= by_z[1].z_order);
        assert!(by_z[1].z_order <= by_z[2].z_order);
    }
}
