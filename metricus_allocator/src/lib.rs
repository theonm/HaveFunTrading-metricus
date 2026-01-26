#![doc = include_str!("../README.md")]

use metricus::{Counter, CounterOps, Id, PreAllocatedMetric};
use std::alloc::{GlobalAlloc, Layout};
use std::cell::Cell;
use std::sync::LazyLock;

const ALLOC_COUNTER_ID: Id = Id::MAX - 1004;
const ALLOC_BYTES_COUNTER_ID: Id = Id::MAX - 1003;
const DEALLOC_COUNTER_ID: Id = Id::MAX - 1002;
const DEALLOC_BYTES_COUNTER_ID: Id = Id::MAX - 1001;

const fn get_aligned_size(layout: Layout) -> usize {
    let alignment_mask: usize = layout.align() - 1;
    (layout.size() + alignment_mask) & !alignment_mask
}

/// This allocator will use instrumentation to count the number of allocations and de-allocations
/// occurring in the program. All calls to allocate (and free) memory are delegated to the concrete
/// allocator (`std::alloc::System` by default). Once the allocator has been registered as
/// `global_allocator` you need to call [enable_allocator_instrumentation] from each thread that
/// wants to include its allocation and de-allocation metrics.
///
/// ```no_run
/// use metricus_allocator::CountingAllocator;
///
/// #[global_allocator]
/// static GLOBAL: CountingAllocator = CountingAllocator;
/// ```
pub struct CountingAllocator;

#[allow(static_mut_refs)]
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // provide metrics only if instrumentation has been enabled for this thread
        if INSTRUMENTATION_ENABLED.get() {
            COUNTERS.alloc_count.increment();
            COUNTERS.alloc_bytes.increment_by(get_aligned_size(layout) as u64);
        }

        // delegate to the appropriate allocator
        #[cfg(all(feature = "jemalloc", not(feature = "mimalloc")))]
        {
            return unsafe { jemallocator::Jemalloc.alloc(layout) };
        }
        #[cfg(all(feature = "mimalloc", not(feature = "jemalloc")))]
        {
            return unsafe { mimalloc::MiMalloc.alloc(layout) };
        }
        unsafe { std::alloc::System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // provide metrics only if instrumentation has been enabled for this thread
        if INSTRUMENTATION_ENABLED.get() {
            COUNTERS.dealloc_count.increment();
            COUNTERS.dealloc_bytes.increment_by(get_aligned_size(layout) as u64);
        }

        // delegate to the appropriate allocator
        #[cfg(all(feature = "jemalloc", not(feature = "mimalloc")))]
        {
            unsafe {
                jemallocator::Jemalloc.dealloc(ptr, layout);
            }
            return;
        }
        #[cfg(all(feature = "mimalloc", not(feature = "jemalloc")))]
        {
            unsafe {
                mimalloc::MiMalloc.dealloc(ptr, layout);
            }
            return;
        }
        unsafe { std::alloc::System.dealloc(ptr, layout) }
    }
}

impl CountingAllocator {
    /// Default counters to be used with the `CountingAllocator`.
    pub fn metrics() -> Vec<PreAllocatedMetric> {
        vec![
            PreAllocatedMetric::counter("global_allocator", ALLOC_COUNTER_ID, &[("fn_name", "alloc")]),
            PreAllocatedMetric::counter("global_allocator", ALLOC_BYTES_COUNTER_ID, &[("fn_name", "alloc_bytes")]),
            PreAllocatedMetric::counter("global_allocator", DEALLOC_COUNTER_ID, &[("fn_name", "dealloc")]),
            PreAllocatedMetric::counter("global_allocator", DEALLOC_BYTES_COUNTER_ID, &[("fn_name", "dealloc_bytes")]),
        ]
    }
}

thread_local! {
    static INSTRUMENTATION_ENABLED: Cell<bool> = const { Cell::new(false) };
}

/// This should be called by a thread that wants to opt in to send allocation and de-allocation
/// metrics. By default, per thread instrumentation is disabled. This is usually backend dependent
/// as some backends can support sending metrics from multiple threads whereas others can be limited
/// to the main thread only.
///
/// ## Examples
///
/// Enable instrumentation for the main thread.
/// ```no_run
///
/// use metricus_allocator::enable_allocator_instrumentation;
/// use metricus_allocator::CountingAllocator;
///
/// #[global_allocator]
/// static GLOBAL: CountingAllocator = CountingAllocator;
///
/// fn main() {
///     enable_allocator_instrumentation();
/// }
/// ```
///
/// Enable instrumentation for the background thread.
/// ```no_run
///
/// use metricus_allocator::enable_allocator_instrumentation;
/// use metricus_allocator::CountingAllocator;
///
/// #[global_allocator]
/// static GLOBAL: CountingAllocator = CountingAllocator;
///
/// fn main() {
///     let _ = std::thread::spawn(|| {
///         enable_allocator_instrumentation();
///     });
/// }
/// ```
pub fn enable_allocator_instrumentation() {
    INSTRUMENTATION_ENABLED.set(true);
}

static COUNTERS: LazyLock<Counters> = LazyLock::new(|| Counters {
    // `counter_with_id` creates a counter object without registering it.
    // These allocation counters are created lazily on first use and cache the active metrics handle.
    // If they are initialized before `set_metrics`, they will remain bound to the no-op backend.
    // Ensure the backend is set before enabling allocator instrumentation if you want these to emit.
    alloc_count: Counter::new_with_id(ALLOC_COUNTER_ID),
    alloc_bytes: Counter::new_with_id(ALLOC_BYTES_COUNTER_ID),
    dealloc_count: Counter::new_with_id(DEALLOC_COUNTER_ID),
    dealloc_bytes: Counter::new_with_id(DEALLOC_BYTES_COUNTER_ID),
});

struct Counters {
    alloc_count: Counter,
    alloc_bytes: Counter,
    dealloc_count: Counter,
    dealloc_bytes: Counter,
}
