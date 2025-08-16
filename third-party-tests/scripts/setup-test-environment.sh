#!/bin/bash

# Redfire Switch - Third Party Test Environment Setup
# This script sets up Asterisk, FreeSWITCH, and PJSIP for interoperability testing

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging function
log() {
    echo -e "${BLUE}[$(date +'%Y-%m-%d %H:%M:%S')] $1${NC}"
}

error() {
    echo -e "${RED}[ERROR] $1${NC}"
    exit 1
}

success() {
    echo -e "${GREEN}[SUCCESS] $1${NC}"
}

warn() {
    echo -e "${YELLOW}[WARNING] $1${NC}"
}

# Check if running as root for some operations
check_root() {
    if [[ $EUID -ne 0 ]]; then
        warn "Some operations may require sudo privileges"
    fi
}

# Detect OS
detect_os() {
    if [[ -f /etc/os-release ]]; then
        . /etc/os-release
        OS=$NAME
        VERSION=$VERSION_ID
    else
        error "Cannot detect operating system"
    fi
    log "Detected OS: $OS $VERSION"
}

# Install system dependencies
install_dependencies() {
    log "Installing system dependencies..."
    
    case "$OS" in
        *"Ubuntu"*|*"Debian"*)
            sudo apt-get update
            sudo apt-get install -y \
                asterisk asterisk-modules asterisk-config \
                freeswitch freeswitch-mod-sofia freeswitch-mod-dialplan-xml \
                freeswitch-mod-conference freeswitch-sounds-music \
                libpjproject2 pjsip-tools \
                sipp \
                tcpdump wireshark-common \
                sox libsox-fmt-all \
                curl wget git \
                docker.io docker-compose \
                python3 python3-pip \
                nodejs npm
            ;;
        *"CentOS"*|*"Red Hat"*|*"Rocky"*)
            sudo yum install -y epel-release
            sudo yum install -y \
                asterisk asterisk-configs \
                freeswitch freeswitch-application-conference \
                sipp \
                tcpdump wireshark \
                sox \
                curl wget git \
                docker docker-compose \
                python3 python3-pip \
                nodejs npm
            ;;
        *"Arch"*)
            sudo pacman -Sy --noconfirm \
                asterisk \
                freeswitch \
                sipp \
                tcpdump wireshark-cli \
                sox \
                curl wget git \
                docker docker-compose \
                python python-pip \
                nodejs npm
            ;;
        *)
            warn "Unsupported OS: $OS. Please install dependencies manually."
            ;;
    esac
    
    success "System dependencies installed"
}

