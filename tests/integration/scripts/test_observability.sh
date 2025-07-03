#!/bin/bash
# shellcheck disable=SC2034,SC2155  # Variables may be used in sourcing scripts, declare separately

# Observability Integration Tests for obsctl
# Focused on generating telemetry data for dashboards and monitoring

# Ensure this script is being sourced, not executed
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    echo "ERROR: This script should be sourced, not executed directly"
    exit 1
fi

# Function to run observability tests
run_observability_tests() {
    print_header "Starting Observability Integration Tests"

    if [[ "$OTEL_ENABLED" != "true" ]]; then
        print_warning "OpenTelemetry is disabled. Observability tests will not generate telemetry data."
        print_info "Enable OTEL with: --otel true"
    fi

    # Setup test environment
    setup_test_environment

    print_info "Generating telemetry data for observability dashboards..."

    # Test 1: Generate diverse operation traces
    print_info "Test 1: Generating Operation Traces"
    generate_operation_traces

    # Test 2: Generate performance metrics
    print_info "Test 2: Generating Performance Metrics"
    generate_performance_metrics

    # Test 3: Generate error scenarios for monitoring
    print_info "Test 3: Generating Error Scenarios"
    generate_error_scenarios

    # Test 4: Generate concurrent operation traces
    print_info "Test 4: Generating Concurrent Operation Traces"
    generate_concurrent_traces

    # Test 5: Generate different file size patterns
    print_info "Test 5: Generating File Size Pattern Traces"
    generate_file_size_patterns

    print_success "All observability tests completed successfully"

    # Show dashboard information
    show_dashboard_info

    # Generate performance report
    generate_performance_report
}

# Generate traces for different operations
generate_operation_traces() {
    print_verbose "Generating traces for different obsctl operations"

    local operations=(
        "ls"
        "mb"
        "cp"
        "head-object"
        "presign"
        "du"
        "rm"
        "rb"
    )

    # Create test bucket for operations
    local trace_bucket="$CURRENT_TEST_BUCKET-traces"
    run_obsctl mb "s3://$trace_bucket"

    # Create test file
    local test_file="$TEST_DATA_DIR/trace_test.txt"
    generate_test_file "$test_file" "2048" "text"

    # Generate traces for each operation
    for operation in "${operations[@]}"; do
        print_verbose "Generating trace for operation: $operation"

        case "$operation" in
            "ls")
                run_obsctl ls "s3://$trace_bucket"
                ;;
            "mb")
                local temp_bucket="$trace_bucket-temp-$(date +%s)"
                run_obsctl mb "s3://$temp_bucket"
                run_obsctl rb "s3://$temp_bucket"
                ;;
            "cp")
                run_obsctl cp "$test_file" "s3://$trace_bucket/trace_test.txt"
                ;;
            "head-object")
                local bucket_name="${trace_bucket}"
                local key_name="trace_test.txt"
                run_obsctl head-object --bucket "$bucket_name" --key "$key_name" || true
                ;;
            "presign")
                run_obsctl presign "s3://$trace_bucket/trace_test.txt" || true
                ;;
            "du")
                run_obsctl du "s3://$trace_bucket" || true
                ;;
            "rm")
                run_obsctl rm "s3://$trace_bucket/trace_test.txt" || true
                ;;
            "rb")
                run_obsctl rb "s3://$trace_bucket" || true
                ;;
        esac

        # Small delay to separate traces
        sleep 1
    done

    print_verbose "Operation traces generated successfully"
}

