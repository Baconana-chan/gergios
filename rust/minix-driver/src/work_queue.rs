//! # WorkQueue — SPSC deferred work queue for threaded IRQ handlers
//!
//! A lock-free single-producer, single-consumer work queue designed for
//! top-half/bottom-half interrupt handling:
//!
//! - **Top-half** (interrupt context): calls `enqueue()` to schedule work.
//!   Must be fast — no allocation, no blocking.
//! - **Bottom-half** (worker thread): calls `process_all()` to drain the queue
//!   and execute all pending work items in priority order.
//!
//! ## Priority Model (softirq-style)
//!
//! Three priority levels, processed in strict order (all Hi → all Tasklet →
//! all Background) to prevent starvation of lower priorities:
//!
//! | Priority  | Use case                          |
//! |-----------|-----------------------------------|
//! | `Hi`      | RX/TX, timer — must be serviced quickly |
//! | `Tasklet` | Normal deferred work (PHY events, link) |
//! | `Background` | Low-priority stats, housekeeping |
//!
//! ## Safety
//!
//! SPSC semantics: only one producer (the interrupt handler) and one consumer
//! (the worker thread). Atomic head/tail indices prevent torn reads/writes
//! without requiring a mutex.
//!
//! ## Re-entrancy
//!
//! All `process_*` methods are **not re-entrant**. They must not be called from
//! within a work function (bottom-half). Doing so could cause unexpected
//! recursion and violate the SPSC protocol.

#![allow(dead_code)]

use core::sync::atomic::{AtomicUsize, Ordering};

/// Maximum number of pending work items per priority level.
const QUEUE_CAPACITY: usize = 64;

/// Number of softirq priority levels.
pub const NR_SOFTIRQ_PRIORITIES: usize = 3;

/// SoftIRQ priority levels, ordered highest-first.
///
/// The ordinal determines processing order: `Hi = 0` is processed first,
/// `Tasklet = 1` second, `Background = 2` last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftIrqPriority {
    /// High priority — RX/TX, timers. Processed first.
    Hi = 0,
    /// Normal priority — PHY events, link changes, connection management.
    Tasklet = 1,
    /// Low priority — statistics, periodic housekeeping. Processed last.
    Background = 2,
}

impl SoftIrqPriority {
    /// Convert to a zero-based index into the priority queue array.
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Return all priorities from highest to lowest for iteration.
    pub const fn all_highest_first() -> [SoftIrqPriority; 3] {
        [SoftIrqPriority::Hi, SoftIrqPriority::Tasklet, SoftIrqPriority::Background]
    }
}

/// A work item = a function pointer + a data pointer.
type WorkFn = unsafe extern "C" fn(data: usize);

/// Lock-free SPSC ring buffer for deferred work at a single priority level.
///
/// # Panics
///
/// Methods only panic if the queue is misused (e.g., `process_all` called from
/// the producer context while an enqueue is in progress). This should not
/// happen in normal top-half/bottom-half usage.
struct SpscRing {
    /// Ring buffer of function pointers.
    items: [Option<WorkFn>; QUEUE_CAPACITY],
    /// Per-entry data values passed to the work function.
    data: [usize; QUEUE_CAPACITY],
    /// Producer index (written by top-half, read by bottom-half).
    head: AtomicUsize,
    /// Consumer index (written by bottom-half, read by top-half).
    tail: AtomicUsize,
}

