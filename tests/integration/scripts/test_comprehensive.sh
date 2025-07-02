#!/bin/bash

# shellcheck disable=SC2034  # Variables may be used in sourcing scripts
# Comprehensive Integration Tests for obsctl
# Tests all major functionality and generates observability data

# Ensure this script is being sourced, not executed
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    echo "ERROR: This script should be sourced, not executed directly"
    exit 1
fi

# Function to run comprehensive tests
run_comprehensive_tests() {
    print_header "Starting Comprehensive Integration Tests"

    # Setup test environment
    setup_test_environment

    # Test data files
    local small_file="$TEST_DATA_DIR/small_test.txt"
    local medium_file="$TEST_DATA_DIR/medium_test.bin"
    local large_file="$TEST_DATA_DIR/large_test.bin"
    local text_file="$TEST_DATA_DIR/structured_test.txt"

    # Download directory
    local download_dir="$TEST_DATA_DIR/downloads"
    mkdir -p "$download_dir"

    print_info "Generating test files..."

    # Generate test files with different characteristics
    generate_test_file "$small_file" "$SMALL_FILE_SIZE" "text"
    generate_test_file "$medium_file" "$MEDIUM_FILE_SIZE" "random"
    generate_test_file "$large_file" "$LARGE_FILE_SIZE" "random"
    generate_test_file "$text_file" "2048" "text"

    # Compute checksums for verification
    local small_checksum medium_checksum large_checksum text_checksum
    small_checksum=$(compute_checksum "$small_file")
    medium_checksum=$(compute_checksum "$medium_file")
    large_checksum=$(compute_checksum "$large_file")
    text_checksum=$(compute_checksum "$text_file")

    print_success "Test files generated successfully"

    # Test 1: Basic bucket operations
    print_info "Test 1: Bucket Operations"
    test_bucket_operations

    # Test 2: Single file upload/download
    print_info "Test 2: Single File Operations"
    test_single_file_upload "$small_file" "small_test.txt" "$small_checksum" "$download_dir"
    test_single_file_upload "$medium_file" "medium_test.bin" "$medium_checksum" "$download_dir"
    test_single_file_upload "$large_file" "large_test.bin" "$large_checksum" "$download_dir"

    # Test 3: Directory synchronization
    print_info "Test 3: Directory Synchronization"
    test_directory_sync

    # Test 4: Object listing and metadata
    print_info "Test 4: Object Listing and Metadata"
    test_object_listing_and_metadata

    # Test 5: Presigned URLs
    print_info "Test 5: Presigned URL Generation"
    test_presigned_urls

    # Test 6: Object deletion
    print_info "Test 6: Object Deletion"
    test_object_deletion

    # Test 7: Error scenarios
    print_info "Test 7: Error Handling"
    test_error_scenarios

    print_success "All comprehensive tests completed successfully"

    # Generate performance report
    generate_performance_report
}

# Test bucket operations
test_bucket_operations() {
    print_verbose "Testing bucket creation and listing"

    # List buckets (should include our test bucket)
    local duration
    duration=$(measure_time run_obsctl ls)
    track_performance "list_buckets" "$duration"

    # Test bucket creation with different names
    local test_bucket_2="$CURRENT_TEST_BUCKET-secondary"
    duration=$(measure_time run_obsctl mb "s3://$test_bucket_2")
    track_performance "create_bucket" "$duration"

    # List buckets again
    run_obsctl ls

    # Remove secondary bucket
    run_obsctl rb "s3://$test_bucket_2"

    print_verbose "Bucket operations test completed"
}

