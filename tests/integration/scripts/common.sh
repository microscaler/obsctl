#!/bin/bash
# shellcheck disable=SC2034,SC2120,SC2119  # Variables may be used in sourcing scripts, function args

# Common utilities for obsctl integration tests
# This script provides shared functions and utilities used across all test modules

# Ensure this script is being sourced, not executed
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    echo "ERROR: This script should be sourced, not executed directly"
    exit 1
fi

# Global test configuration (inherited from environment)
OBSCTL_BINARY="${OBSCTL_BINARY:-./target/debug/obsctl}"
TEST_BUCKET_PREFIX="${TEST_BUCKET_PREFIX:-obsctl-test}"
TEMP_DIR_PREFIX="${TEMP_DIR_PREFIX:-obsctl-temp}"

# Test file configurations (exported for use in test modules)
export SMALL_FILE_SIZE="1024"      # 1KB
export MEDIUM_FILE_SIZE="102400"   # 100KB
export LARGE_FILE_SIZE="1048576"   # 1MB
export XLARGE_FILE_SIZE="10485760" # 10MB

# Common test data
TEST_DATA_DIR=""
CURRENT_TEST_BUCKET=""
CURRENT_TEMP_DIR=""

# Performance tracking (using simple variables instead of associative arrays for compatibility)
PERFORMANCE_METRICS_FILE=""

# Function to run obsctl command with proper configuration
run_obsctl() {
    local cmd_args=("$@")

    print_verbose "Running obsctl: ${cmd_args[*]}"

    if [[ "$OBSCTL_DRY_RUN" == "true" ]]; then
        echo "[DRY RUN] Would execute: $OBSCTL_BINARY --endpoint $OBSCTL_ENDPOINT --region $OBSCTL_REGION --debug info ${cmd_args[*]}"
        return 0
    fi

    $OBSCTL_BINARY --endpoint "$OBSCTL_ENDPOINT" --region "$OBSCTL_REGION" --debug info "${cmd_args[@]}"
}

# Function to generate unique test bucket name
generate_test_bucket() {
    local suffix="${1:-$(date +%s)}"
    echo "${TEST_BUCKET_PREFIX}-${suffix}"
}

# Function to create temporary directory
create_temp_dir() {
    local temp_dir
    temp_dir=$(mktemp -d -t "${TEMP_DIR_PREFIX}-XXXXXX")
    echo "$temp_dir"
}

# Function to generate test file with specific size
generate_test_file() {
    local file_path="$1"
    local size_bytes="$2"
    local content_type="${3:-random}"

    case "$content_type" in
        random)
            dd if=/dev/urandom of="$file_path" bs=1 count="$size_bytes" 2>/dev/null
            ;;
        text)
            {
                echo "# obsctl Integration Test File"
                echo "Generated: $(date)"
                echo "Size: $size_bytes bytes"
                echo "Content: Structured text data"
                echo ""
                # Fill remaining space with lorem ipsum
                local remaining=$((size_bytes - 200))
                if [[ $remaining -gt 0 ]]; then
                    head -c "$remaining" /dev/urandom | base64 | head -c "$remaining"
                fi
            } > "$file_path"
            ;;
        zeros)
            dd if=/dev/zero of="$file_path" bs=1 count="$size_bytes" 2>/dev/null
            ;;
        *)
            echo "Unknown content type: $content_type" >&2
            return 1
            ;;
    esac
}

# Function to compute file checksum
compute_checksum() {
    local file_path="$1"
    local algorithm="${2:-md5}"

    case "$algorithm" in
        md5)
            if command -v md5sum >/dev/null 2>&1; then
                md5sum "$file_path" | cut -d' ' -f1
            elif command -v md5 >/dev/null 2>&1; then
                md5 -q "$file_path"
            else
                echo "ERROR: No MD5 command available" >&2
                return 1
            fi
            ;;
        sha256)
            if command -v sha256sum >/dev/null 2>&1; then
                sha256sum "$file_path" | cut -d' ' -f1
            elif command -v shasum >/dev/null 2>&1; then
                shasum -a 256 "$file_path" | cut -d' ' -f1
            else
                echo "ERROR: No SHA256 command available" >&2
                return 1
            fi
            ;;
        *)
            echo "ERROR: Unknown algorithm: $algorithm" >&2
            return 1
            ;;
    esac
}

