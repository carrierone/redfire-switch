#!/bin/bash

# Redfire Switch Development Environment Setup Script

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

print_header "Redfire Switch Development Environment Setup"

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

# Install Docker if not present
if ! command -v docker &> /dev/null; then
    print_header "Installing Docker"
    
    # Detect OS
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        OS=$ID
        CODENAME=$VERSION_CODENAME
    else
        print_color "$RED" "Cannot detect OS version"
        exit 1
    fi
    
    if [ "$OS" = "debian" ]; then
        print_color "$YELLOW" "Detected Debian $CODENAME"
        curl -fsSL https://download.docker.com/linux/debian/gpg | sudo gpg --dearmor -o /usr/share/keyrings/docker-archive-keyring.gpg
        echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/docker-archive-keyring.gpg] https://download.docker.com/linux/debian $CODENAME stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null
    elif [ "$OS" = "ubuntu" ]; then
        print_color "$YELLOW" "Detected Ubuntu $CODENAME"
        curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /usr/share/keyrings/docker-archive-keyring.gpg
        echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/docker-archive-keyring.gpg] https://download.docker.com/linux/ubuntu $CODENAME stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null
    else
        print_color "$YELLOW" "⚠️  Unknown OS $OS, trying Debian packages..."
        curl -fsSL https://download.docker.com/linux/debian/gpg | sudo gpg --dearmor -o /usr/share/keyrings/docker-archive-keyring.gpg
        echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/docker-archive-keyring.gpg] https://download.docker.com/linux/debian bookworm stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null
    fi
    
    sudo apt-get update
    sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin
    
    # Add user to docker group
    sudo usermod -aG docker $USER
    print_color "$GREEN" "✅ Docker installed"
    print_color "$YELLOW" "⚠️  You may need to log out and back in for Docker permissions to take effect"
else
    print_color "$GREEN" "✅ Docker already installed"
fi

# Install docker-compose if not present
if ! command -v docker-compose &> /dev/null; then
    print_header "Installing Docker Compose"
    sudo curl -L "https://github.com/docker/compose/releases/download/v2.20.0/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
    sudo chmod +x /usr/local/bin/docker-compose
    print_color "$GREEN" "✅ Docker Compose installed"
else
    print_color "$GREEN" "✅ Docker Compose already installed"
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
    netcat-openbsd

print_color "$GREEN" "✅ Network tools installed"

# Create directories
print_header "Creating Project Directories"
mkdir -p logs pcaps tests/results
print_color "$GREEN" "✅ Directories created"

# Set up permissions for packet capture
print_color "$YELLOW" "Setting up packet capture permissions..."
sudo setcap cap_net_raw+ep $(which tcpdump) 2>/dev/null || true
sudo setcap cap_net_raw+ep $(which dumpcap) 2>/dev/null || true

# Build the project
print_header "Building Redfire Switch"
if cargo build; then
    print_color "$GREEN" "✅ Build successful"
else
    print_color "$YELLOW" "⚠️  Build failed - this is expected due to missing implementations"
    print_color "$YELLOW" "   The development environment is still functional for testing"
fi

# Create symlink for easy makefile access
if [ ! -f Makefile ]; then
    ln -s Makefile.dev Makefile
    print_color "$GREEN" "✅ Created Makefile symlink"
fi

# Test Docker access
print_header "Testing Docker Access"
if docker ps &> /dev/null; then
    print_color "$GREEN" "✅ Docker access working"
    
    # Build Docker images
    print_color "$YELLOW" "Building Docker images..."
    if docker-compose -f docker-compose.dev.yml build; then
        print_color "$GREEN" "✅ Docker images built successfully"
    else
        print_color "$YELLOW" "⚠️  Docker build failed - you may need to fix compilation issues first"
    fi
else
    print_color "$YELLOW" "⚠️  Docker access not working - you may need to log out and back in"
fi

# Test SIPp
print_header "Testing SIPp"
if sipp -v | head -1; then
    print_color "$GREEN" "✅ SIPp working"
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

print_color "$GREEN" "🎉 Development environment setup complete!"
echo
print_color "$YELLOW" "Quick start commands:"
echo "  make help          - Show all available commands"
echo "  make dev           - Start development environment"
echo "  make test          - Run SIPp tests"
echo "  make debug         - Run switch in debug mode"
echo "  make pcap-live     - Monitor SIP traffic"
echo
print_color "$YELLOW" "Documentation:"
echo "  cat DEV-ENVIRONMENT.md    - Full documentation"
echo "  make help                 - Available commands"
echo
print_color "$YELLOW" "Test the setup:"
echo "  make docker-up            - Start Docker environment"
echo "  make test-options         - Test OPTIONS ping"
echo

if ! groups $USER | grep -q docker; then
    print_color "$YELLOW" "⚠️  IMPORTANT: You need to log out and back in for Docker permissions to take effect"
fi

print_color "$GREEN" "Happy testing! 🚀"