# Test single file upload and download
test_single_file_upload() {
    local local_file="$1"
    local s3_key="$2"
    local expected_checksum="$3"
    local download_dir="$4"

    local s3_uri="s3://$CURRENT_TEST_BUCKET/$s3_key"
    local downloaded_file="$download_dir/$s3_key"

    print_verbose "Testing upload/download of $s3_key"

    # Upload file
    local file_size
    file_size=$(stat -f%z "$local_file" 2>/dev/null || stat -c%s "$local_file")
    local duration
    duration=$(measure_time run_obsctl cp "$local_file" "$s3_uri")
    track_performance "upload_${s3_key}" "$duration" "$file_size"

    # Verify upload with head-object
    local bucket_name
    local key_name
    bucket_name=$(extract_bucket_name "$s3_uri")
    key_name=$(extract_s3_key "$s3_uri")
    run_obsctl head-object --bucket "$bucket_name" --key "$key_name"

    # Download file
    duration=$(measure_time run_obsctl cp "$s3_uri" "$downloaded_file")
    track_performance "download_${s3_key}" "$duration" "$file_size"

    # Verify file integrity
    if ! verify_file_integrity "$local_file" "$downloaded_file" "$s3_key"; then
        print_error "File integrity check failed for $s3_key"
        return 1
    fi

    print_success "Single file test completed for $s3_key"
}

# Test directory synchronization
test_directory_sync() {
    print_verbose "Testing directory synchronization"

    # Create a directory structure
    local sync_source="$TEST_DATA_DIR/sync_source"
    local sync_dest="$TEST_DATA_DIR/sync_dest"
    mkdir -p "$sync_source/subdir1" "$sync_source/subdir2"

    # Create files in the directory structure
    generate_test_file "$sync_source/file1.txt" "512" "text"
    generate_test_file "$sync_source/file2.bin" "1024" "random"
    generate_test_file "$sync_source/subdir1/nested1.txt" "256" "text"
    generate_test_file "$sync_source/subdir2/nested2.bin" "768" "random"

    # Sync directory to S3
    local s3_prefix="s3://$CURRENT_TEST_BUCKET/sync_test/"
    local duration
    duration=$(measure_time run_obsctl sync "$sync_source/" "$s3_prefix")
    track_performance "sync_to_s3" "$duration"

    # List objects to verify sync
    run_obsctl ls "$s3_prefix" --recursive

    # Sync back from S3 to local
    mkdir -p "$sync_dest"
    duration=$(measure_time run_obsctl sync "$s3_prefix" "$sync_dest/")
    track_performance "sync_from_s3" "$duration"

    # Verify directory structure
    if [[ ! -f "$sync_dest/file1.txt" ]] || [[ ! -f "$sync_dest/subdir1/nested1.txt" ]]; then
        print_error "Directory sync verification failed"
        return 1
    fi

    print_success "Directory synchronization test completed"
}

# Test object listing and metadata operations
test_object_listing_and_metadata() {
    print_verbose "Testing object listing and metadata operations"

    # List all objects in bucket
    local duration
    duration=$(measure_time run_obsctl ls "s3://$CURRENT_TEST_BUCKET" --recursive)
    track_performance "list_objects_recursive" "$duration"

    # List objects with prefix
    run_obsctl ls "s3://$CURRENT_TEST_BUCKET/sync_test/"

    # Test head-object on various files
    local test_objects=(
        "small_test.txt"
        "medium_test.bin"
        "large_test.bin"
        "sync_test/file1.txt"
    )

    for obj in "${test_objects[@]}"; do
        local s3_uri="s3://$CURRENT_TEST_BUCKET/$obj"
        local bucket_name key_name
        bucket_name=$(extract_bucket_name "$s3_uri")
        key_name=$(extract_s3_key "$s3_uri")
        duration=$(measure_time run_obsctl head-object --bucket "$bucket_name" --key "$key_name")
        track_performance "head_object_${obj//\//_}" "$duration"
    done

    # Test du (disk usage) command
    duration=$(measure_time run_obsctl du "s3://$CURRENT_TEST_BUCKET")
    track_performance "disk_usage" "$duration"

    print_success "Object listing and metadata test completed"
}

