#!/bin/bash

# Redfire Switch Libraries Installation Script
# Copyright (C) 2025 Carrier One Inc and contributors
# 
# This script installs only the Redfire codec engine and SIP stack libraries
# without the main application binaries.

set -e

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
CODEC_LIB_NAME="redfire-codec-engine"
SIP_LIB_NAME="redfire-sip-stack" 
SIP_MINIMAL_LIB_NAME="redfire-sip-stack-minimal"
DEFAULT_PREFIX="/usr/local"
PREFIX="${PREFIX:-$DEFAULT_PREFIX}"
BUILD_TYPE="${BUILD_TYPE:-release}"

# Function to print colored output
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to check system dependencies
check_dependencies() {
    print_info "Checking system dependencies..."
    
    if ! command_exists cargo; then
        print_error "Rust/Cargo is required but not installed."
        print_info "Please install Rust from https://rustup.rs/"
        exit 1
    fi
    
    if ! command_exists pkg-config; then
        print_warning "pkg-config not found. Some features may not work correctly."
    fi
    
    print_success "Dependencies check completed"
}

# Function to build codec engine library
build_codec_engine() {
    print_info "Building Redfire Codec Engine library..."
    
    cd "$CODEC_LIB_NAME"
    
    # Determine features to enable
    local features=""
    if command_exists nvcc; then
        print_info "NVIDIA CUDA detected, enabling CUDA support"
        features="cuda"
    elif command_exists hipcc; then
        print_info "AMD ROCm detected, enabling ROCm support"
        features="rocm"
    else
        print_info "No GPU acceleration detected, building CPU-only version"
    fi
    
    # Build the library
    if [ "$BUILD_TYPE" = "release" ]; then
        if [ -n "$features" ]; then
            cargo build --release --features "$features"
        else
            cargo build --release
        fi
    else
        if [ -n "$features" ]; then
            cargo build --features "$features"
        else
            cargo build
        fi
    fi
    
    cd ..
    print_success "Codec engine library built successfully"
}

# Function to build SIP stack library
build_sip_stack() {
    print_info "Building Redfire SIP Stack library..."
    
    cd "$SIP_LIB_NAME"
    
    # Build the library
    if [ "$BUILD_TYPE" = "release" ]; then
        cargo build --release
    else
        cargo build
    fi
    
    cd ..
    print_success "SIP stack library built successfully"
}

# Function to build minimal SIP stack library
build_sip_stack_minimal() {
    print_info "Building Redfire SIP Stack Minimal library..."
    
    cd "$SIP_MINIMAL_LIB_NAME"
    
    # Build the library
    if [ "$BUILD_TYPE" = "release" ]; then
        cargo build --release
    else
        cargo build
    fi
    
    cd ..
    print_success "Minimal SIP stack library built successfully"
}

