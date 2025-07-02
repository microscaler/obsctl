#!/bin/bash
# shellcheck disable=SC2034  # Variables may be used in sourcing scripts

# Performance Integration Tests for obsctl
# Focused on performance benchmarking and stress testing

# Ensure this script is being sourced, not executed
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    echo "ERROR: This script should be sourced, not executed directly"
    exit 1
fi

# Function to run performance tests
run_performance_tests() {
    print_header "Starting Performance Integration Tests"

    # Setup test environment
    setup_test_environment

    print_info "Running performance benchmarks..."

    # Test large file uploads/downloads
    test_large_file_performance

    # Test concurrent operations
    test_concurrent_performance

    # Test batch operations
    test_batch_performance

    print_success "All performance tests completed successfully"
    generate_performance_report
}

# Test large file performance
test_large_file_performance() {
    print_info "Testing large file performance"

    local large_file="$TEST_DATA_DIR/large_perf_test.bin"
    generate_test_file "$large_file" "$XLARGE_FILE_SIZE" "random"  # 10MB

    local s3_uri="s3://$CURRENT_TEST_BUCKET/large_perf_test.bin"
    local download_file="$TEST_DATA_DIR/downloaded_large_perf_test.bin"

    # Upload timing
    local upload_duration
    upload_duration=$(measure_time run_obsctl cp "$large_file" "$s3_uri")
    track_performance "large_file_upload" "$upload_duration" "$XLARGE_FILE_SIZE"

    # Download timing
    local download_duration
    download_duration=$(measure_time run_obsctl cp "$s3_uri" "$download_file")
    track_performance "large_file_download" "$download_duration" "$XLARGE_FILE_SIZE"

    # Verify integrity
    verify_file_integrity "$large_file" "$download_file" "large performance test file"

    print_success "Large file performance test completed"
}

# Test concurrent performance
test_concurrent_performance() {
    print_info "Testing concurrent operation performance"

    # Create multiple test files
    local test_files=()
    for i in {1..10}; do
        local file="$TEST_DATA_DIR/concurrent_perf_${i}.bin"
        generate_test_file "$file" "$MEDIUM_FILE_SIZE" "random"
        test_files+=("$file")
    done

    # Concurrent uploads
    local start_time end_time
    start_time=$(date +%s%N)

    local pids=()
    for i in "${!test_files[@]}"; do
        local file="${test_files[$i]}"
        (run_obsctl cp "$file" "s3://$CURRENT_TEST_BUCKET/concurrent_perf_${i}.bin") &
        pids+=($!)
    done

    # Wait for completion
    for pid in "${pids[@]}"; do
        wait "$pid"
    done

    end_time=$(date +%s%N)
    local concurrent_duration=$(( (end_time - start_time) / 1000000 ))
    track_performance "concurrent_uploads" "$concurrent_duration" $((MEDIUM_FILE_SIZE * 10))

    print_success "Concurrent performance test completed"
}

# Test batch operation performance
test_batch_performance() {
    print_info "Testing batch operation performance"

    # Create directory with many small files
    local batch_dir="$TEST_DATA_DIR/batch_perf"
    mkdir -p "$batch_dir"

    for i in {1..50}; do
        generate_test_file "$batch_dir/batch_${i}.txt" "1024" "text"
    done

    # Sync performance
    local sync_duration
    sync_duration=$(measure_time run_obsctl sync "$batch_dir/" "s3://$CURRENT_TEST_BUCKET/batch_perf/")
    track_performance "batch_sync" "$sync_duration" $((1024 * 50))

    print_success "Batch performance test completed"
}

print_verbose "Performance test module loaded successfully"
