#!/bin/bash

# Redfire Switch Post-Installation Setup Script
# This script performs common post-installation tasks

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

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

# Check if running as root
check_root() {
    if [ "$EUID" -ne 0 ]; then
        log_error "This script must be run as root (use sudo)"
        exit 1
    fi
}

# Generate secure random passwords
generate_password() {
    openssl rand -base64 32 | tr -d "=+/" | cut -c1-25
}

# Configure PostgreSQL database
setup_database() {
    log_info "Setting up PostgreSQL database..."
    
    # Check if PostgreSQL is installed and running
    if ! systemctl is-active --quiet postgresql; then
        log_error "PostgreSQL is not running. Please start it first:"
        log_info "  systemctl start postgresql"
        return 1
    fi
    
    # Generate secure passwords
    DB_PASSWORD=$(generate_password)
    
    # Create database and user
    log_info "Creating database and user..."
    sudo -u postgres psql << EOF
-- Create database
CREATE DATABASE redfire_switch;

-- Create user with secure password
CREATE USER redfire WITH PASSWORD '$DB_PASSWORD';

-- Grant privileges
GRANT ALL PRIVILEGES ON DATABASE redfire_switch TO redfire;

-- Create extensions
\c redfire_switch;
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pg_stat_statements";

\q
EOF

    # Update configuration file with database credentials
    if [ -f /etc/redfire-switch/config.toml ]; then
        sed -i "s|postgresql://redfire:password@localhost/redfire_switch|postgresql://redfire:$DB_PASSWORD@localhost/redfire_switch|g" /etc/redfire-switch/config.toml
        log_success "Updated database configuration"
    fi
    
    # Save credentials to secure file
    cat > /etc/redfire-switch/db-credentials << EOF
# Database credentials - keep secure!
DB_HOST=localhost
DB_NAME=redfire_switch
DB_USER=redfire
DB_PASSWORD=$DB_PASSWORD
EOF
    chown root:redfire /etc/redfire-switch/db-credentials
    chmod 640 /etc/redfire-switch/db-credentials
    
    log_success "Database setup complete"
    log_info "Database password saved to: /etc/redfire-switch/db-credentials"
}

# Initialize database schema
init_database_schema() {
    log_info "Initializing database schema..."
    
    if [ -f /usr/share/redfire-switch/schema.sql ]; then
        # Source credentials
        source /etc/redfire-switch/db-credentials
        
        # Run schema initialization
        PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -U "$DB_USER" -d "$DB_NAME" -f /usr/share/redfire-switch/schema.sql
        
        log_success "Database schema initialized"
    else
        log_warning "Database schema file not found. You may need to initialize manually."
    fi
}

# Configure Redis
setup_redis() {
    log_info "Configuring Redis..."
    
    # Check if Redis is installed
    if ! command -v redis-server >/dev/null 2>&1; then
        log_warning "Redis not found. Installing..."
        apt-get update && apt-get install -y redis-server
    fi
    
    # Configure Redis for Redfire Switch
    cat > /etc/redis/redis-redfire.conf << 'EOF'
# Redis configuration for Redfire Switch
port 6379
bind 127.0.0.1
protected-mode yes
tcp-keepalive 300
timeout 0

# Memory management
maxmemory 512mb
maxmemory-policy allkeys-lru

# Persistence
save 900 1
save 300 10
save 60 10000

# Logging
loglevel notice
logfile /var/log/redis/redis-redfire.log

# Security
requirepass REDIS_PASSWORD_PLACEHOLDER
EOF

    # Generate Redis password
    REDIS_PASSWORD=$(generate_password)
    sed -i "s/REDIS_PASSWORD_PLACEHOLDER/$REDIS_PASSWORD/g" /etc/redis/redis-redfire.conf
    
    # Update Redfire Switch configuration
    if [ -f /etc/redfire-switch/config.toml ]; then
        sed -i "s|redis://127.0.0.1:6379/0|redis://:$REDIS_PASSWORD@127.0.0.1:6379/0|g" /etc/redfire-switch/config.toml
    fi
    
    # Save Redis credentials
    echo "REDIS_PASSWORD=$REDIS_PASSWORD" >> /etc/redfire-switch/db-credentials
    
    # Start Redis with new configuration
    systemctl enable redis-server
    systemctl restart redis-server
    
    log_success "Redis configured and started"
}

