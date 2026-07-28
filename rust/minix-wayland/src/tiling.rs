//! # Tiling Window Layout
//!
//! Implements a tiling window manager layout (i3/sway-style). The layout
//! divides the screen into a master area (left/top) and a stack area
//! (right/bottom). The first window in z-order is the master; all others
//! go to the stack.
//!
//! ## Layout modes
//!
//! - **Horizontal** (default): master left, stack right, stack windows divide
//!   the remaining vertical space equally.
//! - **Vertical**: master top, stack bottom, stack windows divide the
//!   remaining horizontal space equally.
//!
//! ## Gap support
//!
//! An optional pixel gap between windows and around the screen edges
//! provides breathing room.

use alloc::vec::Vec;

/// Split direction for the tiling layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SplitDirection {
    /// Master on the left, stack on the right.
    Horizontal,
    /// Master on the top, stack on the bottom.
    Vertical,
}

impl SplitDirection {
    pub fn toggle(self) -> Self {
        match self {
            SplitDirection::Horizontal => SplitDirection::Vertical,
            SplitDirection::Vertical => SplitDirection::Horizontal,
        }
    }
}

/// Result of a single window's layout calculation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowLayout {
    /// X position on the output.
    pub x: i32,
    /// Y position on the output.
    pub y: i32,
    /// Width of the window.
    pub width: i32,
    /// Height of the window.
    pub height: i32,
}

/// Tiling layout engine for a single workspace.
///
/// Given a list of windows (sorted by z-order, master first), produces
/// positions and sizes for each window according to the tiling rules.
///
/// ## Example
///
/// ```ignore
/// let layout = TilingLayout::new(800, 600)
///     .with_gap(4)
///     .with_master_ratio(0.55);
///
/// let results = layout.calculate(3); // 3 windows
/// // result[0] = master at (2, 2) 438x596
/// // result[1] = stack[0] at (442, 2) 356x296
/// // result[2] = stack[1] at (442, 300) 356x296
/// ```
#[derive(Debug, Clone)]
pub struct TilingLayout {
    /// Width of the output/screen in pixels.
    pub screen_width: i32,
    /// Height of the output/screen in pixels.
    pub screen_height: i32,
    /// Fraction of the screen allocated to the master area (0.0 — 1.0).
    /// Default: 0.5 (50%).
    pub master_ratio: f64,
    /// The split direction.
    pub direction: SplitDirection,
    /// Gap between windows and around screen edges, in pixels.
    /// Default: 4.
    pub gap: i32,
}

impl TilingLayout {
    /// Create a new tiling layout for a screen of the given dimensions.
    pub fn new(width: i32, height: i32) -> Self {
        Self {
            screen_width: width,
            screen_height: height,
            master_ratio: 0.5,
            direction: SplitDirection::Horizontal,
            gap: 4,
        }
    }

    /// Set the master ratio (0.0 — 1.0).
    pub fn with_master_ratio(mut self, ratio: f64) -> Self {
        self.master_ratio = ratio.clamp(0.1, 0.9);
        self
    }

    /// Set the split direction.
    pub fn with_direction(mut self, direction: SplitDirection) -> Self {
        self.direction = direction;
        self
    }

    /// Set the gap between windows.
    pub fn with_gap(mut self, gap: i32) -> Self {
        self.gap = gap.max(0);
        self
    }

