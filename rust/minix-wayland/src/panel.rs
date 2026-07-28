//! # Panel / Status Bar — Phase 4.6
//!
//! A compositor-managed panel bar at the top of the screen. Rendered as
//! a compositor surface with z_order above all windows. Shows:
//!
//! - **Workspace indicators**: coloured squares for each workspace,
//!   active one highlighted. Click to switch.
//! - **Window titles**: coloured dots for each window title on current workspace.
//! - **Clock**: tick-based simulated time (HH:MM:SS) rendered as pixel bars.
//!   Updates every 60 ticks (~1s at 60fps).
//! - **CPU load placeholder**: animated indicator bar.
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────┐
//! │ [1][2][3][4]  ● ● ●    12:34:56  ▓▓▓░░░░░  ░░░░░░░░░ │  ← panel height=28px
//! └──────────────────────────────────────────────────────┘
//!    workspace    window    clock    cpu     spacer
//!    indicators   dots              load
//! ```
//!
//! The panel is rendered entirely using PixelBuffer primitives
//! (filled rects, lines, raw pixel patterns) — no font system dependency.
//! Text labels require a TTF font at runtime (future enhancement).

use alloc::string::String;
use alloc::vec::Vec;

use minix_compositor::compositor::Compositor;
use minix_compositor::pixel_buffer::PixelBuffer;
use minix_compositor::surface::Surface;

// ── Constants ─────────────────────────────────────────────────────────────

/// Panel height in pixels.
pub const PANEL_HEIGHT: u32 = 28;

/// Workspace indicator size (square).
pub const WS_INDICATOR_SIZE: u32 = 12;

/// Gap between workspace indicators.
pub const WS_INDICATOR_GAP: u32 = 4;

/// Left margin for the first workspace indicator.
pub const WS_LEFT_MARGIN: u32 = 8;

/// Height of the clock area background.
pub const CLOCK_AREA_HEIGHT: u32 = 9;

/// Z-order offset: panel is rendered above decorations and windows.
/// Panel is at the very top (highest z in the compositor).
pub const PANEL_Z_ORDER: i32 = 10000;

/// Panel background colour (dark blue-gray).
pub const PANEL_BG: [u8; 4] = [0x1A, 0x1A, 0x2E, 0xFF];

/// Inactive workspace indicator (dim).
pub const WS_INACTIVE: [u8; 4] = [0x40, 0x40, 0x60, 0xFF];

/// Active workspace indicator (bright accent).
pub const WS_ACTIVE: [u8; 4] = [0x50, 0xA0, 0xF0, 0xFF];

/// Window title text area background (slightly lighter than panel bg).
pub const TITLE_BG: [u8; 4] = [0x22, 0x22, 0x38, 0xFF];

/// Clock separator colour.
pub const CLOCK_SEP_COLOR: [u8; 4] = [0x40, 0x40, 0x60, 0xFF];

/// Clock digit segments colour (green-cyan glow).
pub const CLOCK_DIGIT_COLOR: [u8; 4] = [0x40, 0xC0, 0xC0, 0xFF];

/// CPU load bar fill colour (green).
pub const CPU_FILL_COLOR: [u8; 4] = [0x50, 0xE0, 0x50, 0xFF];

/// CPU load bar background (dim).
pub const CPU_BG_COLOR: [u8; 4] = [0x20, 0x30, 0x20, 0xFF];

/// Tick interval for clock update (ticks per second @ 60fps).
pub const CLOCK_TICK_INTERVAL: u64 = 60;

// ── PanelAction ───────────────────────────────────────────────────────────

/// Actions that can result from clicking on the panel.
#[derive(Debug, Clone, PartialEq)]
pub enum PanelAction {
    /// Switch to the workspace at the given index.
    SwitchWorkspace(usize),
    /// Click didn't hit any interactive widget.
    None,
}

// ── Panel ─────────────────────────────────────────────────────────────────

