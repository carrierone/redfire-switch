#!/bin/bash

# Redfire Switch Debian/Ubuntu Installer Script
# Copyright (C) 2025 Carrier One Inc
# 
# This script installs Redfire Switch on Debian/Ubuntu systems

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
PACKAGE_NAME="redfire-switch"
VERSION="0.1.0"
RELEASE_URL="https://github.com/carrierone/redfire-switch/releases"
DEFAULT_REPO="https://packages.carrierone.com/debian"

# Functions
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

check_root() {
    if [ "$EUID" -ne 0 ]; then
        log_error "This script must be run as root (use sudo)"
        exit 1
    fi
}

detect_os() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        OS=$ID
        VERSION_ID=${VERSION_ID}
        VERSION_CODENAME=${VERSION_CODENAME:-}
    else
        log_error "Cannot detect operating system"
        exit 1
    fi

    case $OS in
        debian|ubuntu)
            log_info "Detected: $PRETTY_NAME"
            ;;
        *)
            log_error "Unsupported operating system: $OS"
            log_error "This installer supports Debian and Ubuntu only"
            exit 1
            ;;
    esac
}

check_architecture() {
    ARCH=$(dpkg --print-architecture)
    case $ARCH in
        amd64|arm64)
            log_info "Architecture: $ARCH"
            ;;
        *)
            log_error "Unsupported architecture: $ARCH"
            log_error "Supported architectures: amd64, arm64"
            exit 1
            ;;
    esac
}

install_dependencies() {
    log_info "Installing system dependencies..."
    
    apt-get update
    
    # Essential dependencies
    apt-get install -y \
        curl \
        wget \
        gnupg \
        ca-certificates \
        lsb-release \
        systemd \
        adduser \
        openssl \
        postgresql-client

    # Recommended packages
    log_info "Installing recommended packages..."
    apt-get install -y \
        redis-server \
        postgresql \
        nginx \
        fail2ban \
        ufw \
        htop \
        iotop \
        tcpdump \
        wireshark-common

    # Optional BGP packages
    if command -v exabgp >/dev/null 2>&1; then
        log_info "ExaBGP is already installed"
    else
        log_warning "ExaBGP not found. Installing for BGP Anycast support..."
        apt-get install -y exabgp || log_warning "ExaBGP installation failed. BGP features may not work."
    fi

    if command -v bird >/dev/null 2>&1; then
        log_info "BIRD is already installed"  
    else
        log_warning "BIRD not found. Installing for BGP Anycast support..."
        apt-get install -y bird2 || log_warning "BIRD installation failed. BGP features may not work."
    fi
}

setup_repository() {
    log_info "Setting up Carrier One package repository..."
    
    # Add GPG key
    curl -fsSL https://packages.carrierone.com/gpg | gpg --dearmor > /usr/share/keyrings/carrierone.gpg
    
    # Add repository
    echo "deb [signed-by=/usr/share/keyrings/carrierone.gpg] $DEFAULT_REPO $VERSION_CODENAME main" > /etc/apt/sources.list.d/carrierone.list
    
    # Update package list
    apt-get update
}

install_from_repository() {
    log_info "Installing Redfire Switch from repository..."
    
    if apt-get install -y redfire-switch; then
        log_success "Redfire Switch installed successfully from repository"
        return 0
    else
        log_warning "Repository installation failed. Falling back to direct installation."
        return 1
    fi
}

install_from_package() {
    log_info "Downloading and installing Redfire Switch package..."
    
    # Create temporary directory
    TEMP_DIR=$(mktemp -d)
    cd "$TEMP_DIR"
    
    # Download package
    PACKAGE_URL="${RELEASE_URL}/download/v${VERSION}/redfire-switch_${VERSION}-1_${ARCH}.deb"
    log_info "Downloading from: $PACKAGE_URL"
    
    if wget -q "$PACKAGE_URL"; then
        log_success "Package downloaded successfully"
    else
        log_error "Failed to download package from $PACKAGE_URL"
        log_info "You can manually download and install the package using:"
        log_info "  wget $PACKAGE_URL"
        log_info "  sudo dpkg -i redfire-switch_${VERSION}-1_${ARCH}.deb"
        log_info "  sudo apt-get install -f"
        exit 1
    fi
    
    # Install package
    if dpkg -i "redfire-switch_${VERSION}-1_${ARCH}.deb"; then
        log_success "Package installed successfully"
    else
        log_info "Fixing dependencies..."
        apt-get install -f -y
    fi
    
    # Clean up
    cd /
    rm -rf "$TEMP_DIR"
}

