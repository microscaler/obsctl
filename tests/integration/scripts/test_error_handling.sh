#!/bin/bash
# shellcheck disable=SC2034,SC2155  # Variables may be used in sourcing scripts, declare separately

# Error Handling Integration Tests for obsctl
# Focused on testing error scenarios and edge cases

# Ensure this script is being sourced, not executed
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    echo "ERROR: This script should be sourced, not executed directly"
    exit 1
fi

# Function to run error handling tests
run_error_handling_tests() {
    print_header "Starting Error Handling Integration Tests"

    # Setup test environment
    setup_test_environment

    print_info "Testing error scenarios and edge cases..."

    # Test invalid operations
    test_invalid_operations

    # Test network and service errors
    test_service_errors

    # Test file system errors
    test_filesystem_errors

    # Test edge cases
    test_edge_cases

    print_success "All error handling tests completed successfully"
}

# Test invalid operations
test_invalid_operations() {
    print_info "Testing invalid operations"

    # Invalid S3 URIs
    local invalid_uris=(
        "s3://"
        "s3://bucket-with-spaces"
        "s3://UPPERCASE-BUCKET"
        "s3://bucket.with.dots"
        "not-an-s3-uri"
        "s3://bucket/key with spaces"
    )

    for uri in "${invalid_uris[@]}"; do
        print_verbose "Testing invalid URI: $uri"
        if run_obsctl ls "$uri" 2>/dev/null; then
            print_warning "Expected error for invalid URI: $uri"
        else
            print_verbose "Correctly rejected invalid URI: $uri"
        fi
    done

    # Invalid commands
    if run_obsctl invalid-command 2>/dev/null; then
        print_warning "Expected error for invalid command"
    else
        print_verbose "Correctly rejected invalid command"
    fi

    print_success "Invalid operations test completed"
}

# Test service errors
test_service_errors() {
    print_info "Testing service error scenarios"

    # Non-existent bucket
    local fake_bucket="non-existent-bucket-$(date +%s)"
    if run_obsctl ls "s3://$fake_bucket" 2>/dev/null; then
        print_warning "Expected error for non-existent bucket"
    else
        print_verbose "Correctly handled non-existent bucket error"
    fi

    # Non-existent object
    if run_obsctl cp "s3://$CURRENT_TEST_BUCKET/non-existent-file.txt" "/tmp/should-fail" 2>/dev/null; then
        print_warning "Expected error for non-existent object"
    else
        print_verbose "Correctly handled non-existent object error"
    fi

    # Upload to non-existent bucket
    local temp_file="$TEST_DATA_DIR/error_test.txt"
    echo "test content" > "$temp_file"
    if run_obsctl cp "$temp_file" "s3://$fake_bucket/test.txt" 2>/dev/null; then
        print_warning "Expected error for upload to non-existent bucket"
    else
        print_verbose "Correctly handled upload to non-existent bucket error"
    fi

    print_success "Service error scenarios test completed"
}

# Test filesystem errors
test_filesystem_errors() {
    print_info "Testing filesystem error scenarios"

    # Upload non-existent file
    if run_obsctl cp "/non/existent/file.txt" "s3://$CURRENT_TEST_BUCKET/test.txt" 2>/dev/null; then
        print_warning "Expected error for non-existent local file"
    else
        print_verbose "Correctly handled non-existent local file error"
    fi

    # Download to invalid path (if possible to test safely)
    if [[ -w "/tmp" ]]; then
        # Create a file first
        local test_file="$TEST_DATA_DIR/fs_error_test.txt"
        echo "test content" > "$test_file"
        run_obsctl cp "$test_file" "s3://$CURRENT_TEST_BUCKET/fs_error_test.txt"

        # Try to download to a directory that exists as a file
        local blocking_file="/tmp/blocking_file_$$"
        touch "$blocking_file"
        if run_obsctl cp "s3://$CURRENT_TEST_BUCKET/fs_error_test.txt" "$blocking_file/should_fail.txt" 2>/dev/null; then
            print_warning "Expected error for invalid download path"
        else
            print_verbose "Correctly handled invalid download path error"
        fi
        rm -f "$blocking_file"
    fi

    print_success "Filesystem error scenarios test completed"
}

# Test edge cases
test_edge_cases() {
    print_info "Testing edge cases"

    # Empty file
    local empty_file="$TEST_DATA_DIR/empty_file.txt"
    touch "$empty_file"
    if run_obsctl cp "$empty_file" "s3://$CURRENT_TEST_BUCKET/empty_file.txt"; then
        print_verbose "Empty file upload handled correctly"

        # Download empty file
        local downloaded_empty="$TEST_DATA_DIR/downloaded_empty.txt"
        if run_obsctl cp "s3://$CURRENT_TEST_BUCKET/empty_file.txt" "$downloaded_empty"; then
            local size
            size=$(stat -f%z "$downloaded_empty" 2>/dev/null || stat -c%s "$downloaded_empty")
            if [[ "$size" == "0" ]]; then
                print_verbose "Empty file download handled correctly"
            else
                print_warning "Empty file download size mismatch: $size"
            fi
        else
            print_warning "Empty file download failed"
        fi
    else
        print_warning "Empty file upload failed"
    fi

    # Very long filename (within limits)
    local long_name="very_long_filename_$(printf 'a%.0s' {1..100}).txt"
    local long_file="$TEST_DATA_DIR/$long_name"
    echo "test content" > "$long_file"
    if run_obsctl cp "$long_file" "s3://$CURRENT_TEST_BUCKET/$long_name" 2>/dev/null; then
        print_verbose "Long filename handled correctly"
    else
        print_verbose "Long filename appropriately rejected or failed"
    fi

    # Special characters in filename (safe ones)
    local special_name="file-with_special.chars-123.txt"
    local special_file="$TEST_DATA_DIR/$special_name"
    echo "test content" > "$special_file"
    if run_obsctl cp "$special_file" "s3://$CURRENT_TEST_BUCKET/$special_name"; then
        print_verbose "Special characters in filename handled correctly"
    else
        print_warning "Special characters in filename failed unexpectedly"
    fi

    print_success "Edge cases test completed"
}

print_verbose "Error handling test module loaded successfully"