# Generate performance metrics with different scenarios
generate_performance_metrics() {
    print_verbose "Generating performance metrics for different scenarios"

    # Create files of different sizes
    local file_sizes=(
        "1024:small"      # 1KB
        "10240:medium"    # 10KB
        "102400:large"    # 100KB
        "1048576:xlarge"  # 1MB
    )

    local perf_bucket="$CURRENT_TEST_BUCKET-perf"
    run_obsctl mb "s3://$perf_bucket"

    for size_spec in "${file_sizes[@]}"; do
        local size="${size_spec%:*}"
        local label="${size_spec#*:}"

        print_verbose "Testing performance with $label file ($size bytes)"

        # Create test file
        local test_file="$TEST_DATA_DIR/perf_${label}.bin"
        generate_test_file "$test_file" "$size" "random"

        # Upload with timing
        local upload_duration
        upload_duration=$(measure_time run_obsctl cp "$test_file" "s3://$perf_bucket/perf_${label}.bin")
        track_performance "perf_upload_${label}" "$upload_duration" "$size"

        # Download with timing
        local download_file="$TEST_DATA_DIR/downloaded_perf_${label}.bin"
        local download_duration
        download_duration=$(measure_time run_obsctl cp "s3://$perf_bucket/perf_${label}.bin" "$download_file")
        track_performance "perf_download_${label}" "$download_duration" "$size"

        # Head object with timing
        local head_duration
        head_duration=$(measure_time run_obsctl head-object --bucket "$perf_bucket" --key "perf_${label}.bin")
        track_performance "perf_head_${label}" "$head_duration"

        # Small delay between tests
        sleep 0.5
    done

    # Cleanup performance bucket
    run_obsctl rm "s3://$perf_bucket/" --recursive || true
    run_obsctl rb "s3://$perf_bucket" || true

    print_verbose "Performance metrics generated successfully"
}

# Generate error scenarios for monitoring
generate_error_scenarios() {
    print_verbose "Generating error scenarios for monitoring and alerting"

    local error_scenarios=(
        "non_existent_bucket"
        "non_existent_object"
        "invalid_s3_uri"
        "permission_denied"
        "network_timeout"
    )

    for scenario in "${error_scenarios[@]}"; do
        print_verbose "Generating error scenario: $scenario"

        case "$scenario" in
            "non_existent_bucket")
                run_obsctl ls "s3://non-existent-bucket-$(date +%s)" 2>/dev/null || true
                ;;
            "non_existent_object")
                run_obsctl cp "s3://$CURRENT_TEST_BUCKET/non-existent-file.txt" "/tmp/should-fail" 2>/dev/null || true
                ;;
            "invalid_s3_uri")
                run_obsctl ls "invalid-uri" 2>/dev/null || true
                ;;
            "permission_denied")
                # Try to access a bucket that doesn't exist (simulates permission error)
                run_obsctl ls "s3://aws-logs" 2>/dev/null || true
                ;;
            "network_timeout")
                # This would require a more complex setup, so we'll skip for now
                print_verbose "Skipping network timeout simulation"
                ;;
        esac

        # Small delay between error scenarios
        sleep 0.5
    done

    print_verbose "Error scenarios generated successfully"
}

# Generate concurrent operation traces
generate_concurrent_traces() {
    print_verbose "Generating concurrent operation traces"

    local concurrent_bucket="$CURRENT_TEST_BUCKET-concurrent"
    run_obsctl mb "s3://$concurrent_bucket"

    # Create test files for concurrent operations
    local test_files=()
    for i in {1..5}; do
        local test_file="$TEST_DATA_DIR/concurrent_${i}.txt"
        generate_test_file "$test_file" "5120" "text"  # 5KB files
        test_files+=("$test_file")
    done

    print_verbose "Starting concurrent uploads..."

    # Start concurrent uploads (background processes)
    local pids=()
    for i in "${!test_files[@]}"; do
        local file="${test_files[$i]}"
        local s3_key="concurrent_${i}.txt"
        (
            run_obsctl cp "$file" "s3://$concurrent_bucket/$s3_key"
        ) &
        pids+=($!)
    done

    # Wait for all uploads to complete
    for pid in "${pids[@]}"; do
        wait "$pid"
    done

    print_verbose "Concurrent uploads completed"

    # Start concurrent downloads
    print_verbose "Starting concurrent downloads..."
    local download_pids=()
    for i in {0..4}; do
        local s3_key="concurrent_${i}.txt"
        local download_file="$TEST_DATA_DIR/downloaded_concurrent_${i}.txt"
        (
            run_obsctl cp "s3://$concurrent_bucket/$s3_key" "$download_file"
        ) &
        download_pids+=($!)
    done

    # Wait for all downloads to complete
    for pid in "${download_pids[@]}"; do
        wait "$pid"
    done

    print_verbose "Concurrent downloads completed"

    # Cleanup concurrent bucket
    run_obsctl rm "s3://$concurrent_bucket/" --recursive || true
    run_obsctl rb "s3://$concurrent_bucket" || true

    print_verbose "Concurrent operation traces generated successfully"
}

