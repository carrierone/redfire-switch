#!/bin/bash
# Production deployment script for Redfire Switch
# Usage: ./deploy.sh [environment]

set -e

ENVIRONMENT=${1:-production}
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

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

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed"
        exit 1
    fi
    
    if ! command -v docker-compose &> /dev/null; then
        log_error "Docker Compose is not installed"
        exit 1
    fi
    
    # Check if running as root or with Docker permissions
    if ! docker info &> /dev/null; then
        log_error "Cannot connect to Docker daemon. Are you in the docker group?"
        exit 1
    fi
    
    log_success "Prerequisites check passed"
}

# Build application
build_application() {
    log_info "Building Redfire Switch application..."
    
    cd "${PROJECT_ROOT}"
    
    # Run tests first
    log_info "Running tests..."
    export CARGO_TARGET_DIR=/tmp/redfire-build
    if ! cargo test --workspace --lib; then
        log_error "Tests failed. Deployment aborted."
        exit 1
    fi
    
    # Build the Docker image
    log_info "Building production Docker image..."
    if ! docker build -f Dockerfile.prod -t redfire-switch:latest .; then
        log_error "Docker build failed"
        exit 1
    fi
    
    log_success "Application build completed"
}

# Setup configuration
setup_configuration() {
    log_info "Setting up configuration for environment: ${ENVIRONMENT}"
    
    DEPLOY_DIR="${SCRIPT_DIR}"
    
    # Create necessary directories
    mkdir -p "${DEPLOY_DIR}/config"
    mkdir -p "${DEPLOY_DIR}/ssl"
    mkdir -p "${DEPLOY_DIR}/logs"
    mkdir -p "${DEPLOY_DIR}/secrets"
    mkdir -p "${DEPLOY_DIR}/sql"
    mkdir -p "${DEPLOY_DIR}/prometheus"
    mkdir -p "${DEPLOY_DIR}/grafana/provisioning/dashboards"
    mkdir -p "${DEPLOY_DIR}/grafana/provisioning/datasources"
    mkdir -p "${DEPLOY_DIR}/grafana/dashboards"
    mkdir -p "${DEPLOY_DIR}/nginx"
    
    # Copy configuration files
    if [ -f "${PROJECT_ROOT}/config-production-example.json" ]; then
        cp "${PROJECT_ROOT}/config-production-example.json" "${DEPLOY_DIR}/config/production.json"
        
        # Create replica configuration
        cp "${PROJECT_ROOT}/config-production-example.json" "${DEPLOY_DIR}/config/production-replica.json"
        
        # Modify replica config to use different ports
        sed -i 's/5060/5062/g' "${DEPLOY_DIR}/config/production-replica.json"
        sed -i 's/5061/5063/g' "${DEPLOY_DIR}/config/production-replica.json"
    else
        log_warning "Production configuration template not found, creating default"
        echo '{"sip_profiles": [{"name": "default", "bind_ip": "0.0.0.0", "port": 5060, "protocol": "Udp", "allowed_ips": ["0.0.0.0/0"]}]}' > "${DEPLOY_DIR}/config/production.json"
    fi
    
    # Generate secrets if they don't exist
    if [ ! -f "${DEPLOY_DIR}/secrets/postgres_password.txt" ]; then
        openssl rand -base64 32 > "${DEPLOY_DIR}/secrets/postgres_password.txt"
        chmod 600 "${DEPLOY_DIR}/secrets/postgres_password.txt"
        log_info "Generated PostgreSQL password"
    fi
    
    if [ ! -f "${DEPLOY_DIR}/secrets/grafana_password.txt" ]; then
        openssl rand -base64 32 > "${DEPLOY_DIR}/secrets/grafana_password.txt"
        chmod 600 "${DEPLOY_DIR}/secrets/grafana_password.txt"
        log_info "Generated Grafana password"
    fi
    
    # Create database init script
    cat > "${DEPLOY_DIR}/sql/init.sql" << 'EOF'
-- Redfire Switch Database Schema
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- CDR table for call detail records
CREATE TABLE IF NOT EXISTS cdr (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    call_id VARCHAR(128) NOT NULL,
    calling_number VARCHAR(32),
    called_number VARCHAR(32),
    start_time TIMESTAMP WITH TIME ZONE NOT NULL,
    end_time TIMESTAMP WITH TIME ZONE,
    duration_seconds INTEGER,
    disconnect_reason VARCHAR(64),
    sip_response_code INTEGER,
    billing_amount DECIMAL(10,4),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_cdr_call_id ON cdr(call_id);
CREATE INDEX idx_cdr_start_time ON cdr(start_time);

-- LCR routing table
CREATE TABLE IF NOT EXISTS lcr_routes (
    id SERIAL PRIMARY KEY,
    prefix VARCHAR(32) NOT NULL,
    gateway VARCHAR(128) NOT NULL,
    priority INTEGER NOT NULL DEFAULT 1,
    rate DECIMAL(8,4) NOT NULL DEFAULT 0.0,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_lcr_prefix ON lcr_routes(prefix);
CREATE INDEX idx_lcr_priority ON lcr_routes(priority);

-- Security events table
CREATE TABLE IF NOT EXISTS security_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_type VARCHAR(64) NOT NULL,
    source_ip INET NOT NULL,
    severity VARCHAR(16) NOT NULL,
    description TEXT,
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_security_events_type ON security_events(event_type);
CREATE INDEX idx_security_events_source_ip ON security_events(source_ip);
CREATE INDEX idx_security_events_created_at ON security_events(created_at);
EOF
    
    # Create Prometheus configuration
    cat > "${DEPLOY_DIR}/prometheus/prometheus.yml" << 'EOF'
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'redfire-switch'
    static_configs:
      - targets: ['redfire-switch:8081']
    metrics_path: /metrics
    scrape_interval: 10s
    
  - job_name: 'redfire-switch-replica'
    static_configs:
      - targets: ['redfire-switch-replica:8081']
    metrics_path: /metrics
    scrape_interval: 10s

  - job_name: 'postgres'
    static_configs:
      - targets: ['postgres:5432']
    scrape_interval: 30s

  - job_name: 'node-exporter'
    static_configs:
      - targets: ['host.docker.internal:9100']
    scrape_interval: 30s
EOF
    
    # Create nginx configuration
    cat > "${DEPLOY_DIR}/nginx/nginx.conf" << 'EOF'
events {
    worker_connections 1024;
}

http {
    upstream redfire_api {
        least_conn;
        server redfire-switch:8080;
        server redfire-switch-replica:8080 backup;
    }
    
    server {
        listen 80;
        server_name _;
        
        # Health check endpoint
        location /health {
            access_log off;
            return 200 "healthy\n";
            add_header Content-Type text/plain;
        }
        
        # API proxy
        location /api/ {
            proxy_pass http://redfire_api/;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
        }
        
        # Grafana proxy
        location /grafana/ {
            proxy_pass http://grafana:3000/;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
        }
    }
}
EOF
    
    log_success "Configuration setup completed"
}

# Deploy application
deploy_application() {
    log_info "Deploying Redfire Switch to ${ENVIRONMENT}..."
    
    cd "${SCRIPT_DIR}"
    
    # Pull required images
    log_info "Pulling required Docker images..."
    docker-compose pull postgres prometheus grafana nginx
    
    # Start the deployment
    log_info "Starting services..."
    if ! docker-compose up -d; then
        log_error "Deployment failed"
        exit 1
    fi
    
    # Wait for services to be healthy
    log_info "Waiting for services to become healthy..."
    sleep 30
    
    # Check service health
    for service in redfire-switch redfire-switch-replica postgres prometheus grafana; do
        log_info "Checking health of ${service}..."
        if docker-compose ps ${service} | grep -q "Up (healthy)"; then
            log_success "${service} is healthy"
        else
            log_warning "${service} may not be fully ready yet"
        fi
    done
    
    log_success "Deployment completed successfully!"
}

# Show deployment status
show_status() {
    log_info "Deployment Status:"
    echo
    docker-compose ps
    echo
    log_info "Service URLs:"
    echo "  - Redfire Switch API: http://localhost/api/"
    echo "  - Grafana Dashboard: http://localhost/grafana/"
    echo "  - Prometheus: http://localhost:9090"
    echo "  - Health Check: http://localhost/health"
    echo
    log_info "SIP Endpoints:"
    echo "  - Primary SIP: localhost:5060 (UDP)"
    echo "  - Primary SIP TLS: localhost:5061 (TCP)"  
    echo "  - Replica SIP: localhost:5062 (UDP)"
    echo "  - Replica SIP TLS: localhost:5063 (TCP)"
}

# Rollback deployment
rollback() {
    log_warning "Rolling back deployment..."
    cd "${SCRIPT_DIR}"
    docker-compose down
    log_success "Rollback completed"
}

# Main execution
main() {
    echo "🔥 Redfire Switch Production Deployment"
    echo "======================================="
    echo
    
    case "${1:-deploy}" in
        "deploy")
            check_prerequisites
            build_application
            setup_configuration
            deploy_application
            show_status
            ;;
        "status")
            show_status
            ;;
        "rollback")
            rollback
            ;;
        "logs")
            cd "${SCRIPT_DIR}"
            docker-compose logs -f redfire-switch
            ;;
        *)
            echo "Usage: $0 [deploy|status|rollback|logs]"
            echo
            echo "Commands:"
            echo "  deploy   - Full deployment (default)"
            echo "  status   - Show deployment status"
            echo "  rollback - Rollback deployment"
            echo "  logs     - Show application logs"
            exit 1
            ;;
    esac
}

# Execute main function
main "$@"