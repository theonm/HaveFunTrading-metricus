# metricus-allocator

Contains allocator.

## Usage notes

- Call `metricus::set_metrics` before enabling allocator instrumentation if you expect allocation counters to emit.
- Call `enable_allocator_instrumentation` for each thread that should report allocation metrics.