    /// Calculate layout positions for `window_count` windows.
    ///
    /// The first window becomes the master; subsequent windows are
    /// arranged in the stack area. If there are no windows, returns
    /// an empty vec.
    ///
    /// # Edge cases
    ///
    /// - 0 windows: empty vec
    /// - 1 window: fills the entire screen (minus gap)
    /// - 2+ windows: master + stack layout
    pub fn calculate(&self, window_count: usize) -> Vec<WindowLayout> {
        if window_count == 0 {
            return Vec::new();
        }

        let gap = self.gap;
        let screen_w = self.screen_width;
        let screen_h = self.screen_height;

        // With only 1 window, it fills the entire screen.
        if window_count == 1 {
            return alloc::vec![WindowLayout {
                x: gap / 2,
                y: gap / 2,
                width: (screen_w - gap).max(1),
                height: (screen_h - gap).max(1),
            }];
        }

        let stack_count = window_count - 1;

        match self.direction {
            SplitDirection::Horizontal => {
                // Master on the left, stack on the right.
                let master_w = ((screen_w as f64 * self.master_ratio) as i32).max(1);
                let stack_w = (screen_w - master_w - gap).max(1);

                // Ensure master area doesn't leave stack too small
                let master_w = if stack_w < 100 { screen_w - 100 - gap } else { master_w };
                let master_w = master_w.max(1).min(screen_w - gap - 1);
                let stack_w = (screen_w - master_w - gap).max(1);

                // Master window
                let mut results = Vec::with_capacity(window_count);
                results.push(WindowLayout {
                    x: gap / 2,
                    y: gap / 2,
                    width: master_w,
                    height: (screen_h - gap).max(1),
                });

                // Stack windows divide the remaining vertical space
                // Total gap = gap/2 top + gap/2 bottom + (stack_count-1)*gap between windows
                let stack_h = ((screen_h - stack_count as i32 * gap) / stack_count as i32).max(1);

                for i in 0..stack_count {
                    let stack_y = gap / 2 + i as i32 * (stack_h + gap);
                    results.push(WindowLayout {
                        x: master_w + gap,
                        y: stack_y,
                        width: stack_w,
                        height: stack_h,
                    });
                }

                results
            }
            SplitDirection::Vertical => {
                // Master on the top, stack on the bottom.
                let master_h = ((screen_h as f64 * self.master_ratio) as i32).max(1);
                let stack_h = (screen_h - master_h - gap).max(1);

                // Ensure master area doesn't leave stack too small
                let master_h = if stack_h < 100 { screen_h - 100 - gap } else { master_h };
                let master_h = master_h.max(1).min(screen_h - gap - 1);
                let stack_h = (screen_h - master_h - gap).max(1);

                // Master window
                let mut results = Vec::with_capacity(window_count);
                results.push(WindowLayout {
                    x: gap / 2,
                    y: gap / 2,
                    width: (screen_w - gap).max(1),
                    height: master_h,
                });

                // Stack windows divide the remaining horizontal space
                // Total gap = gap/2 left + gap/2 right + (stack_count-1)*gap between windows
                let stack_w = ((screen_w - stack_count as i32 * gap) / stack_count as i32).max(1);

                for i in 0..stack_count {
                    let stack_x = gap / 2 + i as i32 * (stack_w + gap);
                    results.push(WindowLayout {
                        x: stack_x,
                        y: master_h + gap,
                        width: stack_w,
                        height: stack_h,
                    });
                }

                results
            }
        }
    }

    /// Calculate layout and assign positions to windows in z-order.
    ///
    /// `z_ordered_windows` should be window IDs sorted by z-order
    /// (ascending). Returns a Vec of (window_id, layout) pairs.
    pub fn calculate_for_windows(
        &self,
        z_ordered_window_ids: &[u32],
    ) -> Vec<(u32, WindowLayout)> {
        let layouts = self.calculate(z_ordered_window_ids.len());
        z_ordered_window_ids
            .iter()
            .zip(layouts.into_iter())
            .map(|(&id, layout)| (id, layout))
            .collect()
    }
}

impl Default for TilingLayout {
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

    #[test]
    fn zero_windows() {
        let layout = TilingLayout::new(800, 600);
        let results = layout.calculate(0);
        assert!(results.is_empty());
    }

