 ADR-0006: Grafana Dashboard Architecture

## Status
**Accepted** - Implemented (July 2025)

## Context

obsctl requires comprehensive visualization of S3 operations, performance metrics, and system health. Users need real-time dashboards for monitoring production workloads, identifying performance bottlenecks, and tracking business metrics.

## Decision

Implement unified Grafana dashboard architecture with automated provisioning and comprehensive obsctl monitoring capabilities.

### Dashboard Strategy
- **Single Unified Dashboard** - obsctl-unified.json covering all operational aspects
- **Automated Provisioning** - Zero-configuration dashboard deployment
- **Real-time Monitoring** - 5-second refresh intervals for live operations
- **Enterprise-Grade Visualizations** - Production-ready monitoring panels

### Dashboard Architecture
```
Prometheus (Data Source) → Grafana (Visualization) → obsctl-unified.json (Dashboard)
```

## Implementation Details

### Dashboard Sections

#### 1. Operations Overview
- **Total Operations** - Real-time operation counter
- **Operation Types** - Breakdown by command (cp, sync, ls, rm, etc.)
- **Success Rate** - Operation success percentage
- **Error Rate** - Failed operations tracking

#### 2. Performance Metrics
- **Transfer Rates** - Upload/download speeds (KB/s)
- **Operation Duration** - Command execution times
- **Throughput** - Operations per minute/hour
- **Latency Percentiles** - P50, P95, P99 response times

#### 3. Business Metrics
- **Data Volume** - Bytes uploaded/downloaded over time
- **File Operations** - File counts and sizes
- **Bucket Operations** - Bucket creation/deletion tracking
- **Storage Usage** - Cumulative storage consumption

#### 4. System Health
- **Error Analysis** - Error types and frequencies
- **Resource Utilization** - Memory and CPU usage
- **Connection Health** - S3 endpoint connectivity
- **Queue Depths** - Operation queuing metrics

### Provisioning Strategy
- **Automated Deployment** - Dashboard loads automatically on Grafana startup
- **Package Integration** - Dashboard included in .deb/.rpm packages
- **Docker Compose** - Dashboard provisioned in development environment
- **Configuration Management** - obsctl config dashboard commands

### Visual Design
- **Corporate Theme** - Professional appearance for enterprise use
- **Color Coding** - Consistent color scheme for metric types
- **Responsive Layout** - Works on desktop and mobile devices
- **Interactive Elements** - Drill-down capabilities for detailed analysis

## Alternatives Considered

1. **Multiple Specialized Dashboards** - Rejected due to complexity
2. **Custom Web Interface** - Rejected due to maintenance overhead
3. **Command-Line Only Metrics** - Rejected due to poor UX
4. **Third-Party Monitoring Tools** - Rejected due to vendor lock-in

## Consequences

### Positive
- **Unified View** - Single dashboard for all obsctl monitoring
- **Zero Configuration** - Automatic deployment and setup
- **Real-time Insights** - Live monitoring of operations
- **Enterprise Ready** - Production-grade visualizations
- **Cost Effective** - Open-source solution
- **Extensible** - Easy to add new panels and metrics

### Negative
- **Grafana Dependency** - Requires Grafana infrastructure
- **Learning Curve** - Teams need dashboard interpretation skills
- **Resource Usage** - Additional memory/CPU for dashboard rendering
- **Maintenance** - Dashboard updates require coordination

## Dashboard Management

### Installation Commands
```bash
# Install dashboard from package
obsctl config dashboard install

# List available dashboards  
obsctl config dashboard list

# Remove dashboard
obsctl config dashboard remove obsctl-unified

# Show dashboard info
obsctl config dashboard info obsctl-unified
```

### Security Features
- **Restricted Scope** - Only manages obsctl-specific dashboards
- **Keyword Filtering** - Searches limited to 'obsctl' keyword
- **No Admin Access** - Cannot modify general Grafana configuration
- **Safe Operations** - Cannot delete non-obsctl dashboards

## Validation

### Success Criteria Met
- ✅ Unified dashboard displaying all obsctl metrics
- ✅ Automated provisioning working in Docker Compose
- ✅ Real-time updates with 5-second refresh
- ✅ Dashboard management commands operational
- ✅ Package integration with .deb/.rpm files
- ✅ Professional appearance suitable for enterprise use
- ✅ Interactive drill-down capabilities working

### Performance Validation
- Dashboard loads within 2 seconds
- Real-time updates without performance impact
- Responsive design tested on multiple screen sizes
- Memory usage within acceptable limits

## Migration Notes

Consolidated from multiple specialized dashboards to single unified dashboard:
- Eliminated 7 separate dashboard files
- Reduced maintenance complexity
- Improved user experience with single entry point
- Maintained all functionality in unified interface

## References
- [Grafana Dashboard Documentation](https://grafana.com/docs/grafana/latest/dashboards/)
- [Prometheus Data Source](https://grafana.com/docs/grafana/latest/datasources/prometheus/)
- [obsctl Dashboard Source](../packaging/dashboards/obsctl-unified.json)
- [Dashboard Management Commands](../src/commands/config.rs)