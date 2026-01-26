[![Build Status](https://img.shields.io/endpoint.svg?url=https%3A%2F%2Factions-badge.atrox.dev%2FHaveFunTrading%2Fmetricus%2Fbadge%3Fref%3Dmain&style=flat&label=build&logo=none)](https://actions-badge.atrox.dev/HaveFunTrading/metricus/goto?ref=main)
[![Crates.io](https://img.shields.io/crates/v/metricus.svg)](https://crates.io/crates/metricus)
[![Documentation](https://docs.rs/metricus/badge.svg)](https://docs.rs/metricus/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

# metric&#181;s

Low-latency metrics framework.

## Crates
- `metricus`: core types and the `Metrics` backend trait (`Counter`, `Histogram`).
- `metricus_agent`: metrics backend that uses background aggregator + exporters (UDP, file, unix sockets).
- `metricus_allocator`: optional counting global allocator.
- `metricus_macros`: `#[counter]` and `#[span]` helpers.

## Quick start

Initialize the default agent backend, then create counters and histograms with tags.

```rust
use metricus_agent::MetricsAgent;
use metricus::{empty_tags, Counter, CounterOps, Histogram, HistogramOps};

fn main() -> metricus_agent::Result<()> {
    MetricsAgent::init()?; // start background aggregator with default config

    // create counter with no tags
    let requests = Counter::new("requests_total", empty_tags());
    requests.increment();

    // create histogram with tags
    let latency = Histogram::new("request_latency_ns", &[("service", "api"), ("route", "/v1/orders")]);
    latency.record(1_250);

    Ok(())
}
```

Macros let you attach counters or spans directly to functions. This will automatically add
`fn_name` tag with instrument method name.

```rust
use metricus_macros::{counter, span};

#[counter(measurement = "requests", tags(service = "api"))]
fn handle_request() {
    // work
}

#[span(measurement = "latency", tags(service = "api"))]
fn handle_request_with_span() {
    // work
}
```

When you want to time a single block, create a span directly from a histogram.
```rust
use metricus::{Histogram, HistogramOps};

fn handle_request(histogram: &Histogram) {
    {
        let _span = histogram.span();
        // do the timed work, span records on drop
    }
    // remaining work
}
```

## Custom backends
The project ships with `metricus_agent` backend that uses background aggregator and various exporters. If you
wish to use your own custom backed you need to implement `metricus::Metrics` and register it via `metricus::set_metrics`. 