# Generate SSL certificates
setup_ssl_certificates() {
    log_info "Setting up SSL certificates..."
    
    # Create certificate directory
    mkdir -p /etc/ssl/redfire-switch
    
    # Generate self-signed certificate for testing
    openssl req -x509 -newkey rsa:4096 -keyout /etc/ssl/redfire-switch/key.pem \
        -out /etc/ssl/redfire-switch/cert.pem -days 365 -nodes \
        -subj "/C=US/ST=State/L=City/O=Organization/CN=sip.example.com"
    
    # Set permissions
    chown root:redfire /etc/ssl/redfire-switch/*
    chmod 640 /etc/ssl/redfire-switch/key.pem
    chmod 644 /etc/ssl/redfire-switch/cert.pem
    
    # Update configuration
    if [ -f /etc/redfire-switch/config.toml ]; then
        sed -i 's|certificate_path = "/etc/ssl/certs/sip.example.com.pem"|certificate_path = "/etc/ssl/redfire-switch/cert.pem"|g' /etc/redfire-switch/config.toml
        sed -i 's|private_key_path = "/etc/ssl/private/sip.example.com.key"|private_key_path = "/etc/ssl/redfire-switch/key.pem"|g' /etc/redfire-switch/config.toml
    fi
    
    log_success "Self-signed SSL certificates generated"
    log_warning "For production, replace with proper certificates from a CA"
}

# Configure system limits
setup_system_limits() {
    log_info "Configuring system limits..."
    
    # Create limits configuration
    cat > /etc/security/limits.d/redfire-switch.conf << 'EOF'
# System limits for Redfire Switch
redfire    soft    nofile    65535
redfire    hard    nofile    65535
redfire    soft    nproc     32768
redfire    hard    nproc     32768
redfire-web soft   nofile    8192
redfire-web hard   nofile    8192
EOF

    # Configure systemd limits
    mkdir -p /etc/systemd/system/redfire-switch.service.d
    cat > /etc/systemd/system/redfire-switch.service.d/limits.conf << 'EOF'
[Service]
LimitNOFILE=65535
LimitNPROC=32768
EOF

    log_success "System limits configured"
}

# Configure firewall
setup_firewall() {
    log_info "Configuring firewall..."
    
    if command -v ufw >/dev/null 2>&1; then
        # UFW configuration
        ufw allow 5060/udp comment "SIP signaling"
        ufw allow 5060/tcp comment "SIP signaling"
        ufw allow 5061/tcp comment "SIP TLS"
        ufw allow 5061/udp comment "SIP TLS"
        ufw allow 10000:20000/udp comment "RTP media"
        
        log_success "UFW firewall rules added"
        log_info "Enable firewall with: ufw enable"
    elif command -v firewall-cmd >/dev/null 2>&1; then
        # Firewalld configuration
        firewall-cmd --permanent --add-port=5060/udp
        firewall-cmd --permanent --add-port=5060/tcp
        firewall-cmd --permanent --add-port=5061/tcp
        firewall-cmd --permanent --add-port=5061/udp
        firewall-cmd --permanent --add-port=10000-20000/udp
        firewall-cmd --reload
        
        log_success "Firewalld rules configured"
    else
        log_warning "No supported firewall found. Please configure manually:"
        log_info "  SIP: 5060/tcp, 5060/udp, 5061/tcp, 5061/udp"
        log_info "  RTP: 10000-20000/udp"
    fi
}

# Configure log rotation
setup_log_rotation() {
    log_info "Setting up log rotation..."
    
    cat > /etc/logrotate.d/redfire-switch << 'EOF'
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

    log_success "Log rotation configured"
}

# Create systemd override directories
setup_systemd_overrides() {
    log_info "Setting up systemd overrides..."
    
    # Create override directories
    mkdir -p /etc/systemd/system/redfire-switch.service.d
    mkdir -p /etc/systemd/system/redfire-switch-bgp.service.d
    mkdir -p /etc/systemd/system/redfire-switch-web.service.d
    
    # Environment file for sensitive configuration
    cat > /etc/redfire-switch/environment << 'EOF'
# Environment variables for Redfire Switch
# This file contains sensitive configuration - keep secure!

# Rust configuration
RUST_LOG=info
RUST_BACKTRACE=1

# Service-specific settings
# CUSTOM_CONFIG_PATH=/etc/redfire-switch/custom.toml
# EXTERNAL_IP=auto-detect
EOF

    chown root:redfire /etc/redfire-switch/environment
    chmod 640 /etc/redfire-switch/environment
    
    log_success "Systemd overrides configured"
}

# Configure monitoring (basic setup)
setup_monitoring() {
    log_info "Setting up basic monitoring..."
    
    # Create monitoring scripts directory
    mkdir -p /usr/local/bin/redfire-switch
    
    # Health check script
    cat > /usr/local/bin/redfire-switch/health-check << 'EOF'
#!/bin/bash
# Basic health check for Redfire Switch

# Check if service is running
if ! systemctl is-active --quiet redfire-switch; then
    echo "CRITICAL: Redfire Switch service is not running"
    exit 2
fi

# Check if ports are listening
if ! ss -tlun | grep -q ":5060 "; then
    echo "CRITICAL: SIP port 5060 is not listening"
    exit 2
fi

# Check database connectivity (if configured)
if [ -f /etc/redfire-switch/db-credentials ]; then
    source /etc/redfire-switch/db-credentials
    if ! PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -U "$DB_USER" -d "$DB_NAME" -c "SELECT 1;" >/dev/null 2>&1; then
        echo "WARNING: Database connectivity issue"
        exit 1
    fi
fi

echo "OK: All checks passed"
exit 0
EOF

    chmod +x /usr/local/bin/redfire-switch/health-check
    
    # Create cron job for health monitoring
    cat > /etc/cron.d/redfire-switch-health << 'EOF'
# Health check for Redfire Switch (every 5 minutes)
*/5 * * * * root /usr/local/bin/redfire-switch/health-check >/dev/null 2>&1 || logger "Redfire Switch health check failed"
EOF

    log_success "Basic monitoring configured"
}

