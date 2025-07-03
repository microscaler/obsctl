#!/bin/bash
# shellcheck disable=SC2034  # Variables may be used in sourcing scripts

# obsctl Integration Test Runner
# Main entrypoint for all integration testing with argument parsing and modular design

set -euo pipefail

# Script directory for sourcing modules
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPTS_DIR="$SCRIPT_DIR/scripts"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Default configuration
DEFAULT_ENDPOINT="http://localhost:9000"
DEFAULT_REGION="us-east-1"
DEFAULT_TEST_TYPE="comprehensive"
DEFAULT_OTEL_ENABLED="true"
DEFAULT_CLEANUP="true"
DEFAULT_VERBOSE="false"

# Configuration variables
ENDPOINT="$DEFAULT_ENDPOINT"
REGION="$DEFAULT_REGION"
TEST_TYPE="$DEFAULT_TEST_TYPE"
OTEL_ENABLED="$DEFAULT_OTEL_ENABLED"
CLEANUP="$DEFAULT_CLEANUP"
VERBOSE="$DEFAULT_VERBOSE"
DRY_RUN="false"

# Function to print colored output
print_header() {
    echo -e "${CYAN}[HEADER]${NC} $1"
}

print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_verbose() {
    if [[ "$VERBOSE" == "true" ]]; then
        echo -e "${CYAN}[VERBOSE]${NC} $1"
    fi
}

# Function to show usage
show_usage() {
    cat << EOF
obsctl Integration Test Runner

USAGE:
    $0 [OPTIONS] [TEST_TYPE]

TEST TYPES:
    comprehensive    Run comprehensive integration tests (default)
    basic           Run basic integration tests
    performance     Run performance-focused tests
    observability   Run observability-focused tests
    concurrent      Run concurrent operation tests
    error-handling  Run error scenario tests
    all             Run all test types sequentially

OPTIONS:
    -e, --endpoint URL      MinIO endpoint URL (default: $DEFAULT_ENDPOINT)
    -r, --region REGION     AWS region (default: $DEFAULT_REGION)
    -o, --otel BOOL         Enable OpenTelemetry (default: $DEFAULT_OTEL_ENABLED)
    -c, --cleanup BOOL      Cleanup after tests (default: $DEFAULT_CLEANUP)
    -v, --verbose           Enable verbose output
    -n, --dry-run           Show what would be executed without running
    -h, --help              Show this help message

EXAMPLES:
    $0                                          # Run comprehensive tests with defaults
    $0 basic --verbose                          # Run basic tests with verbose output
    $0 performance --endpoint http://localhost:9000
    $0 all --no-cleanup --otel false           # Run all tests, no cleanup, no OTEL
    $0 --dry-run comprehensive                  # Show what comprehensive tests would do

ENVIRONMENT:
    The following environment variables can be set:
    - AWS_ACCESS_KEY_ID (default: minioadmin)
    - AWS_SECRET_ACCESS_KEY (default: minioadmin123)
    - AWS_DEFAULT_REGION (overrides --region)
    - OTEL_EXPORTER_OTLP_ENDPOINT (overrides OTEL endpoint)

OBSERVABILITY:
    When OTEL is enabled, check these dashboards after tests:
    - Grafana: http://localhost:3000 (admin/admin)
    - Jaeger: http://localhost:16686
    - Prometheus: http://localhost:9090
    - MinIO Console: http://localhost:9001 (minioadmin/minioadmin123)

EOF
}

# Function to parse arguments
parse_arguments() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            -e|--endpoint)
                ENDPOINT="$2"
                shift 2
                ;;
            -r|--region)
                REGION="$2"
                shift 2
                ;;
            -o|--otel)
                OTEL_ENABLED="$2"
                shift 2
                ;;
            --no-otel)
                OTEL_ENABLED="false"
                shift
                ;;
            -c|--cleanup)
                CLEANUP="$2"
                shift 2
                ;;
            --no-cleanup)
                CLEANUP="false"
                shift
                ;;
            -v|--verbose)
                VERBOSE="true"
                shift
                ;;
            -n|--dry-run)
                DRY_RUN="true"
                shift
                ;;
            -h|--help)
                show_usage
                exit 0
                ;;
            comprehensive|basic|performance|observability|concurrent|error-handling|all)
                TEST_TYPE="$1"
                shift
                ;;
            *)
                print_error "Unknown option: $1"
                show_usage
                exit 1
                ;;
        esac
    done
}

