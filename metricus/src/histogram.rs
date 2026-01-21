//! A `Histogram` proxy struct for managing a metrics histogram.

use crate::access::get_metrics;
use crate::{Id, Tags};
#[cfg(feature = "rdtsc")]
use quanta::Clock;
use std::cell::LazyCell;
#[cfg(not(feature = "rdtsc"))]
use std::time::Instant;

/// Facilitates the creation of a new histogram, recording of values, and
/// generation of spans for timing operations.
/// The `Histogram` does not have an inherent notion of measurement units (e.g., milliseconds, bytes)
/// and some convention should be in place.
///
/// ## Examples
///
/// Create a histogram and record values without specifying units explicitly:
///
/// ```no_run
/// use metricus::{Histogram, HistogramOps};
///
/// let tags = [("operation", "db_query"), ("status", "success")];
/// let histogram = Histogram::new("query_duration", &tags);
///
/// // Here, 1500 might represent microseconds, but it's implied and must be consistent in usage.
/// histogram.record(1500);
/// ```
///
/// Another option is to use `#[span]` macro to instrument your code to automatically measure duration
/// of a given function.
///
/// ```no_run
/// use metricus_macros::span;
///
/// #[span(measurement = "latencies", tags(key1 = "value1", key2 = "value2"))]
/// fn my_function_with_tags() {
///     // function body
/// }
///
/// my_function_with_tags();
/// ````

#[derive(Debug)]
pub struct Histogram {
    id: Id,
    #[cfg(feature = "rdtsc")]
    clock: Clock,
}

impl Histogram {
    /// Creates a new histogram with the specified name and tags.
    /// Units of measurement are not defined by the histogram itself but should be implied
    /// and consistently used based on the metric being tracked.
    ///
    /// ## Examples
    ///
    /// Create a histogram with tags.
    /// ```no_run
    /// use metricus::Histogram;
    ///
    /// let tags = [("feature", "login"), ("result", "success")];
    /// let histogram = Histogram::new("login_duration", &tags);
    /// ```
    ///
    /// Create a histogram without tags.
    /// ```no_run
    /// use metricus::{empty_tags, Histogram};
    ///
    /// let histogram = Histogram::new("login_duration", empty_tags());
    /// ```
    pub fn new(name: &str, tags: Tags) -> Self {
        let histogram_id = get_metrics().new_histogram(name, tags);
        Self {
            id: histogram_id,
            #[cfg(feature = "rdtsc")]
            clock: Clock::new(),
        }
    }
}

/// Defines a series of operations that can be performed on a `Histogram`.
pub trait HistogramOps {
    /// Records a value in the histogram.
    /// The unit of the value is implied and should be consistent with the histogram's intended use.
    ///
    /// ```no_run
    /// use metricus::{Histogram, HistogramOps};
    ///
    /// let histogram = Histogram::new("response_time", &[]);
    /// // Assuming milliseconds as the unit for response time.
    /// histogram.record(200);
    /// ```
    fn record(&self, value: u64);

    /// Starts a span for timing an operation, automatically recording the duration upon completion.
    /// The duration recorded is in nanoseconds.
    ///
    /// ```no_run
    /// use metricus::{Histogram, HistogramOps};
    /// let histogram = Histogram::new("task_duration", &[]);
    /// {
    ///     let _span = histogram.span(); // Timing starts here, in nanoseconds.
    ///     // Execute operation...
    /// } // Timing ends and duration is recorded here.
    /// ```
    ///
    /// It is important to use a named binding when assigning the `Span` instead of `let _ = histogram.span()`.
    /// The latter form will result in `Span` being dropped immediately. Instead, prefer to use the [Histogram::with_span]
    /// method to prevent any miss-use.
    fn span(&self) -> Span<'_>;

    /// Accepts a closure whose duration will be measured. The duration recorded is in nanoseconds.
    ///
    /// ```no_run
    /// use metricus::{Histogram, HistogramOps};
    ///
    /// let histogram = Histogram::new("task_duration", &[]);
    /// histogram.with_span(|| {
    ///   // Execute operation...
    /// });
    /// ```
    fn with_span<F: FnOnce() -> R, R>(&self, f: F) -> R;
}

impl HistogramOps for Histogram {
    #[inline]
    fn record(&self, value: u64) {
        get_metrics().record(self.id, value);
    }

    #[inline]
    fn span(&self) -> Span<'_> {
        Span {
            histogram: self,
            #[cfg(feature = "rdtsc")]
            start_raw: self.clock.raw(),
            #[cfg(not(feature = "rdtsc"))]
            start_instant: Instant::now(),
        }
    }

    #[inline]
    fn with_span<F: FnOnce() -> R, R>(&self, f: F) -> R {
        let _span = self.span();
        f()
    }
}

impl<F: FnOnce() -> Histogram> HistogramOps for LazyCell<Histogram, F> {
    #[inline]
    fn record(&self, value: u64) {
        LazyCell::force(self).record(value)
    }

    #[inline]
    fn span(&self) -> Span<'_> {
        LazyCell::force(self).span()
    }

    #[inline]
    fn with_span<G: FnOnce() -> R, R>(&self, f: G) -> R {
        LazyCell::force(self).with_span(f)
    }
}

impl Drop for Histogram {
    fn drop(&mut self) {
        get_metrics().delete_histogram(self.id);
    }
}

/// Used for measuring how long given operation takes. The duration is recorded in nanoseconds.
pub struct Span<'a> {
    histogram: &'a Histogram,
    #[cfg(feature = "rdtsc")]
    start_raw: u64,
    #[cfg(not(feature = "rdtsc"))]
    start_instant: Instant,
}

impl Drop for Span<'_> {
    fn drop(&mut self) {
        #[cfg(feature = "rdtsc")]
        {
            let end_raw = self.histogram.clock.raw();
            let elapsed = self.histogram.clock.delta_as_nanos(self.start_raw, end_raw);
            self.histogram.record(elapsed);
        }
        #[cfg(not(feature = "rdtsc"))]
        self.histogram.record(self.start_instant.elapsed().as_nanos() as u64);
    }
}
