#!/bin/bash

# shellcheck disable=SC2034  # Variables may be used in sourcing scripts
# Basic Integration Tests for obsctl
# Simple tests for core functionality

# Ensure this script is being sourced, not executed
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    echo "ERROR: This script should be sourced, not executed directly"
    exit 1
fi

# Function to run basic tests
run_basic_tests() {
    print_header "Starting Basic Integration Tests"

    # Setup test environment
    setup_test_environment

    # Test data
    local test_file="$TEST_DATA_DIR/basic_test.txt"
    local download_file="$TEST_DATA_DIR/downloaded_basic_test.txt"

    print_info "Generating test file..."
    generate_test_file "$test_file" "1024" "text"
    local test_checksum
    test_checksum=$(compute_checksum "$test_file")

    # Test 1: Basic upload
    print_info "Test 1: Basic File Upload"
    local s3_uri="s3://$CURRENT_TEST_BUCKET/basic_test.txt"
    local duration
    duration=$(measure_time run_obsctl cp "$test_file" "$s3_uri")
    track_performance "basic_upload" "$duration" "1024"
    print_success "File uploaded successfully"

    # Test 2: List objects
    print_info "Test 2: List Objects"
    duration=$(measure_time run_obsctl ls "s3://$CURRENT_TEST_BUCKET")
    track_performance "basic_list" "$duration"
    print_success "Objects listed successfully"

    # Test 3: Object metadata
    print_info "Test 3: Object Metadata"
    local bucket_name key_name
    bucket_name=$(extract_bucket_name "$s3_uri")
    key_name=$(extract_s3_key "$s3_uri")
    duration=$(measure_time run_obsctl head-object --bucket "$bucket_name" --key "$key_name")
    track_performance "basic_head_object" "$duration"
    print_success "Object metadata retrieved successfully"

    # Test 4: Basic download
    print_info "Test 4: Basic File Download"
    duration=$(measure_time run_obsctl cp "$s3_uri" "$download_file")
    track_performance "basic_download" "$duration" "1024"
    print_success "File downloaded successfully"

    # Test 5: Verify integrity
    print_info "Test 5: File Integrity Verification"
    if verify_file_integrity "$test_file" "$download_file" "basic test file"; then
        print_success "File integrity verified successfully"
    else
        print_error "File integrity verification failed"
        return 1
    fi

    # Test 6: Basic deletion
    print_info "Test 6: Basic File Deletion"
    duration=$(measure_time run_obsctl rm "$s3_uri")
    track_performance "basic_delete" "$duration"
    print_success "File deleted successfully"

    # Test 7: Verify deletion
    print_info "Test 7: Verify Deletion"
    local list_output
    list_output=$(run_obsctl ls "s3://$CURRENT_TEST_BUCKET" 2>/dev/null || true)
    if [[ "$list_output" == *"basic_test.txt"* ]]; then
        print_error "File was not properly deleted"
        return 1
    else
        print_success "File deletion verified"
    fi

    print_success "All basic tests completed successfully"

    # Generate performance report
    generate_performance_report
}

print_verbose "Basic test module loaded successfully"