/// The system panel / status bar.
///
/// Created as a compositor surface with `z_order = PANEL_Z_ORDER`,
/// positioned at `(0, 0)` with full screen width and `PANEL_HEIGHT`.
pub struct Panel {
    /// The compositor surface ID for this panel.
    pub surface_id: u64,
    /// Panel width (full screen width).
    pub width: u32,
    /// Tick counter for simple animations.
    pub tick_count: u64,
    /// Workspace names (for display).
    pub workspace_names: Vec<String>,
    /// Currently active workspace index.
    pub active_workspace: usize,
    /// Window titles on the current workspace (rendered in the title area).
    pub window_titles: Vec<String>,
    /// Simulated clock hours (0-23).
    pub clock_hours: u32,
    /// Simulated clock minutes (0-59).
    pub clock_minutes: u32,
    /// Simulated clock seconds (0-59).
    pub clock_seconds: u32,
    /// CPU load meter (0-100). Simulated: oscillates for demo.
    pub cpu_load: u32,
}

impl Panel {
    /// Create a new panel and add its surface to the compositor.
    ///
    /// The panel surface is positioned at `(0, 0)` with the given
    /// screen width and `PANEL_HEIGHT`, with `z_order = PANEL_Z_ORDER`.
    pub fn create(comp: &mut Compositor, screen_width: u32, workspace_names: &[String]) -> Self {
        let mut surface = Surface::new(screen_width, PANEL_HEIGHT, 0, 0);
        surface.z_order = PANEL_Z_ORDER;
        surface.visible = true;

        // Fill with background colour
        surface.buffer.clear(PANEL_BG);
        // Draw a 1px bottom border line
        let border_y = (PANEL_HEIGHT - 1) as i32;
        surface.buffer.draw_line(0, border_y, screen_width as i32 - 1, border_y, [0x30, 0x30, 0x50, 0xFF]);

        let surface_id = comp.add_surface(surface);

        let names: Vec<String> = if workspace_names.is_empty() {
            (1..=4).map(|i| alloc::format!("{}", i)).collect()
        } else {
            workspace_names.to_vec()
        };

        Self {
            surface_id,
            width: screen_width,
            tick_count: 0,
            workspace_names: names,
            active_workspace: 0,
            window_titles: Vec::new(),
            clock_hours: 0,
            clock_minutes: 0,
            clock_seconds: 0,
            cpu_load: 0,
        }
    }

    /// Render a single digit (0-9) as a 4x7 pixel pattern at (x, y).
    fn render_digit(buf: &mut PixelBuffer, x: u32, y: u32, digit: u32) {
        // 4x7 pixel glyphs for digits 0-9
        // Each 4-bit nibble is one row (bits: 0b_row7_row6_row5_row4_row3_row2_row1_row0)
        // Each nibble: bits ABCD where A=leftmost, D=rightmost
        const DIGITS: [u32; 10] = [
            0b0111_0101_0101_0101_0101_0101_0111, // 0
            0b0010_0110_0010_0010_0010_0010_0111, // 1
            0b0111_0001_0001_0111_0100_0100_0111, // 2
            0b0111_0001_0001_0011_0001_0001_0111, // 3
            0b0101_0101_0101_0111_0001_0001_0001, // 4
            0b0111_0100_0100_0111_0001_0001_0111, // 5
            0b0111_0100_0100_0111_0101_0101_0111, // 6
            0b0111_0001_0001_0010_0010_0010_0010, // 7
            0b0111_0101_0101_0111_0101_0101_0111, // 8
            0b0111_0101_0101_0111_0001_0001_0111, // 9
        ];

        let pattern = if digit < 10 { DIGITS[digit as usize] } else { 0 };
        for row in 0..7 {
            let row_bits = (pattern >> ((6 - row) * 4)) & 0xF;
            for col in 0..4 {
                if (row_bits >> (3 - col)) & 1 != 0 {
                    buf.set_pixel(x + col, y + row as u32, CLOCK_DIGIT_COLOR);
                }
            }
        }
    }

