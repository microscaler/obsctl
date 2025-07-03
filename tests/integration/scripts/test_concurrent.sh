#!/bin/bash
# shellcheck disable=SC2034,SC2155  # Variables may be used in sourcing scripts, declare separately

# Concurrent Operations Integration Tests for obsctl
# Focused on testing concurrent operations and race conditions

# Ensure this script is being sourced, not executed
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    echo "ERROR: This script should be sourced, not executed directly"
    exit 1
fi

# Function to run concurrent tests
run_concurrent_tests() {
    print_header "Starting Concurrent Operations Integration Tests"

    # Setup test environment
    setup_test_environment

    print_info "Testing concurrent operations..."

    # Test concurrent uploads
    test_concurrent_uploads

    # Test concurrent downloads
    test_concurrent_downloads

    # Test mixed concurrent operations
    test_mixed_concurrent_operations

    print_success "All concurrent tests completed successfully"
    generate_performance_report
}

# Test concurrent uploads
test_concurrent_uploads() {
    print_info "Testing concurrent uploads"

    local test_files=()
    for i in {1..8}; do
        local file="$TEST_DATA_DIR/concurrent_upload_${i}.bin"
        generate_test_file "$file" "$MEDIUM_FILE_SIZE" "random"
        test_files+=("$file")
    done

    local pids=()
    local start_time
    start_time=$(date +%s%N)

    for i in "${!test_files[@]}"; do
        local file="${test_files[$i]}"
        (run_obsctl cp "$file" "s3://$CURRENT_TEST_BUCKET/concurrent_upload_${i}.bin") &
        pids+=($!)
    done

    for pid in "${pids[@]}"; do
        wait "$pid"
    done

    local end_time duration
    end_time=$(date +%s%N)
    duration=$(( (end_time - start_time) / 1000000 ))
    track_performance "concurrent_uploads_8files" "$duration" $((MEDIUM_FILE_SIZE * 8))

    print_success "Concurrent uploads test completed"
}

# Test concurrent downloads
test_concurrent_downloads() {
    print_info "Testing concurrent downloads"

    local pids=()
    local start_time
    start_time=$(date +%s%N)

    for i in {0..7}; do
        local download_file="$TEST_DATA_DIR/downloaded_concurrent_${i}.bin"
        (run_obsctl cp "s3://$CURRENT_TEST_BUCKET/concurrent_upload_${i}.bin" "$download_file") &
        pids+=($!)
    done

    for pid in "${pids[@]}"; do
        wait "$pid"
    done

    local end_time duration
    end_time=$(date +%s%N)
    duration=$(( (end_time - start_time) / 1000000 ))
    track_performance "concurrent_downloads_8files" "$duration" $((MEDIUM_FILE_SIZE * 8))

    # Verify some files
    for i in {0..2}; do
        local original="$TEST_DATA_DIR/concurrent_upload_${i}.bin"
        local downloaded="$TEST_DATA_DIR/downloaded_concurrent_${i}.bin"
        verify_file_integrity "$original" "$downloaded" "concurrent file $i"
    done

    print_success "Concurrent downloads test completed"
}

# Test mixed concurrent operations
test_mixed_concurrent_operations() {
    print_info "Testing mixed concurrent operations"

    local pids=()

    # Mix of uploads, downloads, and list operations
    for i in {1..3}; do
        local file="$TEST_DATA_DIR/mixed_${i}.txt"
        generate_test_file "$file" "2048" "text"
        (run_obsctl cp "$file" "s3://$CURRENT_TEST_BUCKET/mixed_${i}.txt") &
        pids+=($!)
    done

    # Concurrent list operations
    for i in {1..2}; do
        (run_obsctl ls "s3://$CURRENT_TEST_BUCKET" >/dev/null) &
        pids+=($!)
    done

    # Concurrent head-object operations
    for i in {0..1}; do
        (run_obsctl head-object "s3://$CURRENT_TEST_BUCKET/concurrent_upload_${i}.bin" >/dev/null) &
        pids+=($!)
    done

    # Wait for all operations
    for pid in "${pids[@]}"; do
        wait "$pid"
    done

    print_success "Mixed concurrent operations test completed"
}

print_verbose "Concurrent test module loaded successfully"