# Function to verify file integrity
verify_file_integrity() {
    local original_file="$1"
    local downloaded_file="$2"
    local description="${3:-file}"

    print_verbose "Verifying integrity of $description"

    # Check if files exist
    if [[ ! -f "$original_file" ]]; then
        print_error "Original file not found: $original_file"
        return 1
    fi

    if [[ ! -f "$downloaded_file" ]]; then
        print_error "Downloaded file not found: $downloaded_file"
        return 1
    fi

    # Size comparison
    local original_size downloaded_size
    if command -v stat >/dev/null 2>&1; then
        original_size=$(stat -f%z "$original_file" 2>/dev/null || stat -c%s "$original_file")
        downloaded_size=$(stat -f%z "$downloaded_file" 2>/dev/null || stat -c%s "$downloaded_file")
    else
        original_size=$(wc -c < "$original_file")
        downloaded_size=$(wc -c < "$downloaded_file")
    fi

    if [[ "$original_size" != "$downloaded_size" ]]; then
        print_error "Size mismatch for $description: original=$original_size, downloaded=$downloaded_size"
        return 1
    fi

    # Checksum comparison
    local original_checksum downloaded_checksum
    original_checksum=$(compute_checksum "$original_file")
    downloaded_checksum=$(compute_checksum "$downloaded_file")

    if [[ "$original_checksum" != "$downloaded_checksum" ]]; then
        print_error "Checksum mismatch for $description: original=$original_checksum, downloaded=$downloaded_checksum"
        return 1
    fi

    print_verbose "File integrity verified for $description (size=$original_size, checksum=$original_checksum)"
    return 0
}

# Function to measure execution time
measure_time() {
    local start_time end_time duration
    start_time=$(date +%s%N)

    # Execute the command, suppressing output to avoid interference
    "$@" >/dev/null 2>&1
    local exit_code=$?

    end_time=$(date +%s%N)
    duration=$(( (end_time - start_time) / 1000000 )) # Convert to milliseconds

    echo "$duration"
    return $exit_code
}

# Function to track performance metrics
track_performance() {
    local operation="$1"
    local duration="$2"
    local size="${3:-0}"

    # Initialize metrics file if not set
    if [[ -z "$PERFORMANCE_METRICS_FILE" ]]; then
        PERFORMANCE_METRICS_FILE="$TEST_DATA_DIR/performance_metrics.txt"
    fi

    # Write metrics to file
    echo "${operation}_duration:$duration" >> "$PERFORMANCE_METRICS_FILE"
    echo "${operation}_size:$size" >> "$PERFORMANCE_METRICS_FILE"

    if [[ "$size" -gt 0 ]]; then
        local throughput=$((size * 1000 / duration)) # bytes per second
        echo "${operation}_throughput:$throughput" >> "$PERFORMANCE_METRICS_FILE"
    fi

    print_verbose "Performance: $operation took ${duration}ms (size: $size bytes)"
}

# Function to setup test environment
setup_test_environment() {
    print_verbose "Setting up test environment"

    # Create test bucket
    CURRENT_TEST_BUCKET=$(generate_test_bucket)
    print_verbose "Test bucket: $CURRENT_TEST_BUCKET"

    # Create temporary directory
    CURRENT_TEMP_DIR=$(create_temp_dir)
    export TEST_DATA_DIR="$CURRENT_TEMP_DIR"
    print_verbose "Temporary directory: $CURRENT_TEMP_DIR"

    # Create test bucket
    if [[ "$OBSCTL_DRY_RUN" != "true" ]]; then
        run_obsctl mb "s3://$CURRENT_TEST_BUCKET"
    fi

    print_verbose "Test environment setup completed"
}

# Function to cleanup test environment
cleanup_test_environment() {
    if [[ "$OBSCTL_CLEANUP" != "true" ]]; then
        print_warning "Cleanup disabled, leaving test resources"
        return 0
    fi

    print_verbose "Cleaning up test environment"

    # Remove all objects from test bucket
    if [[ -n "$CURRENT_TEST_BUCKET" && "$OBSCTL_DRY_RUN" != "true" ]]; then
        print_verbose "Removing objects from bucket: $CURRENT_TEST_BUCKET"
        run_obsctl rm --recursive --force "s3://$CURRENT_TEST_BUCKET/" || print_warning "Failed to remove objects"

        print_verbose "Removing bucket: $CURRENT_TEST_BUCKET"
        run_obsctl rb "s3://$CURRENT_TEST_BUCKET" || print_warning "Failed to remove bucket"
    fi

    # Remove temporary directory
    if [[ -n "$CURRENT_TEMP_DIR" && -d "$CURRENT_TEMP_DIR" ]]; then
        print_verbose "Removing temporary directory: $CURRENT_TEMP_DIR"
        rm -rf "$CURRENT_TEMP_DIR"
    fi

    print_verbose "Test environment cleanup completed"
}

