#!/bin/bash

# Vosk Server Setup Script for Anti-Fraud Voice Monitoring
# Based on https://alphacephei.com/vosk/server instructions
# Tested on Ubuntu 20.04/22.04 and CentOS 8/9

set -euo pipefail

# Configuration
VOSK_MODEL_URL="https://alphacephei.com/vosk/models/vosk-model-en-us-0.22.zip"
VOSK_MODEL_DIR="/opt/vosk-model"
VOSK_SERVER_DIR="/opt/vosk-server"
VOSK_SERVER_PORT="2700"
VOSK_USER="vosk"
VOSK_GROUP="vosk"
SERVICE_NAME="vosk-speech-recognition"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging function
log() {
    echo -e "${GREEN}[$(date +'%Y-%m-%d %H:%M:%S')]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
    exit 1
}

warn() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

# Check if running as root
check_root() {
    if [[ $EUID -ne 0 ]]; then
        error "This script must be run as root (use sudo)"
    fi
}

# Detect OS
detect_os() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        OS=$NAME
        VER=$VERSION_ID
    else
        error "Cannot detect OS. Only Ubuntu and CentOS/RHEL are supported."
    fi

    log "Detected OS: $OS $VER"
}

# Install dependencies
install_dependencies() {
    log "Installing dependencies..."

    case "$OS" in
        "Ubuntu"*)
            apt-get update
            apt-get install -y python3 python3-pip python3-venv \
                              wget unzip curl supervisor \
                              build-essential libffi-dev \
                              portaudio19-dev python3-pyaudio \
                              ffmpeg
            ;;
        "CentOS"*|"Red Hat"*|"Rocky"*|"AlmaLinux"*)
            yum update -y
            yum install -y python3 python3-pip \
                          wget unzip curl supervisor \
                          gcc gcc-c++ make libffi-devel \
                          portaudio-devel \
                          ffmpeg
            ;;
        *)
            error "Unsupported OS: $OS"
            ;;
    esac
}

# Create vosk user
create_user() {
    log "Creating vosk user and group..."

    if ! getent group "$VOSK_GROUP" > /dev/null 2>&1; then
        groupadd "$VOSK_GROUP"
        log "Created group: $VOSK_GROUP"
    fi

    if ! getent passwd "$VOSK_USER" > /dev/null 2>&1; then
        useradd -r -g "$VOSK_GROUP" -s /bin/false -d "$VOSK_SERVER_DIR" "$VOSK_USER"
        log "Created user: $VOSK_USER"
    fi
}