    /// Re-render the panel into its compositor surface.
    ///
    /// Called on every tick. Redraws the entire panel from scratch.
    /// Also advances the simulated clock time every `CLOCK_TICK_INTERVAL` ticks.
    pub fn render(&mut self, comp: &mut Compositor) {
        // Update simulated clock every 60 ticks (~1 second)
        if self.tick_count % CLOCK_TICK_INTERVAL == 0 && self.tick_count > 0 {
            self.clock_seconds = (self.clock_seconds + 1) % 60;
            if self.clock_seconds == 0 {
                self.clock_minutes = (self.clock_minutes + 1) % 60;
                if self.clock_minutes == 0 {
                    self.clock_hours = (self.clock_hours + 1) % 24;
                }
            }
        }

        // Simulate CPU load oscillation using sine wave
        // Ranges from ~10% (45-35) to ~80% (45+35)
        self.cpu_load = 45 + ((((self.tick_count / 30) as i64 * 30) as f64).sin() * 35.0) as u32;

        if let Some(s) = comp.get_surface(self.surface_id) {
            // 1. Fill background
            s.buffer.clear(PANEL_BG);

            // 2. Bottom border
            let border_y = (PANEL_HEIGHT - 1) as i32;
            s.buffer.draw_line(0, border_y, self.width as i32 - 1, border_y, [0x30, 0x30, 0x50, 0xFF]);

            // 3. Workspace indicators (left side)
            let ws_total_w = self.workspace_names.len() as u32 * (WS_INDICATOR_SIZE + WS_INDICATOR_GAP);
            let ws_start_x = WS_LEFT_MARGIN;
            let ws_y = (PANEL_HEIGHT - WS_INDICATOR_SIZE) / 2;

            for (i, _name) in self.workspace_names.iter().enumerate() {
                let x = ws_start_x + i as u32 * (WS_INDICATOR_SIZE + WS_INDICATOR_GAP);
                let color = if i == self.active_workspace {
                    WS_ACTIVE
                } else {
                    WS_INACTIVE
                };

                // Draw indicator as a rounded rect
                s.buffer.fill_rounded_rect(x, ws_y, WS_INDICATOR_SIZE, WS_INDICATOR_SIZE, 3, color);
            }

            // 4. Window title area (after workspace indicators, before clock)
            let title_area_x = ws_start_x + ws_total_w + 12;
            let clock_area_w = (4 * 4 + 4) * 6 + 12 + 20; // 6 digits + separators + cpu bar + margin
            let title_area_w = self.width.saturating_sub(title_area_x + clock_area_w);

            if title_area_w > 10 {
                // Draw a subtle separator between indicators and titles
                let sep_x = title_area_x - 6;
                let sep_y1 = (PANEL_HEIGHT - CLOCK_AREA_HEIGHT) / 2;
                let sep_y2 = sep_y1 + CLOCK_AREA_HEIGHT;
                s.buffer.draw_line(sep_x as i32, sep_y1 as i32, sep_x as i32, sep_y2 as i32, CLOCK_SEP_COLOR);

                // Draw window title area background
                s.buffer.fill_rounded_rect(title_area_x, ws_y, title_area_w, WS_INDICATOR_SIZE, 3, TITLE_BG);

                // Draw window title dots (placeholder for text — coloured dots per window)
                let dot_r = 3;
                let dot_y = ws_y + WS_INDICATOR_SIZE / 2;
                let mut dot_x = title_area_x + 6;

                for (_idx, _title) in self.window_titles.iter().enumerate() {
                    if dot_x + dot_r * 2 > title_area_x + title_area_w {
                        break;
                    }
                    // Draw a small filled circle (approximated as a rect)
                    s.buffer.fill_rounded_rect(dot_x, dot_y - dot_r, dot_r * 2, dot_r * 2, dot_r, WS_ACTIVE);
                    dot_x += dot_r * 2 + 4;
                }
            }

            // 5. Clock display (right side)
            let digit_w = 4;
            let digit_gap = 1;
            let colon_w = 2;
            let clock_base_x = self.width.saturating_sub(clock_area_w);
            let clock_y = (PANEL_HEIGHT - 7) / 2;

            // Format: HH:MM:SS
            let digits = [
                self.clock_hours / 10, self.clock_hours % 10,
                self.clock_minutes / 10, self.clock_minutes % 10,
                self.clock_seconds / 10, self.clock_seconds % 10,
            ];

            let mut cx = clock_base_x;
            let colon_col = CLOCK_DIGIT_COLOR;

            // Draw a small bg rect for the clock area
            s.buffer.fill_rounded_rect(
                clock_base_x - 4, clock_y.saturating_sub(1),
                (digit_w + digit_gap) * 6 + colon_w * 2 + 8, 9, 2,
                [0x10, 0x18, 0x20, 0xFF],
            );

            for (i, &d) in digits.iter().enumerate() {
                Self::render_digit(&mut s.buffer, cx, clock_y, d);
                cx += digit_w + digit_gap;

                // Draw colon after hours and minutes (at positions 2 and 4)
                if i == 1 || i == 3 {
                    let colon_x = cx;
                    let colon_y1 = clock_y + 1;
                    let colon_y2 = clock_y + 5;
                    s.buffer.set_pixel(colon_x, colon_y1, colon_col);
                    s.buffer.set_pixel(colon_x + 1, colon_y1, colon_col);
                    s.buffer.set_pixel(colon_x, colon_y2, colon_col);
                    s.buffer.set_pixel(colon_x + 1, colon_y2, colon_col);
                    cx += colon_w + digit_gap;
                }
            }

            // 6. CPU load indicator (after clock)
            let cpu_x = cx + 6;
            let cpu_w = 20;
            let cpu_h = 6;
            let cpu_y = (PANEL_HEIGHT - cpu_h) / 2;

            // Background bar
            s.buffer.fill_rounded_rect(cpu_x, cpu_y, cpu_w, cpu_h, 2, CPU_BG_COLOR);
            // Fill bar
            let fill_w = (cpu_w as u32 * self.cpu_load / 100).max(2);
            s.buffer.fill_rounded_rect(cpu_x, cpu_y, fill_w, cpu_h, 2, CPU_FILL_COLOR);

            // Mark dirty for re-composite
            s.mark_dirty();
        }

        self.tick_count = self.tick_count.wrapping_add(1);
    }