# Generate API keys and secrets
setup_api_security() {
    log_info "Generating API keys and secrets..."
    
    # Generate API key
    API_KEY=$(openssl rand -hex 32)
    
    # Generate encryption keys for session storage
    ENCRYPTION_KEY=$(openssl rand -hex 32)
    
    # Update configuration
    if [ -f /etc/redfire-switch/config.toml ]; then
        sed -i "s/api_key = \"your-api-key-here\"/api_key = \"$API_KEY\"/g" /etc/redfire-switch/config.toml
    fi
    
    # Save keys securely
    cat >> /etc/redfire-switch/db-credentials << EOF

# API and encryption keys
API_KEY=$API_KEY
ENCRYPTION_KEY=$ENCRYPTION_KEY
EOF

    log_success "API keys generated and saved"
}

# Optimize system for SIP workloads
optimize_system() {
    log_info "Optimizing system for SIP workloads..."
    
    # Kernel parameter optimizations
    cat > /etc/sysctl.d/99-redfire-switch.conf << 'EOF'
# Network optimizations for SIP workloads
net.core.rmem_max = 134217728
net.core.wmem_max = 134217728
net.ipv4.tcp_rmem = 4096 87380 134217728
net.ipv4.tcp_wmem = 4096 65536 134217728
net.core.netdev_max_backlog = 5000
net.ipv4.tcp_congestion_control = bbr

# File descriptor limits
fs.file-max = 1048576

# Memory management
vm.swappiness = 10
vm.dirty_ratio = 15
vm.dirty_background_ratio = 5
EOF

    # Apply sysctl settings
    sysctl -p /etc/sysctl.d/99-redfire-switch.conf
    
    log_success "System optimized for SIP workloads"
}

