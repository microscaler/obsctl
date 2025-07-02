# OTEL SDK Migration PRD

## Current State
- Manual HTTP requests to OTEL collector using `reqwest`
- Broken `init_tracing()` function with compilation errors
- Mixed approach: proper metrics collection but manual emission
- Only `cp` and `sync` commands have OTEL integration

## Target State
- Proper OpenTelemetry Rust SDK usage
- Automatic metrics emission via SDK instruments
- All commands instrumented with OTEL
- gRPC-only communication to OTEL collector

## Migration Plan

### Phase 1: Fix OTEL SDK Initialization
1. **Fix `init_tracing()` function**
   - Remove broken SDK calls
   - Implement proper OTLP exporter setup
   - Use correct SDK APIs for Rust
   - gRPC-only endpoint configuration

2. **Remove manual HTTP approach**
   - Delete `send_otel_telemetry()` function
   - Delete `emit_otel_metrics()` manual HTTP calls
   - Delete `emit_otel_error()` manual HTTP calls

### Phase 2: Implement Proper SDK Instrumentation
1. **Create proper OTEL instruments**
   - Counters for operations (uploads, downloads, etc.)
   - Histograms for operation duration
   - Gauges for current state metrics
   - Use global meter provider

2. **Update ObsctlMetrics to use OTEL instruments**
   - Replace manual counters with OTEL counters
   - Automatic emission via SDK
   - Remove manual JSON payload creation

### Phase 3: Instrument All Commands
1. **Add OTEL to missing commands**
   - `ls` command (currently missing)
   - `rm` command
   - `du` command
   - `presign` command
   - `head_object` command

2. **Use proper span instrumentation**
   - Wrap operations in spans
   - Add relevant attributes
   - Automatic error recording

### Phase 4: Testing and Validation
1. **Verify metrics flow**
   - obsctl → OTEL collector (gRPC) → Prometheus
   - Check Prometheus metrics endpoint
   - Validate Grafana dashboards

## Implementation Tasks

### Task 1: Fix `init_tracing()` function
- [ ] Remove broken `opentelemetry_otlp::new_pipeline()` calls
- [ ] Implement proper OTLP gRPC exporter
- [ ] Set up global meter and tracer providers
- [ ] Test OTEL initialization

### Task 2: Replace manual HTTP with SDK instruments
- [ ] Create proper OTEL counters in `ObsctlMetrics`
- [ ] Remove `send_otel_telemetry()` function
- [ ] Remove manual JSON payload creation
- [ ] Update command files to use SDK

### Task 3: Add OTEL to all commands
- [ ] Instrument `ls` command
- [ ] Instrument `rm` command  
- [ ] Instrument `du` command
- [ ] Instrument `presign` command
- [ ] Instrument `head_object` command

### Task 4: End-to-end testing
- [ ] Run traffic generator
- [ ] Verify metrics in Prometheus
- [ ] Check OTEL collector logs
- [ ] Validate Grafana dashboards

## Success Criteria
1. No manual HTTP requests to OTEL collector
2. All metrics automatically emitted via SDK
3. All commands instrumented with OTEL
4. Metrics visible in Prometheus
5. Traffic generator produces visible metrics

## Dependencies
- OTEL collector configured for gRPC-only
- Prometheus scraping OTEL collector metrics
- Rust OTEL SDK dependencies in Cargo.toml 