    /// Handle a click on the panel.
    ///
    /// Returns `PanelAction::SwitchWorkspace(index)` if a workspace
    /// indicator was clicked, or `PanelAction::None` otherwise.
    pub fn handle_click(&self, x: i32, y: i32) -> PanelAction {
        // Check if click is within panel vertical bounds
        if y < 0 || y >= PANEL_HEIGHT as i32 {
            return PanelAction::None;
        }

        // Check workspace indicator area
        let ws_start_x = WS_LEFT_MARGIN as i32;
        let ws_y = ((PANEL_HEIGHT - WS_INDICATOR_SIZE) / 2) as i32;

        if y >= ws_y && y < ws_y + WS_INDICATOR_SIZE as i32 {
            for i in 0..self.workspace_names.len() {
                let ix = ws_start_x + i as i32 * (WS_INDICATOR_SIZE + WS_INDICATOR_GAP) as i32;
                if x >= ix && x < ix + WS_INDICATOR_SIZE as i32 {
                    return PanelAction::SwitchWorkspace(i);
                }
            }
        }

        PanelAction::None
    }

    /// Update the active workspace index.
    pub fn set_active_workspace(&mut self, index: usize) {
        self.active_workspace = index;
    }

    /// Update the list of workspace names.
    pub fn set_workspace_names(&mut self, names: &[String]) {
        self.workspace_names = names.to_vec();
    }

    /// Update the list of window titles on the current workspace.
    pub fn set_window_titles(&mut self, titles: &[String]) {
        self.window_titles = titles.to_vec();
    }
}