# Main setup function
main() {
    echo "=== Redfire Switch Post-Installation Setup ==="
    echo ""
    
    check_root
    
    # Run setup functions
    setup_database
    init_database_schema
    setup_redis
    setup_ssl_certificates
    setup_system_limits
    setup_firewall
    setup_log_rotation
    setup_systemd_overrides
    setup_monitoring
    setup_api_security
    optimize_system
    
    # Reload systemd and restart services
    systemctl daemon-reload
    
    # Start and enable services
    log_info "Starting Redfire Switch services..."
    
    if systemctl start redfire-switch; then
        systemctl enable redfire-switch
        log_success "Redfire Switch started and enabled"
    else
        log_error "Failed to start Redfire Switch. Check logs:"
        log_info "  journalctl -u redfire-switch -f"
    fi
    
    # Display summary
    cat << EOF

${GREEN}=== Setup Complete ===${NC}

${BLUE}Services Status:${NC}
$(systemctl is-active redfire-switch && echo "✓ Redfire Switch: Running" || echo "✗ Redfire Switch: Failed")
$(systemctl is-active postgresql && echo "✓ PostgreSQL: Running" || echo "✗ PostgreSQL: Not running")
$(systemctl is-active redis-server && echo "✓ Redis: Running" || echo "✗ Redis: Not running")

${BLUE}Configuration Files:${NC}
  Main config:      /etc/redfire-switch/config.toml
  BGP Anycast:      /etc/redfire-switch/bgp-anycast.toml
  Credentials:      /etc/redfire-switch/db-credentials
  Environment:      /etc/redfire-switch/environment

${BLUE}Service Management:${NC}
  Status:           systemctl status redfire-switch
  Logs:             journalctl -u redfire-switch -f
  Restart:          systemctl restart redfire-switch
  Health check:     /usr/local/bin/redfire-switch/health-check

${BLUE}Security Notes:${NC}
  - Database and Redis passwords have been generated
  - Self-signed SSL certificates created (replace for production)
  - API keys generated and saved securely
  - System limits optimized for SIP workloads
  - Firewall rules added (enable with 'ufw enable')

${BLUE}Next Steps:${NC}
1. Review and customize /etc/redfire-switch/config.toml
2. Configure SIP trunks and routing rules
3. Set up proper SSL certificates for production
4. Configure monitoring and alerting
5. Test SIP connectivity and call routing

${YELLOW}Important:${NC} Keep /etc/redfire-switch/db-credentials secure!

EOF
}

# Handle command line options
case "${1:-}" in
    --help|-h)
        echo "Redfire Switch Post-Installation Setup"
        echo ""
        echo "Usage: $0 [OPTIONS]"
        echo ""
        echo "Options:"
        echo "  --help, -h          Show this help message"
        echo "  --database-only     Setup database only"
        echo "  --redis-only        Setup Redis only"
        echo "  --ssl-only          Setup SSL certificates only"
        echo "  --no-database       Skip database setup"
        echo "  --no-redis          Skip Redis setup"
        echo ""
        exit 0
        ;;
    --database-only)
        check_root
        setup_database
        init_database_schema
        ;;
    --redis-only)
        check_root
        setup_redis
        ;;
    --ssl-only)
        check_root
        setup_ssl_certificates
        ;;
    *)
        main
        ;;
esac