build_from_source() {
    log_info "Building Redfire Switch from source..."
    
    # Install Rust
    if ! command -v cargo >/dev/null 2>&1; then
        log_info "Installing Rust toolchain..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source ~/.cargo/env
    fi
    
    # Install build dependencies
    apt-get install -y \
        build-essential \
        pkg-config \
        libssl-dev \
        libpq-dev \
        libclang-dev \
        cmake \
        git
    
    # Clone and build
    TEMP_DIR=$(mktemp -d)
    cd "$TEMP_DIR"
    
    git clone https://github.com/carrierone/redfire-switch.git
    cd redfire-switch
    
    log_info "Building Redfire Switch (this may take several minutes)..."
    cargo build --release --features "bgp-anycast,redis-cluster"
    
    # Install binary
    install -m 755 target/release/redfire-switch /usr/bin/redfire-switch
    
    # Install systemd files
    install -m 644 systemd/*.service /etc/systemd/system/
    
    # Create directories and users (simplified version of package scripts)
    addgroup --system redfire || true
    adduser --system --no-create-home --home /var/lib/redfire-switch \
            --shell /usr/sbin/nologin --group redfire redfire || true
    adduser --system --no-create-home --home /var/lib/redfire-switch/web \
            --shell /usr/sbin/nologin --ingroup redfire redfire-web || true
    
    mkdir -p /var/lib/redfire-switch /var/log/redfire-switch /etc/redfire-switch /run/redfire-switch
    chown redfire:redfire /var/lib/redfire-switch /var/log/redfire-switch /run/redfire-switch
    chown root:redfire /etc/redfire-switch
    chmod 750 /var/lib/redfire-switch /var/log/redfire-switch /etc/redfire-switch
    
    # Install configuration templates
    if [ -f config-template.toml ]; then
        install -m 640 config-template.toml /etc/redfire-switch/config.toml
        chown root:redfire /etc/redfire-switch/config.toml
    fi
    
    # Enable services
    systemctl daemon-reload
    systemctl enable redfire-switch.service
    
    log_success "Redfire Switch built and installed from source"
    
    # Clean up
    cd /
    rm -rf "$TEMP_DIR"
}

configure_firewall() {
    log_info "Configuring firewall..."
    
    if command -v ufw >/dev/null 2>&1; then
        # Allow SIP ports
        ufw allow 5060/udp comment "SIP signaling"
        ufw allow 5060/tcp comment "SIP signaling"
        ufw allow 5061/tcp comment "SIP TLS"
        ufw allow 5061/udp comment "SIP TLS"
        
        # Allow RTP port range (configurable)
        ufw allow 10000:20000/udp comment "RTP media"
        
        # Allow web interface (optional)
        # ufw allow 8080/tcp comment "Redfire Switch web interface"
        
        log_success "Firewall rules configured"
        log_info "Enable firewall with: ufw enable"
    else
        log_warning "UFW not found. Please configure firewall manually:"
        log_info "  SIP signaling: 5060/tcp, 5060/udp, 5061/tcp, 5061/udp"
        log_info "  RTP media: 10000-20000/udp (configurable)"
        log_info "  Web interface: 8080/tcp (optional)"
    fi
}

setup_database() {
    log_info "Setting up PostgreSQL database..."
    
    if systemctl is-active --quiet postgresql; then
        log_info "PostgreSQL is running"
        
        # Create database and user
        sudo -u postgres psql -c "CREATE DATABASE redfire_switch;" 2>/dev/null || log_info "Database already exists"
        sudo -u postgres psql -c "CREATE USER redfire WITH PASSWORD 'redfire_password';" 2>/dev/null || log_info "User already exists"
        sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE redfire_switch TO redfire;" 2>/dev/null || true
        
        log_success "Database setup complete"
        log_warning "Default password is 'redfire_password' - please change it!"
        log_info "Update /etc/redfire-switch/config.toml with database connection details"
    else
        log_warning "PostgreSQL is not running. Start it with: systemctl start postgresql"
    fi
}

post_install_info() {
    cat << EOF

${GREEN}=== Redfire Switch Installation Complete ===${NC}

${BLUE}Next Steps:${NC}
1. Edit configuration: /etc/redfire-switch/config.toml
2. Configure database connection and initialize schema
3. Set up SIP trunks and routing rules
4. Start the service: systemctl start redfire-switch

${BLUE}Service Management:${NC}
  Status:         systemctl status redfire-switch
  Start:          systemctl start redfire-switch  
  Stop:           systemctl stop redfire-switch
  Restart:        systemctl restart redfire-switch
  Logs:           journalctl -u redfire-switch -f

${BLUE}Configuration Files:${NC}
  Main config:    /etc/redfire-switch/config.toml
  BGP Anycast:    /etc/redfire-switch/bgp-anycast.toml

${BLUE}Web Interface:${NC}
  Enable:         touch /etc/redfire-switch/web-enabled
  Start:          systemctl enable --now redfire-switch-web
  URL:            http://localhost:8080

${BLUE}Security:${NC}
  - Configure firewall rules for your network
  - Change default database password
  - Review security settings in configuration
  - Consider enabling fail2ban for additional protection

${BLUE}Documentation:${NC}
  Manual:         man redfire-switch
  Examples:       /usr/share/doc/redfire-switch/examples/
  GitHub:         https://github.com/carrierone/redfire-switch

For support, visit: https://support.carrierone.com

EOF
}

# Main installation flow
main() {
    echo "=== Redfire Switch Installer ==="
    echo ""
    
    check_root
    detect_os
    check_architecture
    
    # Install dependencies first
    install_dependencies
    
    # Try different installation methods
    if setup_repository && install_from_repository; then
        log_success "Installed from repository"
    elif install_from_package; then
        log_success "Installed from package"
    else
        log_info "Falling back to source build..."
        build_from_source
    fi
    
    # Post-installation setup
    configure_firewall
    setup_database
    
    # Display completion info
    post_install_info
}

# Handle command line arguments
case "${1:-}" in
    --help|-h)
        echo "Redfire Switch Installer"
        echo ""
        echo "Usage: $0 [OPTIONS]"
        echo ""
        echo "Options:"
        echo "  --help, -h          Show this help message"
        echo "  --source            Force build from source"
        echo "  --package-only      Only install package dependencies"
        echo "  --no-firewall       Skip firewall configuration"
        echo "  --no-database       Skip database setup"
        echo ""
        exit 0
        ;;
    --source)
        check_root
        detect_os
        check_architecture
        install_dependencies
        build_from_source
        post_install_info
        ;;
    --package-only)
        check_root
        detect_os
        install_dependencies
        ;;
    *)
        main
        ;;
esac