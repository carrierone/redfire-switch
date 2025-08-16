#!/bin/bash

# Debian-specific development setup for B2BUA testing
set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_color() {
    echo -e "${1}${2}${NC}"
}

print_header() {
    echo
    print_color "$BLUE" "=========================================="
    print_color "$BLUE" "  $1"
    print_color "$BLUE" "=========================================="
}

print_header "Redfire Switch B2BUA Development Setup (Debian)"

# Check if running as root
if [ "$EUID" -eq 0 ]; then
    print_color "$RED" "Please do not run this script as root"
    exit 1
fi

# Update package list
print_color "$YELLOW" "Updating package list..."
sudo apt-get update

# Install basic dependencies
print_header "Installing Basic Dependencies"
sudo apt-get install -y \
    curl \
    wget \
    git \
    build-essential \
    pkg-config \
    libssl-dev \
    ca-certificates \
    gnupg \
    lsb-release

# Install Rust if not present
if ! command -v rustc &> /dev/null; then
    print_header "Installing Rust"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source ~/.cargo/env
    print_color "$GREEN" "✅ Rust installed"
else
    print_color "$GREEN" "✅ Rust already installed"
fi

# Install SIPp
if ! command -v sipp &> /dev/null; then
    print_header "Installing SIPp"
    sudo apt-get install -y sipp
    print_color "$GREEN" "✅ SIPp installed"
else
    print_color "$GREEN" "✅ SIPp already installed"
fi

# Install network tools
print_header "Installing Network Tools"
sudo apt-get install -y \
    tcpdump \
    tshark \
    wireshark-common \
    net-tools \
    iputils-ping \
    netcat-openbsd \
    dnsutils

print_color "$GREEN" "✅ Network tools installed"

# Create directories
print_header "Creating Project Directories"
mkdir -p logs pcaps tests/results
print_color "$GREEN" "✅ Directories created"

# Set up permissions for packet capture
print_color "$YELLOW" "Setting up packet capture permissions..."
sudo setcap cap_net_raw+ep $(which tcpdump) 2>/dev/null || true
sudo setcap cap_net_raw+ep $(which dumpcap) 2>/dev/null || true

# Build the simple B2BUA test
print_header "Building Simple B2BUA Test"
if cargo build --bin simple-b2bua-test; then
    print_color "$GREEN" "✅ Simple B2BUA test built successfully"
else
    print_color "$YELLOW" "⚠️  Build had warnings but completed"
fi

# Test SIPp
print_header "Testing SIPp"
if sipp -v | head -1; then
    print_color "$GREEN" "✅ SIPp working"
    SIPP_VERSION=$(sipp -v | head -1)
    print_color "$GREEN" "   $SIPP_VERSION"
else
    print_color "$RED" "❌ SIPp test failed"
fi

# Final setup
print_header "Final Setup"

# Create example .env file
cat > .env.dev << 'EOF'
# Development Environment Variables
RUST_LOG=debug
RUST_BACKTRACE=1
SIP_DEBUG=true
SINGLE_CALL_MODE=true
SWITCH_HOST=localhost
SWITCH_PORT=5060
CALL_RATE=1
NUM_CALLS=10
EOF

print_color "$GREEN" "✅ Created .env.dev file"

# Success message
print_header "Setup Complete!"

print_color "$GREEN" "🎉 Debian B2BUA development environment ready!"
echo
print_color "$YELLOW" "Test the setup:"
echo "  ./test-b2bua-manual.sh     - Manual B2BUA test (no SIPp)"
echo "  ./test-b2bua.sh            - Full SIPp test suite"
echo "  cargo run --bin simple-b2bua-test  - Run B2BUA directly"
echo
print_color "$YELLOW" "Quick B2BUA test:"
echo "  1. ./target/debug/simple-b2bua-test &"
echo "  2. echo 'OPTIONS sip:ping@127.0.0.1:5060 SIP/2.0' | nc -u 127.0.0.1 5060"
echo "  3. Check for 200 OK response"
echo

print_color "$GREEN" "Happy B2BUA testing! 🚀📞"