# Function to validate configuration
validate_configuration() {
    print_verbose "Validating configuration..."

    # Check if obsctl binary exists
    if [[ ! -f "./target/debug/obsctl" ]]; then
        print_error "obsctl binary not found at ./target/debug/obsctl"
        print_error "Please run: cargo build --features otel"
        exit 1
    fi

    # Check if MinIO is accessible (unless dry run)
    if [[ "$DRY_RUN" != "true" ]]; then
        if ! curl -s "$ENDPOINT" >/dev/null; then
            print_error "MinIO not accessible at $ENDPOINT"
            print_error "Please ensure Docker Compose stack is running: docker compose up -d"
            exit 1
        fi
        print_verbose "MinIO endpoint $ENDPOINT is accessible"
    fi

    # Validate boolean values
    case "$OTEL_ENABLED" in
        true|false) ;;
        *) print_error "Invalid OTEL value: $OTEL_ENABLED (must be true or false)"; exit 1 ;;
    esac

    case "$CLEANUP" in
        true|false) ;;
        *) print_error "Invalid cleanup value: $CLEANUP (must be true or false)"; exit 1 ;;
    esac

    print_verbose "Configuration validation passed"
}

# Function to setup environment
setup_environment() {
    print_verbose "Setting up test environment..."

    # Set default AWS credentials if not provided
    export AWS_ACCESS_KEY_ID="${AWS_ACCESS_KEY_ID:-minioadmin}"
    export AWS_SECRET_ACCESS_KEY="${AWS_SECRET_ACCESS_KEY:-minioadmin123}"
    export AWS_DEFAULT_REGION="${AWS_DEFAULT_REGION:-$REGION}"

    # Setup OTEL environment if enabled
    if [[ "$OTEL_ENABLED" == "true" ]]; then
        export OTEL_ENABLED="true"
        export OTEL_EXPORTER_OTLP_ENDPOINT="${OTEL_EXPORTER_OTLP_ENDPOINT:-http://localhost:4317}"
        export OTEL_SERVICE_NAME="obsctl-integration-test"
        export OTEL_SERVICE_VERSION="0.1.0"
        print_verbose "OpenTelemetry enabled with endpoint: $OTEL_EXPORTER_OTLP_ENDPOINT"
    else
        export OTEL_ENABLED="false"
        print_verbose "OpenTelemetry disabled"
    fi

    # Export configuration for scripts
    export OBSCTL_ENDPOINT="$ENDPOINT"
    export OBSCTL_REGION="$REGION"
    export OBSCTL_CLEANUP="$CLEANUP"
    export OBSCTL_VERBOSE="$VERBOSE"
    export OBSCTL_DRY_RUN="$DRY_RUN"

    print_verbose "Environment setup completed"
}

# Function to source required scripts
source_scripts() {
    print_verbose "Sourcing test scripts..."

    local required_scripts=(
        "common.sh"
        "test_basic.sh"
        "test_comprehensive.sh"
        "test_performance.sh"
        "test_observability.sh"
        "test_concurrent.sh"
        "test_error_handling.sh"
    )

    for script in "${required_scripts[@]}"; do
        local script_path="$SCRIPTS_DIR/$script"
        if [[ -f "$script_path" ]]; then
            print_verbose "Sourcing $script"
            # shellcheck source=/dev/null
            source "$script_path"
        else
            print_warning "Script not found: $script_path"
        fi
    done

    print_verbose "Script sourcing completed"
}

