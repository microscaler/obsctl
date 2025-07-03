### Why you don’t see any metrics

Your **instrumentation code is fine**—the counters and histograms are being
“recorded”.
But **nothing is shipping those metrics anywhere** because **no meter provider /
export pipeline is ever installed** at runtime.

<details>
<summary>OpenTelemetry recap (click to expand)</summary>

```
 ┌───────────────┐     ┌───────────────┐     ┌─────────────────┐
 │  Instrumented │     │  SDK meter    │     │   Exporter      │
 │    code       │ ─►  │  provider     │ ─►  │  (OTLP/Prom…)   │ ─► Collector
 └───────────────┘     └───────────────┘     └─────────────────┘
```

* If you never create the middle box (SDK meter provider) the calls in your
  code just noop.
* The `opentelemetry::global::meter(..)` helper only looks up whatever meter
  provider is **currently registered**; by default that is a **no-op
  provider**.

</details>

---

## 1 – Add the metrics SDK + OTLP exporter

### `Cargo.toml`

```toml
[features]
default = []
otel = ["opentelemetry", "opentelemetry-otlp", "opentelemetry/sdk", "opentelemetry/metrics"]

[dependencies]
# telemetry
opentelemetry          = { version = "0.22", optional = true, default-features = false, features = ["metrics"] }
opentelemetry-otlp     = { version = "0.15", optional = true, default-features = false, features = ["grpc-tonic"] }
opentelemetry_sdk      = { version = "0.22", optional = true }

# enable when building with `--features otel`
```

*`opentelemetry/metrics`* is required; without it the metrics API is a
stub.

---

## 2 – Initialise the pipeline once during start-up

```rust
#[cfg(feature = "otel")]
fn init_otel_metrics(endpoint: &str, service_name: &str) -> anyhow::Result<()> {
    use opentelemetry::sdk::{
        metrics::{controllers, processors, selectors},
        Resource,
    };
    use opentelemetry::KeyValue;

    // ── Resource (service.* attributes) ───────────────────────────────
    let resource = Resource::new(vec![
        KeyValue::new("service.name", service_name.to_owned()),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
    ]);

    // ── Build the OTLP exporter over gRPC ─────────────────────────────
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(endpoint);

    // ── Periodic reader (export every 30 s) ───────────────────────────
    let reader = opentelemetry_otlp::new_pipeline()
        .metrics(exporter)
        .with_period(std::time::Duration::from_secs(30))
        .build();

    // ── Build a meter provider; use Delta temporality for Prom-style   –
    let controller = controllers::basic(processors::factory(
        selectors::simple::inexpensive(),
        // cumulative OR delta – pick what your collector expects
        opentelemetry::sdk::metrics::temporality::delta(),
    ))
    .with_resource(resource)
    .with_reader(reader)
    .build();

    // ── Make it globally visible to `global::meter()` helpers ─────────
    opentelemetry::global::set_meter_provider(controller);

    Ok(())
}
```

Call this **once** in `main` (before any metrics are recorded):

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ...
    #[cfg(feature = "otel")]
    if config.otel.enabled {
        init_otel_metrics(
            config.otel.endpoint.as_deref().unwrap_or("http://localhost:4317"),
            &config.otel.service_name,
        )?;
    }
    // ...
}
```

---

## 3 – Flush on shutdown

At the very end of `main` (or in a `ctrl_c` handler):

```rust
#[cfg(feature = "otel")]
opentelemetry::global::shutdown_tracer_provider(); // flush & close
```

*(One call shuts down **both** traces & metrics providers.)*

---

## 4 – Minor fixes & good practices

| Issue                                 | Quick Fix                                                                                                                         |
| ------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| **Creating instruments inside loops** | Store counters/histograms in a lazy static once, then just `.add()`/`.record()`; cheaper & keeps OTLP instrument cache small.     |
| **`build()` vs `init()`**             | In 0.22 the builder pattern uses `.init()`. (`.build()` still compiles but returns an instrument inside an `Arc`—fine but noisy.) |
| **No metrics in collector GUI**       | Confirm your collector is running **v0.93+** (older collectors disabled metrics by default).                                      |

---

### After these steps…

1. Run `obsctl` with `--feature otel` (or build with `--features otel`).
2. Open the OTLP collector dashboard → you should now see counters such as
   `obsctl_uploads_total`, `obsctl_errors_total`, histograms, etc.

If you still don’t see anything, enable debug logs for OTEL:

```bash
OTEL_LOG_LEVEL=debug ./obsctl ...
```

…and watch the exporter “Exporting metrics” messages.

---

Let me know if you’d like me to patch `Cargo.toml` and `main.rs` in the
canvas, or if you prefer a PR diff.
