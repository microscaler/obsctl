# obsctl Grafana Dashboards

This directory contains Grafana dashboards for monitoring the obsctl S3 CLI tool and its observability stack.

## Available Dashboards

### 1. obsctl OTEL Collector Overview (`obsctl-overview`)
**Primary monitoring dashboard for the OpenTelemetry Collector**

- **OTEL Collector Status**: Real-time health status (UP/DOWN)
- **OTEL Collector Uptime**: How long the collector has been running
- **Exporter Queue Size**: Current queue size for telemetry export
- **Queue Usage %**: Percentage of queue capacity being used
- **Memory Usage**: RSS memory and heap allocation trends
- **CPU Usage**: Collector CPU utilization over time
- **Queue Metrics**: Queue size vs capacity trending

### 2. obsctl Distributed Tracing (`obsctl-traces`)
**Jaeger traces and distributed tracing visualization**

- **Recent obsctl Operations**: Table view of recent traces from obsctl operations
- **OTEL Collector Health**: Health monitoring for trace collection
- **Telemetry Queue Activity**: Queue activity for trace processing
- **Direct link to Jaeger UI**: Click the "Jaeger UI" link to view detailed traces

### 3. obsctl System Monitoring (`obsctl-system`)
**Infrastructure and system-level monitoring**

- **Service Status**: Health status for OTEL Collector and Prometheus
- **OTEL Uptime**: Collector uptime tracking
- **Queue Size**: Current telemetry queue size
- **Memory Usage**: Detailed memory consumption metrics
- **CPU Usage**: System CPU utilization
- **Prometheus TSDB Activity**: Database activity and metrics ingestion rates
- **Direct links**: Quick access to Prometheus UI and MinIO Console

## Dashboard Access

All dashboards are automatically provisioned and available at:
- **Grafana**: http://localhost:3000 (admin/admin)

## Related Services

- **Jaeger Traces**: http://localhost:16686
- **Prometheus Metrics**: http://localhost:9090  
- **MinIO Console**: http://localhost:9001 (minioadmin/minioadmin123)

## Understanding the Data

### Telemetry Flow
1. **obsctl** operations generate OpenTelemetry traces
2. **OTEL Collector** receives and processes traces via port 4317
3. **Jaeger** stores and displays distributed traces
4. **Prometheus** collects metrics about the collector itself
5. **Grafana** visualizes both metrics and provides trace access

### Key Metrics to Monitor

- **Queue Size**: Should remain low; high values indicate backpressure
- **Memory Usage**: Monitor for memory leaks or excessive consumption  
- **CPU Usage**: Track collector performance impact
- **Service Status**: Ensure all components are healthy (UP)

### Generating Test Data

Run integration tests to generate telemetry data:
```bash
# Generate comprehensive telemetry data
tests/integration/run_tests.sh observability --verbose

# Generate specific test patterns
tests/integration/run_tests.sh performance
tests/integration/run_tests.sh concurrent
```

## Troubleshooting

### No Data in Dashboards
1. Verify all services are running: `docker compose ps`
2. Check OTEL Collector logs: `docker compose logs otel-collector`
3. Run observability tests to generate data
4. Ensure obsctl is built with OTEL features: `cargo build --features otel`

### Missing Traces in Jaeger
1. Check that obsctl operations are using OTEL endpoint: `http://localhost:4317`
2. Verify service name in traces: `obsctl-integration-test`
3. Check OTEL Collector configuration for Jaeger export

### Dashboard Errors
1. Restart Grafana: `docker compose restart grafana`
2. Check datasource connections in Grafana UI
3. Verify Prometheus is scraping metrics: http://localhost:9090/targets

## Customization

Dashboards are provisioned from JSON files in `/var/lib/grafana/dashboards`. 
To modify:
1. Edit the JSON files in `.docker/grafana/dashboards/`
2. Restart Grafana: `docker compose restart grafana`
3. Changes will be automatically loaded

## Performance Notes

- Dashboards refresh every 5 seconds by default
- Historical data retention follows Prometheus configuration
- Jaeger traces are stored in memory (ephemeral)
- For production use, configure persistent storage for both Prometheus and Jaeger 