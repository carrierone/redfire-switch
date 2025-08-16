#!/bin/bash

# Debian Package Builder for Redfire Switch
# This script builds a .deb package from the compiled binary

set -e

# Configuration
PACKAGE_NAME="redfire-switch"
VERSION="0.1.0"
ARCHITECTURE="amd64"
BUILD_DIR="$(pwd)/debian-build"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if binary exists
check_binary() {
    if [ ! -f "$CARGO_TARGET_DIR/release/redfire-switch" ]; then
        log_error "Binary not found at $CARGO_TARGET_DIR/release/redfire-switch"
        log_info "Build the project first with: cargo build --release --features bgp-anycast"
        exit 1
    fi
    log_info "Found binary: $CARGO_TARGET_DIR/release/redfire-switch"
}

# Create package directory structure
create_package_structure() {
    log_info "Creating package directory structure..."
    
    rm -rf "$BUILD_DIR"
    mkdir -p "$BUILD_DIR"
    
    # Create directory structure
    mkdir -p "$BUILD_DIR/DEBIAN"
    mkdir -p "$BUILD_DIR/usr/bin"
    mkdir -p "$BUILD_DIR/usr/share/redfire-switch"
    mkdir -p "$BUILD_DIR/usr/share/doc/redfire-switch"
    mkdir -p "$BUILD_DIR/usr/share/man/man1"
    mkdir -p "$BUILD_DIR/etc/systemd/system"
    mkdir -p "$BUILD_DIR/usr/lib/tmpfiles.d"
    mkdir -p "$BUILD_DIR/etc/logrotate.d"
    mkdir -p "$BUILD_DIR/usr/share/bash-completion/completions"
    mkdir -p "$BUILD_DIR/usr/share/zsh/site-functions"
    mkdir -p "$BUILD_DIR/usr/share/fish/vendor_completions.d"
    
    log_success "Package structure created"
}

# Copy files to package
copy_files() {
    log_info "Copying files to package..."
    
    # Copy binary
    cp "$CARGO_TARGET_DIR/release/redfire-switch" "$BUILD_DIR/usr/bin/"
    chmod 755 "$BUILD_DIR/usr/bin/redfire-switch"
    
    # Copy systemd service files
    cp systemd/*.service "$BUILD_DIR/etc/systemd/system/"
    
    # Copy tmpfiles configuration
    cp systemd/redfire-switch.tmpfiles "$BUILD_DIR/usr/lib/tmpfiles.d/redfire-switch.conf"
    
    # Copy configuration templates
    cp config-template.toml "$BUILD_DIR/usr/share/redfire-switch/"
    cp bgp-anycast-template.toml "$BUILD_DIR/usr/share/redfire-switch/"
    
    # Copy documentation
    cp README.md "$BUILD_DIR/usr/share/doc/redfire-switch/" || true
    cp INSTALLATION.md "$BUILD_DIR/usr/share/doc/redfire-switch/" || true
    cp LICENSE "$BUILD_DIR/usr/share/doc/redfire-switch/" || true
    
    # Copy examples if they exist
    if [ -d examples ]; then
        cp -r examples "$BUILD_DIR/usr/share/doc/redfire-switch/"
    fi
    
    # Copy database schema if it exists
    if [ -f schema.sql ]; then
        cp schema.sql "$BUILD_DIR/usr/share/redfire-switch/"
    fi
    
    # Copy scripts
    mkdir -p "$BUILD_DIR/usr/share/redfire-switch/scripts"
    cp scripts/post-install.sh "$BUILD_DIR/usr/share/redfire-switch/scripts/"
    chmod +x "$BUILD_DIR/usr/share/redfire-switch/scripts/post-install.sh"
    
    # Create logrotate configuration
    cat > "$BUILD_DIR/etc/logrotate.d/redfire-switch" << 'EOF'
/var/log/redfire-switch/*.log {
    daily
    rotate 30
    compress
    delaycompress
    missingok
    notifempty
    create 644 redfire redfire
    postrotate
        systemctl reload redfire-switch.service
    endscript
}
EOF
    
    # Generate shell completions (if the binary supports it)
    if "$CARGO_TARGET_DIR/release/redfire-switch" completion bash >/dev/null 2>&1; then
        "$CARGO_TARGET_DIR/release/redfire-switch" completion bash > "$BUILD_DIR/usr/share/bash-completion/completions/redfire-switch"
        "$CARGO_TARGET_DIR/release/redfire-switch" completion zsh > "$BUILD_DIR/usr/share/zsh/site-functions/_redfire-switch"
        "$CARGO_TARGET_DIR/release/redfire-switch" completion fish > "$BUILD_DIR/usr/share/fish/vendor_completions.d/redfire-switch.fish"
    fi
    
    log_success "Files copied to package"
}

# Copy Debian control files
copy_control_files() {
    log_info "Copying Debian control files..."
    
    cp debian/DEBIAN/* "$BUILD_DIR/DEBIAN/"
    
    # Make scripts executable
    chmod +x "$BUILD_DIR/DEBIAN/preinst"
    chmod +x "$BUILD_DIR/DEBIAN/postinst"
    chmod +x "$BUILD_DIR/DEBIAN/prerm"
    chmod +x "$BUILD_DIR/DEBIAN/postrm"
    
    log_success "Control files copied"
}

# Calculate installed size
calculate_size() {
    log_info "Calculating package size..."
    
    SIZE=$(du -sk "$BUILD_DIR" | cut -f1)
    sed -i "s/Installed-Size: [0-9]*/Installed-Size: $SIZE/" "$BUILD_DIR/DEBIAN/control"
    
    log_info "Package size: ${SIZE}KB"
}

