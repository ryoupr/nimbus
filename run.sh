#!/bin/bash

# Nimbus v3.0 (Rust) - Unix Launcher Script

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_NAME="nimbus"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging function
log() {
    echo -e "${BLUE}[$(date +'%Y-%m-%d %H:%M:%S')]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

# Check if Rust is installed
check_rust() {
    if ! command -v cargo &> /dev/null; then
        error "Rust/Cargo is not installed. Please install Rust from https://rustup.rs/"
        exit 1
    fi
    
    local rust_version=$(rustc --version | cut -d' ' -f2)
    log "Using Rust version: $rust_version"
}

# Check if AWS CLI is installed
check_aws_cli() {
    if ! command -v aws &> /dev/null; then
        warn "AWS CLI is not installed. Some features may not work properly."
        warn "Install AWS CLI from: https://aws.amazon.com/cli/"
    else
        local aws_version=$(aws --version 2>&1 | cut -d' ' -f1)
        log "Using $aws_version"
    fi
}

# Check if Session Manager Plugin is installed
check_session_manager_plugin() {
    if ! command -v session-manager-plugin &> /dev/null; then
        warn "AWS Session Manager Plugin is not installed."
        warn "Install from: https://docs.aws.amazon.com/systems-manager/latest/userguide/session-manager-working-with-install-plugin.html"
    else
        log "Session Manager Plugin is available"
    fi
}

# Build the project
build_project() {
    log "Building $PROJECT_NAME..."
    
    cd "$SCRIPT_DIR"
    
    if [ "$1" = "--release" ]; then
        log "Building in release mode..."
        cargo build --release
        success "Release build completed"
    else
        log "Building in debug mode..."
        cargo build
        success "Debug build completed"
    fi
}

# Run the project
run_project() {
    log "Running $PROJECT_NAME..."
    
    cd "$SCRIPT_DIR"
    
    if [ "$1" = "--release" ]; then
        cargo run --release -- "${@:2}"
    else
        cargo run -- "$@"
    fi
}

# Install the project
install_project() {
    log "Installing $PROJECT_NAME..."
    
    cd "$SCRIPT_DIR"
    cargo install --path .
    
    success "$PROJECT_NAME installed successfully"
    log "You can now run 'nimbus' from anywhere"
}

# Run tests
run_tests() {
    log "Running tests for $PROJECT_NAME..."
    
    cd "$SCRIPT_DIR"
    
    # Run unit tests
    log "Running unit tests..."
    cargo test
    
    # Run property-based tests if available
    if grep -q "proptest" Cargo.toml; then
        log "Running property-based tests..."
        cargo test --features proptest
    fi
    
    success "All tests completed"
}

# Clean build artifacts
clean_project() {
    log "Cleaning build artifacts..."
    
    cd "$SCRIPT_DIR"
    cargo clean
    
    success "Clean completed"
}

# Show help
show_help() {
    echo "Nimbus v3.0 (Rust) - Unix Launcher"
    echo ""
    echo "Usage: $0 [COMMAND] [OPTIONS]"
    echo ""
    echo "Commands:"
    echo "  build [--release]     Build the project"
    echo "  run [--release] [ARGS] Run the project with optional arguments"
    echo "  install              Install the binary system-wide"
    echo "  test                 Run all tests"
    echo "  clean                Clean build artifacts"
    echo "  check                Check dependencies"
    echo "  help                 Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 build --release"
    echo "  $0 run connect --instance-id i-1234567890abcdef0"
    echo "  $0 run tui"
    echo "  $0 install"
    echo ""
}

# Check dependencies
check_dependencies() {
    log "Checking dependencies..."
    
    check_rust
    check_aws_cli
    check_session_manager_plugin
    
    success "Dependency check completed"
}

# Main execution
main() {
    case "${1:-}" in
        "build")
            check_rust
            build_project "$2"
            ;;
        "run")
            check_rust
            run_project "${@:2}"
            ;;
        "install")
            check_rust
            install_project
            ;;
        "test")
            check_rust
            run_tests
            ;;
        "clean")
            clean_project
            ;;
        "check")
            check_dependencies
            ;;
        "help"|"--help"|"-h")
            show_help
            ;;
        "")
            log "Starting $PROJECT_NAME with default settings..."
            check_rust
            run_project
            ;;
        *)
            error "Unknown command: $1"
            echo ""
            show_help
            exit 1
            ;;
    esac
}

# Execute main function with all arguments
main "$@"