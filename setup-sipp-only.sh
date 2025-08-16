#!/bin/bash

# Quick SIPp installation for Debian/Ubuntu
set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

print_color() {
    echo -e "${1}${2}${NC}"
}

print_color "$YELLOW" "Installing SIPp and basic dependencies for B2BUA testing..."

# Update package list
sudo apt-get update

# Install SIPp and network tools
sudo apt-get install -y \
    sipp \
    netcat-openbsd \
    tcpdump \
    net-tools \
    iputils-ping

print_color "$GREEN" "✅ SIPp and network tools installed"

# Test SIPp
if sipp -v | head -1; then
    print_color "$GREEN" "✅ SIPp working correctly"
else
    print_color "$RED" "❌ SIPp installation failed"
fi

print_color "$GREEN" "🚀 Ready for B2BUA testing!"
echo "Run: ./test-b2bua.sh"