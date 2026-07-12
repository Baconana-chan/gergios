//! # Event Loop — Drives the compositor, polls input, and presents frames
//!
//! A simple event loop suitable for MINIX system services. Instead of
//! depending on calloop epoll/kqueue (which requires `std` and Unix fds),
//! this loop uses a polling strategy:
//!
//! 1. Poll all input sources for new events
//! 2. Dispatch events to the compositor / surfaces
//! 3. Composite if dirty
//! 4. Present to backend
//! 5. Wait for next tick
//!
//! On real MINIX, the main event loop would use `ipc_receive()` to wait
//! for clock ticks, device interrupts, or IPC messages. On host platforms,
//! it uses a simple spin-loop placeholder.

use minix_compositor::compositor::Compositor;
use minix_compositor::backend::Backend;
use crate::source::InputSource;

/// Configuration for the compositor event loop.
pub struct EventLoopConfig {
    /// Target frame interval in milliseconds (e.g., 16 ≈ 60fps).
    pub frame_interval_ms: u64,
}

impl Default for EventLoopConfig {
    fn default() -> Self {
        Self {
            frame_interval_ms: 16, // ~60 FPS
        }
    }
}

/// Statistics from the event loop.
#[derive(Debug, Clone, Default)]
pub struct LoopStats {
    pub frames_rendered: u64,
    pub events_processed: u64,
    pub avg_frame_time_ms: f64,
}

/// Result of a single event loop tick.
#[derive(Debug, Clone, Default)]
pub struct LoopTickResult {
    pub events_processed: usize,
}

/// Run a single iteration of the event loop.
///
/// Polls `input` for new events, marks the compositor as dirty if any
/// events were received, then composites and presents.
///
/// Returns the number of events processed.
pub fn run_tick(
    compositor: &mut Compositor,
    backend: &mut dyn Backend,
    input: &mut dyn InputSource,
) -> LoopTickResult {
    let mut processed = 0usize;

    // Poll input sources for events
    if input.has_pending() {
        for event in input.poll() {
            processed += 1;
            // For now, input is tracked but not dispatched to specific
            // surfaces — the compositor just knows something happened.
            // Surface input dispatch will be added in Phase 3 (Wayland)
            // when we have surfaces with focus tracking.
            let _ = event;
        }

        // Mark compositor as needing re-composite if we got events
        if processed > 0 {
            compositor.needs_composite = true;
        }
    }

    // Composite and present
    if compositor.needs_composite {
        compositor.composite(Some(backend));
    }

    LoopTickResult {
        events_processed: processed,
    }
}

/// Run the event loop indefinitely.
///
/// Uses a simple polling loop. On host platforms (non-MINIX), this will
/// run as fast as possible yielding via spin-loop hints.
/// On MINIX, it uses `ipc_receive()` to wait for the next event.
///
/// Returns when the `should_continue` closure returns `false`.
pub fn run_loop<F>(
    compositor: &mut Compositor,
    backend: &mut dyn Backend,
    input: &mut dyn InputSource,
    config: &EventLoopConfig,
    mut should_continue: F,
) -> LoopStats
where
    F: FnMut(&Compositor, &LoopStats) -> bool,
{
    let mut stats = LoopStats::default();

    while should_continue(compositor, &stats) {
        let result = run_tick(compositor, backend, input);
        stats.events_processed += result.events_processed as u64;
        if compositor.needs_composite {
            stats.frames_rendered += 1;
        }

        // Wait for next event depending on platform
        #[cfg(feature = "minix")]
        {
            // On MINIX, wait for the next IPC message
            // (clock tick, input event, or other notification)
            // TODO: receive from specific sources instead of ANY
            let mut msg = minix_rs::Message::new();
            let _ = minix_rs::ipc_receive(minix_rs::ANY, &mut msg);
        }

        #[cfg(not(feature = "minix"))]
        {
            // On host, spin a bit to avoid 100% CPU
            // TODO: use frame_interval_ms to calibrate sleep duration
            // TODO: replace with proper sleep when std is available
            for _ in 0..10_000 {
                core::hint::spin_loop();
            }
        }
    }

    stats
}