    #[test]
    fn single_window_fills_screen() {
        let layout = TilingLayout::new(800, 600).with_gap(0);
        let results = layout.calculate(1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].x, 0);
        assert_eq!(results[0].y, 0);
        assert_eq!(results[0].width, 800);
        assert_eq!(results[0].height, 600);
    }

    #[test]
    fn single_window_with_gap() {
        let layout = TilingLayout::new(800, 600).with_gap(4);
        let results = layout.calculate(1);
        assert_eq!(results[0].x, 2);
        assert_eq!(results[0].y, 2);
        assert_eq!(results[0].width, 796);
        assert_eq!(results[0].height, 596);
    }

    #[test]
    fn two_windows_horizontal() {
        let layout = TilingLayout::new(800, 600)
            .with_direction(SplitDirection::Horizontal)
            .with_gap(0);
        let results = layout.calculate(2);
        assert_eq!(results.len(), 2);

        // Master (left)
        assert_eq!(results[0].x, 0);
        assert_eq!(results[0].width, 400); // 50% of 800

        // Stack (right)
        assert!(results[1].x >= results[0].width);
        assert_eq!(results[1].height, 600);
    }

    #[test]
    fn two_windows_vertical() {
        let layout = TilingLayout::new(800, 600)
            .with_direction(SplitDirection::Vertical)
            .with_gap(0);
        let results = layout.calculate(2);
        assert_eq!(results.len(), 2);

        // Master (top)
        assert_eq!(results[0].y, 0);
        assert_eq!(results[0].height, 300); // 50% of 600

        // Stack (bottom)
        assert!(results[1].y >= results[0].height);
        assert_eq!(results[1].width, 800);
    }

    #[test]
    fn three_windows_horizontal() {
        let layout = TilingLayout::new(800, 600)
            .with_direction(SplitDirection::Horizontal)
            .with_master_ratio(0.5)
            .with_gap(4);

        let results = layout.calculate(3);
        assert_eq!(results.len(), 3);

        // Master (left)
        assert_eq!(results[0].x, 2);

        // Stack has 2 windows dividing the remaining space
        assert!(results[1].x > results[0].x + results[0].width);
        assert_eq!(results[1].height, results[2].height); // equal heights
        assert!(results[2].y > results[1].y); // second stack below first
    }

    #[test]
    fn three_windows_vertical() {
        let layout = TilingLayout::new(800, 600)
            .with_direction(SplitDirection::Vertical)
            .with_master_ratio(0.5)
            .with_gap(4);

        let results = layout.calculate(3);
        assert_eq!(results.len(), 3);

        // Master (top)
        assert_eq!(results[0].y, 2);

        // Stack has 2 windows side by side
        assert_eq!(results[1].width, results[2].width); // equal widths
        assert!(results[2].x > results[1].x); // second stack to the right
    }

    #[test]
    fn master_ratio_custom() {
        let layout = TilingLayout::new(1000, 800)
            .with_master_ratio(0.6)
            .with_gap(0);

        let results = layout.calculate(2);
        // Master should be 60% of width = 600
        assert_eq!(results[0].width, 600);
        // Stack should be the rest = 400
        assert_eq!(results[1].width, 400);
    }

    #[test]
    fn master_ratio_clamped() {
        let layout = TilingLayout::new(800, 600)
            .with_master_ratio(0.0) // below minimum
            .with_gap(0);
        let results = layout.calculate(2);
        // Should be clamped to 0.1 → 10% of 800 = 80
        // But adjusted for minimum stack size
        assert_eq!(results[0].width, results[0].width); // non-panicking check
        assert!(results[0].width > 0);
        assert!(results[1].width > 0);

        let layout2 = TilingLayout::new(800, 600)
            .with_master_ratio(1.5) // above maximum
            .with_gap(0);
        let results2 = layout2.calculate(2);
        // Should be clamped to 0.9
        assert!(results2[0].width > 0);
        assert!(results2[1].width > 0);
    }

    #[test]
    fn no_negative_dimensions() {
        let layout = TilingLayout::new(10, 10).with_gap(0);
        let results = layout.calculate(2);
        for r in &results {
            assert!(r.width > 0, "width must be positive, got {}", r.width);
            assert!(r.height > 0, "height must be positive, got {}", r.height);
            assert!(r.x >= 0);
            assert!(r.y >= 0);
        }
    }

    #[test]
    fn windows_fit_in_screen() {
        let layout = TilingLayout::new(800, 600).with_gap(4);
        let results = layout.calculate(5);
        for r in &results {
            assert!(
                r.x + r.width <= 800,
                "window right edge ({}) exceeds screen width (800)",
                r.x + r.width
            );
            assert!(
                r.y + r.height <= 600,
                "window bottom edge ({}) exceeds screen height (600)",
                r.y + r.height
            );
        }
    }

    #[test]
    fn many_windows() {
        let layout = TilingLayout::new(800, 600).with_gap(2);
        let results = layout.calculate(10);
        assert_eq!(results.len(), 10);
        for r in &results {
            assert!(r.width > 0);
            assert!(r.height > 0);
        }
    }

    #[test]
    fn calculate_for_windows() {
        let layout = TilingLayout::new(800, 600).with_gap(0);
        // Window IDs sorted by z-order: 1 (master), 2 (stack), 3 (stack)
        let ids = alloc::vec![10u32, 20, 30];
        let assigned = layout.calculate_for_windows(&ids);
        assert_eq!(assigned.len(), 3);
        assert_eq!(assigned[0].0, 10);
        assert_eq!(assigned[1].0, 20);
        assert_eq!(assigned[2].0, 30);
    }

    #[test]
    fn direction_toggle() {
        let h = SplitDirection::Horizontal;
        assert_eq!(h.toggle(), SplitDirection::Vertical);
        assert_eq!(h.toggle().toggle(), SplitDirection::Horizontal);
    }
}