# Setup test directories
setup_directories() {
    log "Setting up test directories..."
    
    # Create base directories if they don't exist
    mkdir -p logs/{asterisk,freeswitch,pjsip,redfire,reports}
    mkdir -p recordings/{asterisk,freeswitch,pjsip}
    mkdir -p sounds
    mkdir -p docker
    
    # Set permissions
    chmod 755 logs recordings sounds docker
    chmod +x scripts/*.sh 2>/dev/null || true
    
    success "Directories setup complete"
}

# Configure Asterisk
setup_asterisk() {
    log "Configuring Asterisk..."
    
    # Backup original configs
    if [[ -d /etc/asterisk ]]; then
        sudo cp -r /etc/asterisk /etc/asterisk.backup.$(date +%Y%m%d_%H%M%S) 2>/dev/null || true
    fi
    
    # Copy our test configurations
    sudo cp asterisk/sip.conf /etc/asterisk/ 2>/dev/null || warn "Could not copy sip.conf"
    sudo cp asterisk/pjsip.conf /etc/asterisk/ 2>/dev/null || warn "Could not copy pjsip.conf" 
    sudo cp asterisk/extensions.conf /etc/asterisk/ 2>/dev/null || warn "Could not copy extensions.conf"
    
    # Enable required modules
    cat > /tmp/modules.conf << 'EOF'
[modules]
autoload=yes

; Core modules
load = app_dial.so
load = app_playback.so
load = app_echo.so
load = app_answer.so
load = app_hangup.so
load = app_read.so
load = app_saydigits.so
load = app_confbridge.so
load = app_musiconhold.so

; Channel drivers
load = chan_sip.so
load = chan_pjsip.so

; Codecs
load = codec_ulaw.so
load = codec_alaw.so
load = codec_g729.so
load = codec_g722.so

; RTP
load = res_rtp_asterisk.so

; PBX modules  
load = pbx_config.so

; Applications
load = app_verbose.so
load = app_noop.so
load = func_callerid.so
load = func_channel.so

noload = chan_alsa.so
noload = chan_oss.so
noload = chan_console.so
EOF

    sudo cp /tmp/modules.conf /etc/asterisk/ 2>/dev/null || warn "Could not copy modules.conf"
    
    # Test Asterisk configuration
    if command -v asterisk >/dev/null 2>&1; then
        asterisk -T -c "core show settings" >/dev/null 2>&1 && success "Asterisk configuration valid" || warn "Asterisk configuration may have issues"
    fi
}

# Configure FreeSWITCH
setup_freeswitch() {
    log "Configuring FreeSWITCH..."
    
    # Backup original configs
    if [[ -d /etc/freeswitch ]]; then
        sudo cp -r /etc/freeswitch /etc/freeswitch.backup.$(date +%Y%m%d_%H%M%S) 2>/dev/null || true
    fi
    
    # Copy our test configurations
    sudo cp freeswitch/sofia.conf.xml /etc/freeswitch/autoload_configs/ 2>/dev/null || warn "Could not copy sofia.conf.xml"
    sudo cp freeswitch/dialplan/public.xml /etc/freeswitch/dialplan/ 2>/dev/null || warn "Could not copy public.xml"
    sudo cp freeswitch/dialplan/techprefix.xml /etc/freeswitch/dialplan/ 2>/dev/null || warn "Could not copy techprefix.xml"
    
    # Test FreeSWITCH configuration
    if command -v freeswitch >/dev/null 2>&1; then
        freeswitch -nonat -nocal -nort -t >/dev/null 2>&1 && success "FreeSWITCH configuration valid" || warn "FreeSWITCH configuration may have issues"
    fi
}

# Setup PJSIP tools
setup_pjsip() {
    log "Setting up PJSIP tools..."
    
    # Download test audio if not present
    if [[ ! -f sounds/test-audio.wav ]]; then
        log "Downloading test audio file..."
        curl -o sounds/test-audio.wav "http://www.kozco.com/tech/organfinale.wav" 2>/dev/null || \
        sox -n -r 8000 -c 1 sounds/test-audio.wav synth 30 sine 440 vol 0.5 2>/dev/null || \
        warn "Could not create test audio file"
    fi
    
    success "PJSIP tools setup complete"
}

# Create Docker configurations
setup_docker() {
    log "Creating Docker configurations..."
    
    cat > docker/docker-compose.yml << 'EOF'
version: '3.8'

services:
  asterisk:
    image: asterisk:18
    container_name: redfire-test-asterisk
    ports:
      - "5061:5060/udp"
      - "5061:5060/tcp"
      - "10000-10100:10000-10100/udp"
    volumes:
      - ../asterisk:/etc/asterisk:ro
      - ../logs/asterisk:/var/log/asterisk
      - ../recordings/asterisk:/var/spool/asterisk/monitor
    environment:
      - ASTERISK_UID=1001
      - ASTERISK_GID=1001
    restart: unless-stopped
    networks:
      - redfire-test

  freeswitch:
    image: freeswitch:latest
    container_name: redfire-test-freeswitch
    ports:
      - "5063-5066:5063-5066/udp"
      - "5063-5066:5063-5066/tcp"
      - "20000-20500:20000-20500/udp"
    volumes:
      - ../freeswitch:/etc/freeswitch:ro
      - ../logs/freeswitch:/var/log/freeswitch
      - ../recordings/freeswitch:/var/lib/freeswitch/recordings
    restart: unless-stopped
    networks:
      - redfire-test

  sipp:
    image: ctaloi/sipp:latest
    container_name: redfire-test-sipp
    volumes:
      - ../pjsip/scenarios:/scenarios:ro
      - ../logs/pjsip:/logs
    networks:
      - redfire-test
    profiles:
      - testing

networks:
  redfire-test:
    driver: bridge
    ipam:
      config:
        - subnet: 172.20.0.0/16
EOF

    success "Docker configuration created"
}

# Create monitoring scripts
create_monitoring_scripts() {
    log "Creating monitoring scripts..."
    
    cat > scripts/monitor-traffic.sh << 'EOF'
#!/bin/bash

# Monitor SIP traffic for testing
INTERFACE=${1:-lo}
REDFIRE_PORT=${2:-5060}

echo "Monitoring SIP traffic on interface $INTERFACE, port $REDFIRE_PORT"
echo "Press Ctrl+C to stop"
echo

sudo tcpdump -i $INTERFACE -n -s 0 -A "port $REDFIRE_PORT" | tee logs/sip-traffic-$(date +%Y%m%d_%H%M%S).log
EOF

    cat > scripts/test-connectivity.sh << 'EOF'
#!/bin/bash

# Test basic connectivity to Redfire Switch
REDFIRE_IP=${1:-127.0.0.1}
REDFIRE_PORT=${2:-5060}

echo "Testing connectivity to Redfire Switch at $REDFIRE_IP:$REDFIRE_PORT"

# Test UDP connectivity
echo -n "UDP connectivity: "
if timeout 5 bash -c "</dev/udp/$REDFIRE_IP/$REDFIRE_PORT" 2>/dev/null; then
    echo "OK"
else
    echo "FAILED"
fi

# Test TCP connectivity  
echo -n "TCP connectivity: "
if timeout 5 bash -c "</dev/tcp/$REDFIRE_IP/$REDFIRE_PORT" 2>/dev/null; then
    echo "OK"
else
    echo "FAILED"
fi

# Test SIP OPTIONS
echo -n "SIP OPTIONS ping: "
if command -v sipp >/dev/null 2>&1; then
    timeout 10 sipp -sf pjsip/scenarios/options_ping.xml $REDFIRE_IP:$REDFIRE_PORT -m 1 -q >/dev/null 2>&1 && echo "OK" || echo "FAILED"
else
    echo "SKIPPED (sipp not available)"
fi
EOF

    chmod +x scripts/monitor-traffic.sh scripts/test-connectivity.sh
    success "Monitoring scripts created"
}

# Create simple SIP OPTIONS scenario
create_options_scenario() {
    cat > pjsip/scenarios/options_ping.xml << 'EOF'
<?xml version="1.0" encoding="ISO-8859-1" ?>
<!DOCTYPE scenario SYSTEM "sipp.dtd">

<scenario name="OPTIONS Ping">
  <send retrans="500">
    <![CDATA[
      OPTIONS sip:[remote_ip]:[remote_port] SIP/2.0
      Via: SIP/2.0/[transport] [local_ip]:[local_port];branch=[branch]
      Max-Forwards: 70
      To: <sip:[remote_ip]:[remote_port]>
      From: <sip:test@[local_ip]:[local_port]>;tag=[pid]SIPpTag00[call_number]
      Call-ID: [call_id]
      CSeq: 1 OPTIONS
      Content-Length: 0
    ]]>
  </send>

  <recv response="200">
  </recv>
</scenario>
EOF
}

# Main setup function
main() {
    log "Starting Redfire Switch Third-Party Test Environment Setup"
    log "============================================================"
    
    # Change to script directory
    cd "$(dirname "$0")/.."
    
    check_root
    detect_os
    setup_directories
    create_options_scenario
    
    # Install software (optional, comment out if already installed)
    read -p "Install system dependencies? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        install_dependencies
    fi
    
    setup_asterisk
    setup_freeswitch  
    setup_pjsip
    setup_docker
    create_monitoring_scripts
    
    success "Test environment setup complete!"
    echo
    log "Next steps:"
    echo "  1. Start Redfire Switch: cd .. && cargo run -- start"
    echo "  2. Run connectivity tests: ./scripts/test-connectivity.sh"
    echo "  3. Run full test suite: ./scripts/run-interop-tests.sh"
    echo "  4. Monitor traffic: ./scripts/monitor-traffic.sh"
    echo
    log "Docker usage:"
    echo "  Start services: cd docker && docker-compose up -d"
    echo "  Stop services: cd docker && docker-compose down"
    echo "  View logs: cd docker && docker-compose logs -f"
    echo
}

# Run main function
main "$@"