# Function to run specific test type
run_test_type() {
    local test_type="$1"

    print_header "Running $test_type tests"

    case "$test_type" in
        comprehensive)
            if declare -f run_comprehensive_tests >/dev/null; then
                run_comprehensive_tests
            else
                print_error "Comprehensive test function not found"
                exit 1
            fi
            ;;
        basic)
            if declare -f run_basic_tests >/dev/null; then
                run_basic_tests
            else
                print_error "Basic test function not found"
                exit 1
            fi
            ;;
        performance)
            if declare -f run_performance_tests >/dev/null; then
                run_performance_tests
            else
                print_error "Performance test function not found"
                exit 1
            fi
            ;;
        observability)
            if declare -f run_observability_tests >/dev/null; then
                run_observability_tests
            else
                print_error "Observability test function not found"
                exit 1
            fi
            ;;
        concurrent)
            if declare -f run_concurrent_tests >/dev/null; then
                run_concurrent_tests
            else
                print_error "Concurrent test function not found"
                exit 1
            fi
            ;;
        error-handling)
            if declare -f run_error_handling_tests >/dev/null; then
                run_error_handling_tests
            else
                print_error "Error handling test function not found"
                exit 1
            fi
            ;;
        all)
            local test_types=("basic" "comprehensive" "performance" "observability" "concurrent" "error-handling")
            for type in "${test_types[@]}"; do
                print_header "Running $type tests (part of 'all' suite)"
                run_test_type "$type"
                print_success "$type tests completed"
                echo ""
            done
            return
            ;;
        *)
            print_error "Unknown test type: $test_type"
            exit 1
            ;;
    esac

    print_success "$test_type tests completed successfully"
}

# Function to show configuration
show_configuration() {
    print_header "obsctl Integration Test Configuration"
    echo "Test Type:     $TEST_TYPE"
    echo "Endpoint:      $ENDPOINT"
    echo "Region:        $REGION"
    echo "OTEL Enabled:  $OTEL_ENABLED"
    echo "Cleanup:       $CLEANUP"
    echo "Verbose:       $VERBOSE"
    echo "Dry Run:       $DRY_RUN"
    echo ""

    if [[ "$OTEL_ENABLED" == "true" ]]; then
        print_info "Observability dashboards will be available at:"
        echo "  • Grafana:    http://localhost:3000 (admin/admin)"
        echo "  • Jaeger:     http://localhost:16686"
        echo "  • Prometheus: http://localhost:9090"
        echo "  • MinIO:      http://localhost:9001 (minioadmin/minioadmin123)"
        echo ""
    fi
}

# Function to generate final report
generate_final_report() {
    print_header "Integration Test Report"
    echo "Date:          $(date)"
    echo "Test Type:     $TEST_TYPE"
    echo "Endpoint:      $ENDPOINT"
    echo "Region:        $REGION"
    echo "OTEL Enabled:  $OTEL_ENABLED"
    echo ""

    if [[ "$OTEL_ENABLED" == "true" ]]; then
        print_success "Telemetry data has been sent to the observability stack"
        print_info "Check the dashboards for detailed metrics and traces"
    fi

    print_success "All integration tests completed successfully!"
}

# Main execution function
main() {
    # Parse command line arguments
    parse_arguments "$@"

    # Show configuration
    show_configuration

    # If dry run, show what would be executed
    if [[ "$DRY_RUN" == "true" ]]; then
        print_warning "DRY RUN MODE - No actual tests will be executed"
        print_info "Would run: $TEST_TYPE tests"
        print_info "Would use endpoint: $ENDPOINT"
        print_info "Would use region: $REGION"
        print_info "OTEL would be: $OTEL_ENABLED"
        print_info "Cleanup would be: $CLEANUP"
        exit 0
    fi

    # Validate configuration
    validate_configuration

    # Setup environment
    setup_environment

    # Source required scripts
    source_scripts

    # Run the specified test type
    run_test_type "$TEST_TYPE"

    # Generate final report
    generate_final_report
}

# Check if script is being sourced or executed
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
