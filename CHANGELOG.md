# Changelog

## Unreleased
- Mark `MetricsHandle` as `Send` and `Sync` with an explicit safety contract: it is only safe if backends are either thread-safe (internally synchronized) or are used in a strictly single-threaded manner. This allows allocator counters to live in a `static LazyLock` and fixes the `*mut u8`-derived `Sync` error without changing runtime behavior.
- Clarify allocator instrumentation behavior: allocation counters cache the active handle on first use, so `set_metrics` must run before allocator instrumentation is enabled if you expect these counters to emit.
- Move the release configuration to `[workspace.metadata.release]` to eliminate Cargo's unused manifest key warning while preserving the same metadata for release tooling.
- Remove an extra blank line after the `Histogram` doc comment to satisfy `clippy::empty_line_after_doc_comments` (no behavior change).
- Skip span timing work when the no-op metrics handle is active, returning a no-op span that avoids `Instant::now()`/`rdtsc` and record calls. This reduces overhead when metrics are disabled while keeping behavior unchanged when a real backend is set.
- Avoid u128 nanosecond conversion on non-`rdtsc` span drops by computing nanos from seconds + subsecond nanos with wrapping arithmetic. This keeps the u64 return value behavior while trimming a small amount of conversion overhead.
- Cache the active metrics handle inside `Counter`/`Histogram` so hot-path operations avoid an atomic load. This tightens the initialization contract: `set_metrics` must run before any counters/histograms (including macro statics) are created.
- Replace mutable global metrics access with an immutable handle, moving metrics vtable calls to `&self` and using relaxed loads plus an explicit SeqCst store for the global handle in `set_metrics`. This removes `static mut` access and `get_mut` usage, reducing unsafe mutable aliasing while keeping hot-path reads fast when initialization happens before worker threads.
- Switch counters/histograms and proc-macro statics to `LazyCell` with `LazyCell::force`, dropping `UnsafeCell` and `LazyLock` implementations. This narrows the API surface to a single lazy init path and avoids raw mutable access inside statics while preserving lazy initialization semantics.
- Keep exporter publish signatures using the `Counters`/`Histograms` aliases (including top-level/UDP exporters) for consistency, even though they are just `HashMap<Id, _>` type aliases.
- Add a default-on `span` feature: when disabled, `Span` becomes a no-op type and timing code is compiled out; when enabled, span timing behaves as before.

This refactor makes `MetricsHandle` immutable because it is just a vtable pointer and backend synchronization lives behind it, so callers do not need mutable access to the handle. With an immutable global handle, we can remove `static mut` and `&mut` aliasing, and then use `LazyCell::force` to get shared references to counters and histograms without `UnsafeCell`, making the API cleaner and safer.

Breaking: `CounterOps`/`HistogramOps` are no longer implemented for `LazyLock<UnsafeCell<...>>`. Code using `LazyLock` should switch to `LazyCell` or use a plain `Counter`/`Histogram` value.
Behavioral note: `set_metrics` must run before counters/histograms are created (including macro statics) and before worker threads start; metrics cache the handle and relaxed loads may otherwise observe the old backend.
