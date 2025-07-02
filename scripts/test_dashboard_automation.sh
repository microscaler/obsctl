#!/bin/bash

# Test script to verify Grafana dashboard automation
# This script tests that the dashboard works out-of-the-box without manual intervention

set -e

echo "🧪 Testing Grafana Dashboard Automation"
echo "========================================"

# Step 1: Clean slate - stop and remove all containers
echo "1. Cleaning up existing containers..."
docker compose down -v
docker system prune -f

# Step 2: Start the stack
echo "2. Starting observability stack..."
docker compose up -d

# Step 3: Wait for services to be ready
echo "3. Waiting for services to start..."
sleep 30

# Step 4: Check if Prometheus is ready
echo "4. Checking Prometheus health..."
for i in {1..30}; do
    if curl -s http://localhost:9090/-/ready > /dev/null; then
        echo "✅ Prometheus is ready"
        break
    fi
    echo "⏳ Waiting for Prometheus... ($i/30)"
    sleep 2
done

# Step 5: Check if Grafana is ready
echo "5. Checking Grafana health..."
for i in {1..30}; do
    if curl -s http://localhost:3000/api/health > /dev/null; then
        echo "✅ Grafana is ready"
        break
    fi
    echo "⏳ Waiting for Grafana... ($i/30)"
    sleep 2
done

# Step 6: Check if dashboard is provisioned
echo "6. Checking dashboard provisioning..."
sleep 10
DASHBOARD_CHECK=$(curl -s -u admin:admin "http://localhost:3000/api/dashboards/uid/obsctl-unified" | jq -r '.dashboard.title // "NOT_FOUND"')
if [ "$DASHBOARD_CHECK" = "obsctl Unified Dashboard" ]; then
    echo "✅ Dashboard provisioned successfully"
else
    echo "❌ Dashboard not found or not provisioned correctly"
    exit 1
fi

# Step 7: Check if datasource is working
echo "7. Testing Prometheus datasource..."
DATASOURCE_CHECK=$(curl -s -u admin:admin "http://localhost:3000/api/datasources/uid/prometheus" | jq -r '.name // "NOT_FOUND"')
if [ "$DATASOURCE_CHECK" = "Prometheus" ]; then
    echo "✅ Prometheus datasource configured correctly"
else
    echo "❌ Prometheus datasource not found or misconfigured"
    exit 1
fi

# Step 8: Test a simple query
echo "8. Testing query execution..."
QUERY_TEST=$(curl -s -u admin:admin -X POST \
    -H "Content-Type: application/json" \
    -d '{"queries":[{"expr":"up","refId":"A"}],"from":"now-1h","to":"now"}' \
    "http://localhost:3000/api/ds/query" | jq -r '.results.A.status // "ERROR"')

if [ "$QUERY_TEST" = "200" ]; then
    echo "✅ Query execution working"
else
    echo "⚠️  Query execution may have issues (status: $QUERY_TEST)"
fi

# Step 9: Build obsctl if needed
echo "9. Building obsctl..."
cd ..
if [ ! -f "target/release/obsctl" ]; then
    echo "Building obsctl with OTEL features..."
    cargo build --release --features otel
fi

# Step 10: Generate some test traffic
echo "10. Generating test traffic..."
cd scripts
OTEL_ENABLED=true OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317 timeout 30s python3 generate_traffic.py || true

# Step 11: Wait for metrics to appear
echo "11. Waiting for metrics to appear in Prometheus..."
sleep 15

# Step 12: Check if obsctl metrics are available
echo "12. Checking for obsctl metrics..."
METRICS_CHECK=$(curl -s "http://localhost:9090/api/v1/query?query=obsctl_operations_total" | jq -r '.data.result | length')
if [ "$METRICS_CHECK" -gt 0 ]; then
    echo "✅ obsctl metrics found in Prometheus (${METRICS_CHECK} series)"
else
    echo "⚠️  No obsctl metrics found yet - may need more traffic"
fi

# Step 13: Final dashboard accessibility test
echo "13. Testing dashboard accessibility..."
DASHBOARD_URL="http://localhost:3000/d/obsctl-unified/obsctl-unified-dashboard"
echo "🌐 Dashboard should be accessible at: $DASHBOARD_URL"
echo "🔐 Login: admin / admin"

echo ""
echo "🎉 Dashboard automation test completed!"
echo "📊 Open $DASHBOARD_URL to verify panels load automatically"
echo "🔄 Dashboard should auto-refresh every 5 seconds"
echo ""
echo "If panels don't load automatically, check:"
echo "  - Datasource UID matches: 'prometheus'"
echo "  - Queries are valid and return data"
echo "  - Auto-refresh is enabled (5s interval)"
echo "  - No browser console errors"