# Test presigned URL generation
test_presigned_urls() {
    print_verbose "Testing presigned URL generation"

    local test_object="s3://$CURRENT_TEST_BUCKET/small_test.txt"

    # Generate presigned URLs for different methods
    local methods=("GET" "PUT" "DELETE")
    local durations=("3600" "1800" "7200")

    for i in "${!methods[@]}"; do
        local method="${methods[$i]}"
        local duration_sec="${durations[$i]}"

        local cmd_duration
        cmd_duration=$(measure_time run_obsctl presign "$test_object" --method "$method" --expires-in "$duration_sec")
        track_performance "presign_${method}" "$cmd_duration"
    done

    print_success "Presigned URL test completed"
}

# Test object deletion operations
test_object_deletion() {
    print_verbose "Testing object deletion operations"

    # Create some test objects for deletion
    local delete_test_dir="$TEST_DATA_DIR/delete_test"
    mkdir -p "$delete_test_dir"

    # Create test files
    generate_test_file "$delete_test_dir/delete1.txt" "100" "text"
    generate_test_file "$delete_test_dir/delete2.txt" "200" "text"
    generate_test_file "$delete_test_dir/delete3.txt" "300" "text"

    # Upload files
    run_obsctl cp "$delete_test_dir/delete1.txt" "s3://$CURRENT_TEST_BUCKET/delete_test/delete1.txt"
    run_obsctl cp "$delete_test_dir/delete2.txt" "s3://$CURRENT_TEST_BUCKET/delete_test/delete2.txt"
    run_obsctl cp "$delete_test_dir/delete3.txt" "s3://$CURRENT_TEST_BUCKET/delete_test/delete3.txt"

    # Test single object deletion
    local duration
    duration=$(measure_time run_obsctl rm "s3://$CURRENT_TEST_BUCKET/delete_test/delete1.txt")
    track_performance "delete_single_object" "$duration"

    # Test recursive deletion
    duration=$(measure_time run_obsctl rm "s3://$CURRENT_TEST_BUCKET/delete_test/" --recursive)
    track_performance "delete_recursive" "$duration"

    # Verify deletion
    local list_output
    list_output=$(run_obsctl ls "s3://$CURRENT_TEST_BUCKET/delete_test/" 2>/dev/null || true)
    if [[ -n "$list_output" ]]; then
        print_warning "Some objects may not have been deleted"
    fi

    print_success "Object deletion test completed"
}

# Test error scenarios and edge cases
test_error_scenarios() {
    print_verbose "Testing error scenarios and edge cases"

    # Test operations on non-existent bucket
    if run_obsctl ls "s3://non-existent-bucket-$(date +%s)" 2>/dev/null; then
        print_warning "Expected error for non-existent bucket, but command succeeded"
    else
        print_verbose "Non-existent bucket error handled correctly"
    fi

    # Test download of non-existent object
    if run_obsctl cp "s3://$CURRENT_TEST_BUCKET/non-existent-file.txt" "/tmp/should-not-exist" 2>/dev/null; then
        print_warning "Expected error for non-existent object, but command succeeded"
    else
        print_verbose "Non-existent object error handled correctly"
    fi

    # Test upload to non-existent bucket
    local temp_file="$TEST_DATA_DIR/temp_error_test.txt"
    echo "test content" > "$temp_file"
    if run_obsctl cp "$temp_file" "s3://non-existent-bucket-$(date +%s)/test.txt" 2>/dev/null; then
        print_warning "Expected error for upload to non-existent bucket, but command succeeded"
    else
        print_verbose "Upload to non-existent bucket error handled correctly"
    fi

    # Test invalid S3 URI formats
    local invalid_uris=(
        "s3://"
        "s3://bucket with spaces/key"
        "s3://BUCKET/key"
        "not-an-s3-uri"
    )

    for uri in "${invalid_uris[@]}"; do
        if validate_s3_uri "$uri"; then
            print_warning "URI validation should have failed for: $uri"
        else
            print_verbose "Invalid URI correctly rejected: $uri"
        fi
    done

    print_success "Error scenario test completed"
}

print_verbose "Comprehensive test module loaded successfully"