# Generate file size pattern traces
generate_file_size_patterns() {
    print_verbose "Generating traces for different file size patterns"

    local pattern_bucket="$CURRENT_TEST_BUCKET-patterns"
    run_obsctl mb "s3://$pattern_bucket"

    # Pattern 1: Exponentially increasing file sizes
    print_verbose "Pattern 1: Exponential size increase"
    local exp_sizes=(1024 2048 4096 8192 16384 32768)
    for i in "${!exp_sizes[@]}"; do
        local size="${exp_sizes[$i]}"
        local file="$TEST_DATA_DIR/exp_${i}.bin"
        generate_test_file "$file" "$size" "random"

        local duration
        duration=$(measure_time run_obsctl cp "$file" "s3://$pattern_bucket/exp_${i}.bin")
        track_performance "pattern_exp_${i}" "$duration" "$size"
    done

    # Pattern 2: Random file sizes
    print_verbose "Pattern 2: Random file sizes"
    for i in {1..10}; do
        local size=$((RANDOM % 50000 + 1000))  # Random size between 1KB and 50KB
        local file="$TEST_DATA_DIR/random_${i}.bin"
        generate_test_file "$file" "$size" "random"

        local duration
        duration=$(measure_time run_obsctl cp "$file" "s3://$pattern_bucket/random_${i}.bin")
        track_performance "pattern_random_${i}" "$duration" "$size"
    done

    # Pattern 3: Batch operations
    print_verbose "Pattern 3: Batch operations"
    local batch_dir="$TEST_DATA_DIR/batch"
    mkdir -p "$batch_dir"

    # Create multiple small files
    for i in {1..20}; do
        generate_test_file "$batch_dir/batch_${i}.txt" "512" "text"
    done

    # Sync entire directory
    local sync_duration
    sync_duration=$(measure_time run_obsctl sync "$batch_dir/" "s3://$pattern_bucket/batch/")
    track_performance "pattern_batch_sync" "$sync_duration"

    # Cleanup pattern bucket
    run_obsctl rm "s3://$pattern_bucket/" --recursive || true
    run_obsctl rb "s3://$pattern_bucket" || true

    print_verbose "File size pattern traces generated successfully"
}

# Show dashboard information
show_dashboard_info() {
    print_header "Observability Dashboard Information"

    if [[ "$OTEL_ENABLED" == "true" ]]; then
        print_success "Telemetry data has been generated and sent to the observability stack!"
        echo ""
        print_info "Check the following dashboards for telemetry data:"
        echo "  🎯 Grafana Dashboard:    http://localhost:3000"
        echo "     Username: admin"
        echo "     Password: admin"
        echo ""
        echo "  🔍 Jaeger Tracing:       http://localhost:16686"
        echo "     Service: obsctl-integration-test"
        echo ""
        echo "  📊 Prometheus Metrics:   http://localhost:9090"
        echo "     Query examples:"
        echo "       - up{job=\"otel-collector\"}"
        echo "       - otelcol_process_uptime"
        echo ""
        echo "  💾 MinIO Console:        http://localhost:9001"
        echo "     Username: minioadmin"
        echo "     Password: minioadmin123"
        echo ""
        print_info "The tests generated the following types of telemetry:"
        echo "  • Operation traces (upload, download, list, etc.)"
        echo "  • Performance metrics (duration, throughput)"
        echo "  • Error scenarios (for alerting)"
        echo "  • Concurrent operation patterns"
        echo "  • File size distribution patterns"
        echo ""
        print_warning "Note: It may take 1-2 minutes for all telemetry data to appear in the dashboards"
    else
        print_warning "OpenTelemetry was disabled during this test run"
        print_info "To generate telemetry data, run with: --otel true"
    fi
}

print_verbose "Observability test module loaded successfully"