# Function to generate performance report
generate_performance_report() {
    print_header "Performance Report"

    if [[ -z "$PERFORMANCE_METRICS_FILE" || ! -f "$PERFORMANCE_METRICS_FILE" ]]; then
        print_info "No performance metrics collected"
        return 0
    fi

    echo "Operation Performance Summary:"
    echo "=============================="

    # Group metrics by operation
    local operations=()
    if [[ -f "$PERFORMANCE_METRICS_FILE" ]]; then
        while IFS=':' read -r key value; do
            local operation="${key%_*}"
            if [[ ! " ${operations[*]} " =~ \ ${operation}\  ]]; then
                operations+=("$operation")
            fi
        done < "$PERFORMANCE_METRICS_FILE"
    fi

    # Display metrics for each operation
    for operation in "${operations[@]}"; do
        local duration="N/A"
        local size="0"
        local throughput="N/A"

        # Read metrics for this operation
        while IFS=':' read -r key value; do
            case "$key" in
                "${operation}_duration") duration="$value" ;;
                "${operation}_size") size="$value" ;;
                "${operation}_throughput") throughput="$value" ;;
            esac
        done < "$PERFORMANCE_METRICS_FILE"

        echo ""
        echo "Operation: $operation"
        echo "  Duration:   ${duration}ms"
        echo "  Size:       $(format_bytes "$size")"
        if [[ "$throughput" != "N/A" ]]; then
            echo "  Throughput: $(format_bytes "$throughput")/s"
        fi
    done

    echo ""
}

# Function to format bytes in human-readable format
format_bytes() {
    local bytes="$1"
    local units=("B" "KB" "MB" "GB" "TB")
    local unit_index=0
    local size="$bytes"

    while [[ $size -gt 1024 && $unit_index -lt $((${#units[@]} - 1)) ]]; do
        size=$((size / 1024))
        unit_index=$((unit_index + 1))
    done

    echo "${size}${units[$unit_index]}"
}

# Function to wait for condition with timeout
wait_for_condition() {
    local condition_cmd="$1"
    local timeout_seconds="${2:-30}"
    local check_interval="${3:-1}"

    local elapsed=0
    while [[ $elapsed -lt $timeout_seconds ]]; do
        if eval "$condition_cmd"; then
            return 0
        fi
        sleep "$check_interval"
        elapsed=$((elapsed + check_interval))
    done

    print_error "Timeout waiting for condition: $condition_cmd"
    return 1
}

# Function to retry command with backoff
retry_with_backoff() {
    local max_attempts="$1"
    local delay="$2"
    shift 2
    local cmd=("$@")

    local attempt=1
    while [[ $attempt -le $max_attempts ]]; do
        if "${cmd[@]}"; then
            return 0
        fi

        if [[ $attempt -lt $max_attempts ]]; then
            print_warning "Command failed (attempt $attempt/$max_attempts), retrying in ${delay}s..."
            sleep "$delay"
            delay=$((delay * 2)) # Exponential backoff
        fi

        attempt=$((attempt + 1))
    done

    print_error "Command failed after $max_attempts attempts: ${cmd[*]}"
    return 1
}

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to validate S3 URI format
validate_s3_uri() {
    local uri="$1"
    if [[ ! "$uri" =~ ^s3://[a-zA-Z0-9][a-zA-Z0-9._-]*[a-zA-Z0-9](/.*)?$ ]]; then
        print_error "Invalid S3 URI format: $uri"
        return 1
    fi
    return 0
}

# Function to extract bucket name from S3 URI
extract_bucket_name() {
    local uri="$1"
    echo "$uri" | sed 's|s3://||' | cut -d'/' -f1
}

# Function to extract key from S3 URI
extract_s3_key() {
    local uri="$1"
    local path_part
    local path_part="${uri#s3://*/}"
    echo "${path_part#/}"
}

# Trap function for cleanup on exit
cleanup_on_exit() {
    local exit_code=$?
    print_verbose "Cleaning up on exit (code: $exit_code)"
    cleanup_test_environment
    exit $exit_code
}

# Set trap for cleanup
trap cleanup_on_exit EXIT INT TERM

print_verbose "Common utilities loaded successfully"