# Build the package
build_package() {
    log_info "Building Debian package..."
    
    PACKAGE_FILE="${PACKAGE_NAME}_${VERSION}-1_${ARCHITECTURE}.deb"
    
    # Build the package
    fakeroot dpkg-deb --build "$BUILD_DIR" "$PACKAGE_FILE"
    
    if [ -f "$PACKAGE_FILE" ]; then
        log_success "Package built: $PACKAGE_FILE"
        
        # Show package info
        log_info "Package information:"
        dpkg -I "$PACKAGE_FILE"
        
        # Show package contents
        log_info "Package contents:"
        dpkg -c "$PACKAGE_FILE"
        
        # Lint the package
        if command -v lintian >/dev/null 2>&1; then
            log_info "Running lintian checks..."
            lintian "$PACKAGE_FILE" || log_warning "Lintian found some issues (this is normal for development packages)"
        fi
        
        log_success "Package ready for installation with: sudo dpkg -i $PACKAGE_FILE"
    else
        log_error "Failed to build package"
        exit 1
    fi
}

# Test package installation (optional)
test_package() {
    if [ "$1" = "--test" ]; then
        log_info "Testing package installation..."
        
        # Create a test environment (requires Docker or similar)
        if command -v docker >/dev/null 2>&1; then
            log_info "Testing with Docker..."
            
            # Create test Dockerfile
            cat > Dockerfile.test << EOF
FROM debian:bullseye
RUN apt-get update && apt-get install -y systemd postgresql redis-server
COPY $PACKAGE_FILE /tmp/
RUN dpkg -i /tmp/$PACKAGE_FILE || apt-get install -f -y
CMD ["/usr/bin/redfire-switch", "--version"]
EOF
            
            # Build and test
            docker build -f Dockerfile.test -t redfire-switch-test .
            docker run --rm redfire-switch-test
            
            # Cleanup
            rm Dockerfile.test
            
            log_success "Package test completed"
        else
            log_warning "Docker not available for testing"
        fi
    fi
}

# Main function
main() {
    echo "=== Redfire Switch Debian Package Builder ==="
    echo ""
    
    # Check dependencies
    if ! command -v dpkg-deb >/dev/null 2>&1; then
        log_error "dpkg-deb not found. Install with: apt-get install dpkg-dev"
        exit 1
    fi
    
    if ! command -v fakeroot >/dev/null 2>&1; then
        log_error "fakeroot not found. Install with: apt-get install fakeroot"
        exit 1
    fi
    
    check_binary
    create_package_structure
    copy_files
    copy_control_files
    calculate_size
    build_package
    test_package "$@"
    
    # Cleanup
    log_info "Cleaning up build directory..."
    rm -rf "$BUILD_DIR"
    
    log_success "Build complete!"
}

# Handle command line arguments
case "${1:-}" in
    --help|-h)
        echo "Redfire Switch Debian Package Builder"
        echo ""
        echo "Usage: $0 [OPTIONS]"
        echo ""
        echo "Options:"
        echo "  --help, -h      Show this help message"
        echo "  --test          Test package installation with Docker"
        echo "  --clean         Clean build artifacts and exit"
        echo ""
        echo "Environment variables:"
        echo "  CARGO_TARGET_DIR    Cargo target directory (default: target)"
        echo ""
        echo "Prerequisites:"
        echo "  - Rust binary must be built with: cargo build --release --features bgp-anycast"
        echo "  - dpkg-dev and fakeroot packages must be installed"
        echo ""
        exit 0
        ;;
    --clean)
        log_info "Cleaning build artifacts..."
        rm -rf "$BUILD_DIR"
        rm -f "${PACKAGE_NAME}_${VERSION}-1_${ARCHITECTURE}.deb"
        log_success "Clean complete"
        exit 0
        ;;
    *)
        main "$@"
        ;;
esac