impl SpscRing {
    const fn new() -> Self {
        const NONE: Option<WorkFn> = None;
        SpscRing {
            items: [NONE; QUEUE_CAPACITY],
            data: [0; QUEUE_CAPACITY],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Enqueue a work item from interrupt (top-half) context.
    ///
    /// Returns `true` if the item was enqueued, `false` if the queue is full.
    fn enqueue(&self, work: WorkFn, data: usize) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        let next = (head + 1) % QUEUE_CAPACITY;

        if next == tail {
            return false;
        }

        unsafe {
            let items_ptr = self.items.as_ptr() as *mut Option<WorkFn>;
            let data_ptr = self.data.as_ptr() as *mut usize;
            *items_ptr.add(head) = Some(work);
            *data_ptr.add(head) = data;
        }
        self.head.store(next, Ordering::Release);
        true
    }

    /// Process up to `max_items` pending work items.
    ///
    /// Returns the number of items processed.
    fn process_up_to(&self, max_items: usize) -> usize {
        let mut processed = 0;
        while processed < max_items {
            let tail = self.tail.load(Ordering::Relaxed);
            let head = self.head.load(Ordering::Acquire);

            if tail == head {
                break;
            }

            let (work_fn, work_data) = unsafe {
                let items_ptr = self.items.as_ptr() as *mut Option<WorkFn>;
                let data_ptr = self.data.as_ptr() as *mut usize;
                let w = *items_ptr.add(tail);
                let d = *data_ptr.add(tail);
                *items_ptr.add(tail) = None;
                (w, d)
            };

            let next = (tail + 1) % QUEUE_CAPACITY;
            self.tail.store(next, Ordering::Release);

            if let Some(f) = work_fn {
                unsafe { f(work_data) };
            }

            processed += 1;
        }
        processed
    }

    /// Process all pending work items.
    fn process_all(&self) -> usize {
        self.process_up_to(QUEUE_CAPACITY)
    }

    fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire) == self.tail.load(Ordering::Acquire)
    }

    fn pending(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        if head >= tail {
            head - tail
        } else {
            QUEUE_CAPACITY - (tail - head)
        }
    }
}

/// Multi-priority work queue with softirq-style priority levels.
///
/// Wraps three independent SPSC ring buffers (one per priority).
/// `process_all()` drains in strict priority order: all Hi, then all Tasklet,
/// then all Background. This prevents high-priority RX/TX work from being
/// blocked by lower-priority PHY event processing.
pub struct PriorityWorkQueue {
    /// Per-priority SPSC ring buffers.
    queues: [SpscRing; NR_SOFTIRQ_PRIORITIES],
}

impl PriorityWorkQueue {
    /// Create a new empty priority work queue.
    pub const fn new() -> Self {
        PriorityWorkQueue {
            queues: [
                SpscRing::new(),
                SpscRing::new(),
                SpscRing::new(),
            ],
        }
    }

    /// Enqueue a work item at the given priority from interrupt (top-half) context.
    ///
    /// Returns `true` if the item was enqueued, `false` if the priority queue is full.
    /// Safe to call from interrupt context — no allocation, no blocking.
    #[inline]
    pub fn enqueue(&self, priority: SoftIrqPriority, work: WorkFn, data: usize) -> bool {
        self.queues[priority.index()].enqueue(work, data)
    }

    /// Process all pending work items in priority order (Hi → Tasklet → Background).
    ///
    /// Drains each priority queue completely before moving to the next lower
    /// priority. Returns the total number of items processed.
    pub fn process_all(&self) -> usize {
        let mut total = 0;
        for prio in SoftIrqPriority::all_highest_first() {
            total += self.queues[prio.index()].process_all();
        }
        total
    }

    /// Process pending work items for a single priority level only.
    ///
    /// Useful when you want to drain only high-priority work quickly without
    /// potentially blocking on lower priorities. Returns number of items processed.
    #[inline]
    pub fn process_priority(&self, priority: SoftIrqPriority) -> usize {
        self.queues[priority.index()].process_all()
    }

    /// Process up to `budget` items across all priority levels.
    ///
    /// Allocates the budget proportionally: Hi gets priority, then Tasklet,
    /// then Background. Returns the total number of items processed.
    /// This prevents starvation of lower priorities when interrupts arrive
    /// faster than the worker can drain them.
    ///
    /// `budget` is the maximum number of items to process in this call.
    /// `hi_ratio` is the fraction (0..=100) of the budget reserved for Hi priority.
    /// Default: `budget = 32, hi_ratio = 60` gives 60% Hi / 30% Tasklet / 10% Background.
    pub fn process_budgeted(&self, budget: usize, hi_ratio: usize) -> usize {
        let hi_budget = (budget * hi_ratio) / 100;
        let remaining = budget.saturating_sub(hi_budget);
        let tasklet_budget = (remaining * 3) / 4; // 75% of remaining → 30% of total
        let bg_budget = remaining.saturating_sub(tasklet_budget); // ~10% of total

        let mut total = 0;
        total += self.queues[SoftIrqPriority::Hi.index()].process_up_to(hi_budget);
        total += self.queues[SoftIrqPriority::Tasklet.index()].process_up_to(tasklet_budget);
        total += self.queues[SoftIrqPriority::Background.index()].process_up_to(bg_budget);
        total
    }

