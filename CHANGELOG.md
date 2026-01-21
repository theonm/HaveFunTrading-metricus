# Changelog

## Unreleased
- Replace mutable global metrics access with an immutable handle, moving metrics vtable calls to `&self` and using relaxed loads plus release stores for the global handle. This removes `static mut` access and `get_mut` usage, reducing unsafe mutable aliasing while keeping hot-path reads fast when initialization happens before worker threads.
- Switch counters/histograms and proc-macro statics to `LazyCell` with `LazyCell::force`, dropping `UnsafeCell` and `LazyLock` implementations. This narrows the API surface to a single lazy init path and avoids raw mutable access inside statics while preserving lazy initialization semantics.
- Update exporter publish signatures so the top-level API and UDP exporter accept `HashMap<Id, _>` directly, while stream/unix datagram keep the `Counters`/`Histograms` aliases. This aligns the public entry point with the concrete map used by aggregation and keeps the alias-based internal interfaces intact to avoid extra refactors.
  In practice, `Counters`/`Histograms` are just aliases for `HashMap<Id, Counter>` and `HashMap<Id, Histogram>`, so the change only makes the public signatures explicit about the underlying type. This removes an unnecessary layer of indirection at the API boundary without changing behavior.

This refactor makes `MetricsHandle` immutable because it is just a vtable pointer and backend synchronization lives behind it, so callers do not need mutable access to the handle. With an immutable global handle, we can remove `static mut` and `&mut` aliasing, and then use `LazyCell::force` to get shared references to counters and histograms without `UnsafeCell`, making the API cleaner and safer.

Breaking: `CounterOps`/`HistogramOps` are no longer implemented for `LazyLock<UnsafeCell<...>>`. Code using `LazyLock` should switch to `LazyCell` or use a plain `Counter`/`Histogram` value.
Behavioral note: `set_metrics` should run before worker threads start; late initialization may observe the old handle because loads are now relaxed.