// ════════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_panel(comp: &mut Compositor) -> Panel {
        let names: Vec<String> = (1..=4).map(|i| alloc::format!("{}", i)).collect();
        Panel::create(comp, 800, &names)
    }

    #[test]
    fn panel_creation() {
        let mut comp = Compositor::new(800, 600);
        let panel = make_panel(&mut comp);

        assert_eq!(panel.width, 800);
        assert_eq!(panel.workspace_names.len(), 4);
        assert_eq!(panel.active_workspace, 0);
        assert!(panel.surface_id > 0);

        // Surface exists in compositor
        let s = comp.get_surface(panel.surface_id);
        assert!(s.is_some());
        let s_ref = s.unwrap();
        assert_eq!(s_ref.buffer.width, 800);
        assert_eq!(s_ref.buffer.height, PANEL_HEIGHT);
        assert_eq!(s_ref.z_order, PANEL_Z_ORDER);
        assert!(s_ref.visible);
    }

    #[test]
    fn panel_render() {
        let mut comp = Compositor::new(800, 600);
        let mut panel = make_panel(&mut comp);

        panel.render(&mut comp);

        // After render, surface should be dirty
        let s = comp.get_surface(panel.surface_id).unwrap();
        assert!(s.dirty);
        // Buffer should have non-zero content (background fill)
        assert!(s.buffer.as_bytes().iter().any(|&b| b != 0));
        // Top-left pixel should be background colour
        assert_eq!(s.buffer.get_pixel(0, 0), PANEL_BG);
        // Bottom-left pixel should be border colour (or bg if border is above)
        // Actually, border is drawn at y=27, so pixel at 0,0 is just bg
    }

    #[test]
    fn panel_click_outside_returns_none() {
        let mut comp = Compositor::new(800, 600);
        let panel = make_panel(&mut comp);

        // Click below the panel
        assert!(matches!(panel.handle_click(100, 100), PanelAction::None));
        // Click above the panel
        assert!(matches!(panel.handle_click(100, -1), PanelAction::None));
    }

    #[test]
    fn panel_click_workspace_indicator() {
        let mut comp = Compositor::new(800, 600);
        let panel = make_panel(&mut comp);

        // Workspace indicators start at x=8, each is 12px wide with 4px gap
        // WS 0: x=8..20, WS 1: x=24..36, WS 2: x=40..52, WS 3: x=56..68
        let ws_y = ((PANEL_HEIGHT - WS_INDICATOR_SIZE) / 2) as i32;

        // Click on workspace 0
        assert!(matches!(panel.handle_click(10, ws_y + 2), PanelAction::SwitchWorkspace(0)));
        // Click on workspace 1
        assert!(matches!(panel.handle_click(26, ws_y + 2), PanelAction::SwitchWorkspace(1)));
        // Click on workspace 2
        assert!(matches!(panel.handle_click(42, ws_y + 2), PanelAction::SwitchWorkspace(2)));
        // Click on workspace 3
        assert!(matches!(panel.handle_click(58, ws_y + 2), PanelAction::SwitchWorkspace(3)));
        // Click between indicators (gap)
        assert!(matches!(panel.handle_click(21, ws_y + 2), PanelAction::None));
        // Click before first indicator
        assert!(matches!(panel.handle_click(2, ws_y + 2), PanelAction::None));
    }

    #[test]
    fn panel_set_active_workspace() {
        let mut comp = Compositor::new(800, 600);
        let mut panel = make_panel(&mut comp);

        assert_eq!(panel.active_workspace, 0);
        panel.set_active_workspace(2);
        assert_eq!(panel.active_workspace, 2);
    }

    #[test]
    fn panel_set_window_titles() {
        let mut comp = Compositor::new(800, 600);
        let mut panel = make_panel(&mut comp);

        let titles = alloc::vec![
            alloc::string::String::from("terminal"),
            alloc::string::String::from("file-manager"),
        ];
        panel.set_window_titles(&titles);
        assert_eq!(panel.window_titles.len(), 2);
        assert_eq!(panel.window_titles[0], "terminal");
    }

    #[test]
    fn panel_render_with_titles() {
        let mut comp = Compositor::new(800, 600);
        let mut panel = make_panel(&mut comp);

        panel.set_window_titles(&[
            alloc::string::String::from("terminal"),
            alloc::string::String::from("file-manager"),
        ]);
        panel.set_active_workspace(1);
        panel.render(&mut comp);

        // Should still have non-zero buffer content
        let s = comp.get_surface(panel.surface_id).unwrap();
        assert!(s.dirty);
        assert!(s.buffer.as_bytes().iter().any(|&b| b != 0));
    }

    #[test]
    fn panel_click_after_render() {
        let mut comp = Compositor::new(800, 600);
        let mut panel = make_panel(&mut comp);

        panel.render(&mut comp);
        panel.set_active_workspace(1);

        let ws_y = ((PANEL_HEIGHT - WS_INDICATOR_SIZE) / 2) as i32;
        // Even after render, clicking workspace 0 should return workspace 0
        // (click handling doesn't depend on render state)
        assert!(matches!(panel.handle_click(10, ws_y + 2), PanelAction::SwitchWorkspace(0)));
    }
}