# Function to install libraries
install_libraries() {
    print_info "Installing libraries to $PREFIX..."
    
    # Create directories
    sudo mkdir -p "$PREFIX/lib"
    sudo mkdir -p "$PREFIX/include/redfire"
    sudo mkdir -p "$PREFIX/lib/pkgconfig"
    
    # Determine target directory based on build type
    local target_dir="target"
    if [ "$BUILD_TYPE" = "release" ]; then
        target_dir="target/release"
    else
        target_dir="target/debug"
    fi
    
    # Install codec engine library
    if [ -f "$CODEC_LIB_NAME/$target_dir/lib$CODEC_LIB_NAME.rlib" ]; then
        sudo cp "$CODEC_LIB_NAME/$target_dir/lib$CODEC_LIB_NAME.rlib" "$PREFIX/lib/"
        print_info "Installed codec engine library (Rust static)"
    fi
    
    if [ -f "$CODEC_LIB_NAME/$target_dir/lib$CODEC_LIB_NAME.so" ]; then
        sudo cp "$CODEC_LIB_NAME/$target_dir/lib$CODEC_LIB_NAME.so" "$PREFIX/lib/"
        sudo ldconfig
        print_info "Installed codec engine library (shared)"
    fi
    
    # Install SIP stack library
    if [ -f "$SIP_LIB_NAME/$target_dir/lib$SIP_LIB_NAME.rlib" ]; then
        sudo cp "$SIP_LIB_NAME/$target_dir/lib$SIP_LIB_NAME.rlib" "$PREFIX/lib/"
        print_info "Installed SIP stack library (Rust static)"
    fi
    
    if [ -f "$SIP_LIB_NAME/$target_dir/lib$SIP_LIB_NAME.so" ]; then
        sudo cp "$SIP_LIB_NAME/$target_dir/lib$SIP_LIB_NAME.so" "$PREFIX/lib/"
        sudo ldconfig
        print_info "Installed SIP stack library (shared)"
    fi
    
    # Install minimal SIP stack library
    if [ -f "$SIP_MINIMAL_LIB_NAME/$target_dir/lib$SIP_MINIMAL_LIB_NAME.rlib" ]; then
        sudo cp "$SIP_MINIMAL_LIB_NAME/$target_dir/lib$SIP_MINIMAL_LIB_NAME.rlib" "$PREFIX/lib/"
        print_info "Installed minimal SIP stack library (Rust static)"
    fi
    
    if [ -f "$SIP_MINIMAL_LIB_NAME/$target_dir/lib$SIP_MINIMAL_LIB_NAME.so" ]; then
        sudo cp "$SIP_MINIMAL_LIB_NAME/$target_dir/lib$SIP_MINIMAL_LIB_NAME.so" "$PREFIX/lib/"
        sudo ldconfig
        print_info "Installed minimal SIP stack library (shared)"
    fi
    
    # Create pkg-config files
    create_pkgconfig_files
    
    print_success "Libraries installed successfully to $PREFIX"
}

# Function to create pkg-config files
create_pkgconfig_files() {
    print_info "Creating pkg-config files..."
    
    # Codec engine pkg-config
    cat > /tmp/redfire-codec-engine.pc << EOF
prefix=$PREFIX
exec_prefix=\${prefix}
libdir=\${exec_prefix}/lib
includedir=\${prefix}/include

Name: Redfire Codec Engine
Description: Professional audio codec translation engine with GPU acceleration
Version: 0.1.0
Libs: -L\${libdir} -lredfire_codec_engine
Cflags: -I\${includedir}
EOF
    
    sudo mv /tmp/redfire-codec-engine.pc "$PREFIX/lib/pkgconfig/"
    
    # SIP stack pkg-config
    cat > /tmp/redfire-sip-stack.pc << EOF
prefix=$PREFIX
exec_prefix=\${prefix}
libdir=\${exec_prefix}/lib
includedir=\${prefix}/include

Name: Redfire SIP Stack
Description: Complete SIP, SIP-I, and SIP-T protocol stack implementation
Version: 0.1.0
Libs: -L\${libdir} -lredfire_sip_stack
Cflags: -I\${includedir}
EOF
    
    sudo mv /tmp/redfire-sip-stack.pc "$PREFIX/lib/pkgconfig/"
    
    # Minimal SIP stack pkg-config
    cat > /tmp/redfire-sip-stack-minimal.pc << EOF
prefix=$PREFIX
exec_prefix=\${prefix}
libdir=\${exec_prefix}/lib
includedir=\${prefix}/include

Name: Redfire SIP Stack Minimal
Description: Lightweight SIP implementation with minimal dependencies
Version: 0.1.0
Libs: -L\${libdir} -lredfire_sip_stack_minimal
Cflags: -I\${includedir}
EOF
    
    sudo mv /tmp/redfire-sip-stack-minimal.pc "$PREFIX/lib/pkgconfig/"
    
    print_success "pkg-config files created"
}

# Function to run tests
run_tests() {
    if [ "$RUN_TESTS" = "true" ]; then
        print_info "Running codec engine tests..."
        cd "$CODEC_LIB_NAME"
        cargo test
        cd ..
        
        print_info "Running SIP stack tests..."
        cd "$SIP_LIB_NAME"
        cargo test
        cd ..
        
        print_success "All tests passed"
    fi
}