    /// Check if all priority queues are empty.
    pub fn is_empty(&self) -> bool {
        for prio in SoftIrqPriority::all_highest_first() {
            if !self.queues[prio.index()].is_empty() {
                return false;
            }
        }
        true
    }

    /// Return the total number of pending items across all priorities.
    pub fn pending(&self) -> usize {
        let mut total = 0;
        for prio in SoftIrqPriority::all_highest_first() {
            total += self.queues[prio.index()].pending();
        }
        total
    }

    /// Return number of pending items at a given priority.
    #[inline]
    pub fn pending_priority(&self, priority: SoftIrqPriority) -> usize {
        self.queues[priority.index()].pending()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    #[test]
    fn enqueue_dequeue_works() {
        static CALLED: AtomicBool = AtomicBool::new(false);
        unsafe extern "C" fn test_work(_data: usize) {
            CALLED.store(true, Ordering::Release);
        }

        let wq = PriorityWorkQueue::new();
        assert!(wq.is_empty());

        assert!(wq.enqueue(SoftIrqPriority::Tasklet, test_work, 42));
        assert!(!wq.is_empty());
        assert_eq!(wq.pending_priority(SoftIrqPriority::Tasklet), 1);

        let count = wq.process_all();
        assert_eq!(count, 1);
        assert!(CALLED.load(Ordering::Acquire));
        assert!(wq.is_empty());
    }

    #[test]
    fn data_passed_correctly() {
        static RESULT: AtomicUsize = AtomicUsize::new(0);
        unsafe extern "C" fn test_work(data: usize) {
            RESULT.store(data, Ordering::Release);
        }

        let wq = PriorityWorkQueue::new();
        assert!(wq.enqueue(SoftIrqPriority::Hi, test_work, 99));
        wq.process_all();
        assert_eq!(RESULT.load(Ordering::Acquire), 99);
    }

    #[test]
    fn multiple_items_processed() {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        unsafe extern "C" fn test_work(_data: usize) {
            COUNT.fetch_add(1, Ordering::Release);
        }

        let wq = PriorityWorkQueue::new();
        for _ in 0..10 {
            assert!(wq.enqueue(SoftIrqPriority::Tasklet, test_work, 0));
        }
        assert_eq!(wq.pending(), 10);

        let count = wq.process_all();
        assert_eq!(count, 10);
        assert_eq!(COUNT.load(Ordering::Acquire), 10);
    }

    #[test]
    fn full_queue_returns_false() {
        unsafe extern "C" fn test_work(_data: usize) {}

        let wq = PriorityWorkQueue::new();
        // Fill the Hi queue (capacity-1 items)
        for _ in 0..QUEUE_CAPACITY - 1 {
            assert!(wq.enqueue(SoftIrqPriority::Hi, test_work, 0));
        }
        // Next enqueue at Hi should fail
        assert!(!wq.enqueue(SoftIrqPriority::Hi, test_work, 0));

        // Other priorities still work
        assert!(wq.enqueue(SoftIrqPriority::Tasklet, test_work, 0));

        // Process all, then can enqueue at Hi again
        wq.process_all();
        assert!(wq.enqueue(SoftIrqPriority::Hi, test_work, 0));
    }

    #[test]
    fn priority_ordering_hi_first() {
        static EXECUTION_ORDER: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);

        unsafe extern "C" fn hi_work(_data: usize) {
            let prev = EXECUTION_ORDER.fetch_add(10, Ordering::Release);
            assert_eq!(prev, 0, "Hi should run first");
        }
        unsafe extern "C" fn tasklet_work(_data: usize) {
            let prev = EXECUTION_ORDER.fetch_add(1, Ordering::Release);
            assert_eq!(prev, 10, "Tasklet should run after Hi");
        }
        unsafe extern "C" fn bg_work(_data: usize) {
            let prev = EXECUTION_ORDER.fetch_add(0, Ordering::Release);
            assert_eq!(prev, 11, "Background should run last");
        }

        let wq = PriorityWorkQueue::new();

        // Enqueue in reverse priority order
        assert!(wq.enqueue(SoftIrqPriority::Background, bg_work, 0));
        assert!(wq.enqueue(SoftIrqPriority::Tasklet, tasklet_work, 0));
        assert!(wq.enqueue(SoftIrqPriority::Hi, hi_work, 0));

        wq.process_all();
        assert_eq!(EXECUTION_ORDER.load(Ordering::Acquire), 11);
    }