# Download and setup Vosk model
setup_model() {
    log "Setting up Vosk model..."

    # Create model directory
    mkdir -p "$VOSK_MODEL_DIR"

    # Download model if not exists
    if [ ! -f "$VOSK_MODEL_DIR/am/final.mdl" ]; then
        info "Downloading Vosk English model (this may take a while)..."
        wget -O /tmp/vosk-model.zip "$VOSK_MODEL_URL"

        log "Extracting model..."
        unzip -q /tmp/vosk-model.zip -d /tmp/

        # Find the extracted directory (name may vary)
        MODEL_EXTRACTED_DIR=$(find /tmp -maxdepth 1 -name "vosk-model-*" -type d | head -1)

        if [ -z "$MODEL_EXTRACTED_DIR" ]; then
            error "Failed to find extracted model directory"
        fi

        # Move model files
        mv "$MODEL_EXTRACTED_DIR"/* "$VOSK_MODEL_DIR/"

        # Clean up
        rm -rf /tmp/vosk-model.zip "$MODEL_EXTRACTED_DIR"

        log "Model extracted to: $VOSK_MODEL_DIR"
    else
        log "Vosk model already exists"
    fi

    # Set permissions
    chown -R "$VOSK_USER:$VOSK_GROUP" "$VOSK_MODEL_DIR"
    chmod -R 755 "$VOSK_MODEL_DIR"
}

# Install Vosk server
install_vosk_server() {
    log "Installing Vosk server..."

    # Create server directory
    mkdir -p "$VOSK_SERVER_DIR"

    # Create Python virtual environment
    python3 -m venv "$VOSK_SERVER_DIR/venv"

    # Activate virtual environment and install packages
    source "$VOSK_SERVER_DIR/venv/bin/activate"

    # Upgrade pip
    pip install --upgrade pip

    # Install Vosk and WebSocket server
    pip install vosk websockets asyncio soundfile

    deactivate

    log "Vosk server environment created"
}

# Create Vosk server script
create_server_script() {
    log "Creating Vosk server script..."

    cat > "$VOSK_SERVER_DIR/vosk_server.py" << 'EOF'
#!/usr/bin/env python3
"""
Vosk WebSocket Server for Anti-Fraud Voice Monitoring
Provides real-time speech recognition via WebSocket API
"""

import asyncio
import websockets
import json
import logging
import vosk
import sys
import os
from pathlib import Path

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s',
    handlers=[
        logging.FileHandler('/var/log/vosk-server.log'),
        logging.StreamHandler(sys.stdout)
    ]
)
logger = logging.getLogger(__name__)

class VoskServer:
    def __init__(self, model_path="/opt/vosk-model", sample_rate=8000):
        self.model_path = model_path
        self.sample_rate = sample_rate
        self.model = None
        self.load_model()

    def load_model(self):
        """Load Vosk model"""
        try:
            logger.info(f"Loading Vosk model from {self.model_path}")

            if not os.path.exists(self.model_path):
                raise FileNotFoundError(f"Model directory not found: {self.model_path}")

            self.model = vosk.Model(self.model_path)
            logger.info("Vosk model loaded successfully")
        except Exception as e:
            logger.error(f"Failed to load model: {e}")
            sys.exit(1)

    async def handle_client(self, websocket, path):
        """Handle WebSocket client connection"""
        client_addr = f"{websocket.remote_address[0]}:{websocket.remote_address[1]}"
        logger.info(f"Client connected: {client_addr}")

        recognizer = vosk.KaldiRecognizer(self.model, self.sample_rate)

        try:
            async for message in websocket:
                if isinstance(message, bytes):
                    # Audio data
                    if recognizer.AcceptWaveform(message):
                        result = json.loads(recognizer.Result())
                        await websocket.send(json.dumps({
                            "type": "result",
                            "text": result.get("text", ""),
                            "confidence": result.get("confidence", 0.0)
                        }))
                    else:
                        partial = json.loads(recognizer.PartialResult())
                        await websocket.send(json.dumps({
                            "type": "partial",
                            "text": partial.get("partial", "")
                        }))
                elif isinstance(message, str):
                    # Control message
                    try:
                        cmd = json.loads(message)
                        if cmd.get("command") == "reset":
                            recognizer = vosk.KaldiRecognizer(self.model, self.sample_rate)
                            await websocket.send(json.dumps({
                                "type": "status",
                                "message": "recognizer_reset"
                            }))
                        elif cmd.get("command") == "final":
                            final = json.loads(recognizer.FinalResult())
                            await websocket.send(json.dumps({
                                "type": "final",
                                "text": final.get("text", ""),
                                "confidence": final.get("confidence", 0.0)
                            }))
                    except json.JSONDecodeError:
                        logger.warning(f"Invalid JSON from {client_addr}: {message}")

        except websockets.exceptions.ConnectionClosed:
            logger.info(f"Client disconnected: {client_addr}")
        except Exception as e:
            logger.error(f"Error handling client {client_addr}: {e}")
        finally:
            logger.info(f"Cleaning up connection for {client_addr}")

    async def start_server(self, host="0.0.0.0", port=2700):
        """Start the WebSocket server"""
        logger.info(f"Starting Vosk server on {host}:{port}")

        try:
            async with websockets.serve(self.handle_client, host, port):
                logger.info(f"Vosk server listening on ws://{host}:{port}")
                await asyncio.Future()  # Run forever
        except Exception as e:
            logger.error(f"Failed to start server: {e}")
            sys.exit(1)

def main():
    """Main entry point"""
    import argparse

    parser = argparse.ArgumentParser(description='Vosk WebSocket Server')
    parser.add_argument('--model', default='/opt/vosk-model',
                       help='Path to Vosk model directory')
    parser.add_argument('--host', default='0.0.0.0',
                       help='Server host (default: 0.0.0.0)')
    parser.add_argument('--port', type=int, default=2700,
                       help='Server port (default: 2700)')
    parser.add_argument('--sample-rate', type=int, default=8000,
                       help='Audio sample rate (default: 8000)')

    args = parser.parse_args()

    # Create and start server
    server = VoskServer(model_path=args.model, sample_rate=args.sample_rate)

    try:
        asyncio.run(server.start_server(host=args.host, port=args.port))
    except KeyboardInterrupt:
        logger.info("Server stopped by user")
    except Exception as e:
        logger.error(f"Server error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
EOF

    chmod +x "$VOSK_SERVER_DIR/vosk_server.py"
    chown -R "$VOSK_USER:$VOSK_GROUP" "$VOSK_SERVER_DIR"

    log "Vosk server script created"
}

# Create systemd service
create_systemd_service() {
    log "Creating systemd service..."

    cat > "/etc/systemd/system/${SERVICE_NAME}.service" << EOF
[Unit]
Description=Vosk Speech Recognition Server
After=network.target
Wants=network.target

[Service]
Type=simple
User=$VOSK_USER
Group=$VOSK_GROUP
WorkingDirectory=$VOSK_SERVER_DIR
Environment=PATH=$VOSK_SERVER_DIR/venv/bin:/usr/local/bin:/usr/bin:/bin
ExecStart=$VOSK_SERVER_DIR/venv/bin/python $VOSK_SERVER_DIR/vosk_server.py --host 0.0.0.0 --port $VOSK_SERVER_PORT --model $VOSK_MODEL_DIR
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal
SyslogIdentifier=vosk-server

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=$VOSK_SERVER_DIR /var/log
CapabilityBoundingSet=

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    log "Systemd service created: $SERVICE_NAME"
}

# Create supervisor configuration (alternative to systemd)
create_supervisor_config() {
    log "Creating supervisor configuration..."

    cat > "/etc/supervisor/conf.d/${SERVICE_NAME}.conf" << EOF
[program:${SERVICE_NAME}]
command=$VOSK_SERVER_DIR/venv/bin/python $VOSK_SERVER_DIR/vosk_server.py --host 0.0.0.0 --port $VOSK_SERVER_PORT --model $VOSK_MODEL_DIR
directory=$VOSK_SERVER_DIR
user=$VOSK_USER
group=$VOSK_GROUP
autostart=true
autorestart=true
startretries=3
redirect_stderr=true
stdout_logfile=/var/log/vosk-server.log
stdout_logfile_maxbytes=50MB
stdout_logfile_backups=5
environment=PATH="$VOSK_SERVER_DIR/venv/bin:/usr/local/bin:/usr/bin:/bin"
EOF

    log "Supervisor configuration created"
}

# Setup log rotation
setup_logging() {
    log "Setting up log rotation..."

    cat > "/etc/logrotate.d/vosk-server" << EOF
/var/log/vosk-server.log {
    daily
    missingok
    rotate 30
    compress
    delaycompress
    notifempty
    copytruncate
    su $VOSK_USER $VOSK_GROUP
}
EOF

    # Create log file
    touch /var/log/vosk-server.log
    chown "$VOSK_USER:$VOSK_GROUP" /var/log/vosk-server.log

    log "Log rotation configured"
}

# Create firewall rules
setup_firewall() {
    log "Setting up firewall rules..."

    # UFW (Ubuntu)
    if command -v ufw &> /dev/null; then
        ufw allow "$VOSK_SERVER_PORT/tcp" comment "Vosk Speech Recognition Server"
        log "UFW firewall rule added for port $VOSK_SERVER_PORT"
    fi

    # Firewalld (CentOS/RHEL)
    if command -v firewall-cmd &> /dev/null; then
        firewall-cmd --permanent --add-port="$VOSK_SERVER_PORT/tcp"
        firewall-cmd --reload
        log "Firewalld rule added for port $VOSK_SERVER_PORT"
    fi
}

# Create client test script
create_test_client() {
    log "Creating test client script..."

    cat > "$VOSK_SERVER_DIR/test_client.py" << 'EOF'
#!/usr/bin/env python3
"""
Test client for Vosk WebSocket server
"""

import asyncio
import websockets
import json
import wave
import sys

async def test_connection(uri, audio_file=None):
    """Test connection to Vosk server"""
    try:
        async with websockets.connect(uri) as websocket:
            print(f"Connected to {uri}")

            if audio_file:
                # Send audio file
                with wave.open(audio_file, 'rb') as wf:
                    print(f"Sending audio file: {audio_file}")

                    while True:
                        data = wf.readframes(1024)
                        if not data:
                            break
                        await websocket.send(data)

                        # Listen for response
                        try:
                            response = await asyncio.wait_for(websocket.recv(), timeout=0.1)
                            result = json.loads(response)
                            if result.get('type') == 'result':
                                print(f"Result: {result.get('text')}")
                        except asyncio.TimeoutError:
                            pass

                # Send final command
                await websocket.send(json.dumps({"command": "final"}))
                final_response = await websocket.recv()
                final_result = json.loads(final_response)
                print(f"Final: {final_result.get('text')}")
            else:
                # Just test connection
                await websocket.send(json.dumps({"command": "reset"}))
                response = await websocket.recv()
                print(f"Server response: {response}")

    except Exception as e:
        print(f"Connection failed: {e}")
        return False

    return True

def main():
    """Main entry point"""
    import argparse

    parser = argparse.ArgumentParser(description='Test Vosk server')
    parser.add_argument('--host', default='localhost', help='Server host')
    parser.add_argument('--port', type=int, default=2700, help='Server port')
    parser.add_argument('--audio', help='WAV audio file to test')

    args = parser.parse_args()

    uri = f"ws://{args.host}:{args.port}"

    success = asyncio.run(test_connection(uri, args.audio))

    if success:
        print("✓ Connection test successful")
        sys.exit(0)
    else:
        print("✗ Connection test failed")
        sys.exit(1)

if __name__ == "__main__":
    main()
EOF

    chmod +x "$VOSK_SERVER_DIR/test_client.py"
    chown "$VOSK_USER:$VOSK_GROUP" "$VOSK_SERVER_DIR/test_client.py"

    log "Test client script created"
}

# Main installation function
main() {
    log "Starting Vosk server installation..."

    check_root
    detect_os
    install_dependencies
    create_user
    setup_model
    install_vosk_server
    create_server_script

    # Choose service management system
    if systemctl --version &> /dev/null; then
        create_systemd_service

        log "Enabling and starting service..."
        systemctl enable "$SERVICE_NAME"
        systemctl start "$SERVICE_NAME"

        # Check status
        if systemctl is-active --quiet "$SERVICE_NAME"; then
            log "✓ Vosk server is running"
        else
            warn "Service may not have started correctly. Check logs with: journalctl -u $SERVICE_NAME"
        fi
    else
        create_supervisor_config

        log "Starting supervisor..."
        supervisorctl reread
        supervisorctl update
        supervisorctl start "$SERVICE_NAME"

        if supervisorctl status "$SERVICE_NAME" | grep -q "RUNNING"; then
            log "✓ Vosk server is running under supervisor"
        else
            warn "Service may not have started correctly. Check logs with: supervisorctl tail $SERVICE_NAME"
        fi
    fi

    setup_logging
    setup_firewall
    create_test_client

    log "Installation completed successfully!"
    info ""
    info "Vosk Speech Recognition Server is now installed and running."
    info ""
    info "Configuration:"
    info "  Model directory: $VOSK_MODEL_DIR"
    info "  Server directory: $VOSK_SERVER_DIR"
    info "  WebSocket endpoint: ws://localhost:$VOSK_SERVER_PORT"
    info "  Service name: $SERVICE_NAME"
    info "  User: $VOSK_USER"
    info ""
    info "Useful commands:"
    info "  Check status:    systemctl status $SERVICE_NAME"
    info "  View logs:       journalctl -u $SERVICE_NAME -f"
    info "  Restart:         systemctl restart $SERVICE_NAME"
    info "  Test connection: $VOSK_SERVER_DIR/venv/bin/python $VOSK_SERVER_DIR/test_client.py"
    info ""
    info "Log files:"
    info "  Server log: /var/log/vosk-server.log"
    info ""
    warn "Remember to configure your firewall to allow connections on port $VOSK_SERVER_PORT"
    warn "if you need external access to the Vosk server."
}

# Run main function
main "$@"