# Function to display usage information
show_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Install Redfire Switch libraries (codec engine and SIP stack)"
    echo ""
    echo "Options:"
    echo "  --prefix PATH     Installation prefix (default: /usr/local)"
    echo "  --debug           Build in debug mode (default: release)"
    echo "  --test            Run tests before installation"
    echo "  --help, -h        Show this help message"
    echo ""
    echo "Environment variables:"
    echo "  PREFIX            Installation prefix"
    echo "  BUILD_TYPE        Build type (release or debug)"
    echo "  RUN_TESTS         Run tests (true or false)"
    echo ""
    echo "Examples:"
    echo "  $0                                    # Install to /usr/local"
    echo "  $0 --prefix /opt/redfire             # Install to /opt/redfire"
    echo "  $0 --debug --test                    # Debug build with tests"
    echo "  PREFIX=/home/user/.local $0          # Install to user directory"
}

# Function to check if running as root when needed
check_permissions() {
    if [ "$PREFIX" = "/usr/local" ] || [ "$PREFIX" = "/usr" ]; then
        if [ "$EUID" -ne 0 ] && ! sudo -n true 2>/dev/null; then
            print_error "Installation to $PREFIX requires sudo privileges."
            print_info "Please run with sudo or use --prefix to install to a user directory."
            exit 1
        fi
    fi
}

# Function to verify installation
verify_installation() {
    print_info "Verifying installation..."
    
    local missing_files=0
    
    if [ ! -f "$PREFIX/lib/pkgconfig/redfire-codec-engine.pc" ]; then
        print_warning "Codec engine pkg-config file not found"
        ((missing_files++))
    fi
    
    if [ ! -f "$PREFIX/lib/pkgconfig/redfire-sip-stack.pc" ]; then
        print_warning "SIP stack pkg-config file not found"
        ((missing_files++))
    fi
    
    if [ ! -f "$PREFIX/lib/pkgconfig/redfire-sip-stack-minimal.pc" ]; then
        print_warning "Minimal SIP stack pkg-config file not found"
        ((missing_files++))
    fi
    
    if [ $missing_files -eq 0 ]; then
        print_success "Installation verification completed successfully"
        print_info "Libraries can now be used by other projects"
        print_info "Use 'pkg-config --libs redfire-codec-engine' to get linking flags"
        print_info "Use 'pkg-config --libs redfire-sip-stack' to get linking flags"
    else
        print_warning "Installation verification found $missing_files missing files"
        print_info "Installation may be incomplete"
    fi
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --prefix)
            PREFIX="$2"
            shift 2
            ;;
        --debug)
            BUILD_TYPE="debug"
            shift
            ;;
        --test)
            RUN_TESTS="true"
            shift
            ;;
        --help|-h)
            show_usage
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            show_usage
            exit 1
            ;;
    esac
done

# Main installation process
main() {
    echo "Redfire Switch Libraries Installation"
    echo "======================================"
    echo ""
    
    print_info "Installation configuration:"
    print_info "  Prefix: $PREFIX"
    print_info "  Build type: $BUILD_TYPE"
    print_info "  Run tests: ${RUN_TESTS:-false}"
    echo ""
    
    # Check if we're in the right directory
    if [ ! -d "$CODEC_LIB_NAME" ] || [ ! -d "$SIP_LIB_NAME" ]; then
        print_error "Library directories not found. Please run this script from the redfire-switch root directory."
        exit 1
    fi
    
    check_permissions
    check_dependencies
    
    build_codec_engine
    build_sip_stack
    build_sip_stack_minimal
    
    run_tests
    
    install_libraries
    verify_installation
    
    echo ""
    print_success "Redfire Switch libraries installation completed!"
    print_info "Libraries installed to: $PREFIX"
    print_info "Add $PREFIX/lib/pkgconfig to PKG_CONFIG_PATH to use pkg-config"
    
    if [ "$PREFIX" != "/usr/local" ] && [ "$PREFIX" != "/usr" ]; then
        print_info "You may need to add $PREFIX/lib to your LD_LIBRARY_PATH"
    fi
}

# Run main function
main "$@"