    #[test]
    fn process_single_priority() {
        static HI_COUNT: AtomicUsize = AtomicUsize::new(0);
        static BG_COUNT: AtomicUsize = AtomicUsize::new(0);

        unsafe extern "C" fn hi_work(_data: usize) {
            HI_COUNT.fetch_add(1, Ordering::Release);
        }
        unsafe extern "C" fn bg_work(_data: usize) {
            BG_COUNT.fetch_add(1, Ordering::Release);
        }

        let wq = PriorityWorkQueue::new();
        assert!(wq.enqueue(SoftIrqPriority::Hi, hi_work, 0));
        assert!(wq.enqueue(SoftIrqPriority::Hi, hi_work, 0));
        assert!(wq.enqueue(SoftIrqPriority::Background, bg_work, 0));
        assert!(wq.enqueue(SoftIrqPriority::Background, bg_work, 0));

        // Only process Hi
        let count = wq.process_priority(SoftIrqPriority::Hi);
        assert_eq!(count, 2);
        assert_eq!(HI_COUNT.load(Ordering::Acquire), 2);
        assert_eq!(BG_COUNT.load(Ordering::Acquire), 0);

        // Remaining should be Background
        assert_eq!(wq.pending(), 2);
        let count = wq.process_priority(SoftIrqPriority::Background);
        assert_eq!(count, 2);
        assert_eq!(BG_COUNT.load(Ordering::Acquire), 2);
    }

    #[test]
    fn budgeted_processing() {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        unsafe extern "C" fn test_work(_data: usize) {
            COUNT.fetch_add(1, Ordering::Release);
        }

        let wq = PriorityWorkQueue::new();
        // Fill with 10 items at each priority
        for _ in 0..10 {
            assert!(wq.enqueue(SoftIrqPriority::Hi, test_work, 0));
            assert!(wq.enqueue(SoftIrqPriority::Tasklet, test_work, 0));
            assert!(wq.enqueue(SoftIrqPriority::Background, test_work, 0));
        }
        assert_eq!(wq.pending(), 30);

        // Budget of 10 with hi_ratio=60 → 6 Hi, 3 Tasklet, 1 Background
        let count = wq.process_budgeted(10, 60);
        assert_eq!(count, 10);
        assert_eq!(COUNT.load(Ordering::Acquire), 10);

        // Remaining: 4 Hi, 7 Tasklet, 9 Background = 20
        assert_eq!(wq.pending(), 20);
    }

    #[test]
    fn priorities_independent_capacity() {
        unsafe extern "C" fn test_work(_data: usize) {}

        let wq = PriorityWorkQueue::new();

        // Fill all three priority queues independently
        for p in [SoftIrqPriority::Hi, SoftIrqPriority::Tasklet, SoftIrqPriority::Background] {
            for _ in 0..QUEUE_CAPACITY - 1 {
                assert!(wq.enqueue(p, test_work, 0));
            }
            // Each should be full now
            assert!(!wq.enqueue(p, test_work, 0));
        }

        assert_eq!(wq.pending(), 3 * (QUEUE_CAPACITY - 1));

        // Process all
        let count = wq.process_all();
        assert_eq!(count, 3 * (QUEUE_CAPACITY - 1));
        assert!(wq.is_empty());
    }
}
