# 🚀 B2BUA Production Deployment Guide

This guide provides comprehensive instructions for deploying the Class 4 carrier-grade B2BUA in production environments.

## 📋 Deployment Options

### 1. **SIP-I Carrier Interconnection** (Class 4)
**Use Case**: PSTN/SS7 interconnection with legacy carriers
**Binary**: `sipi-b2bua-test`
**Features**: ISUP encapsulation, CIC management, RFC 3398 compliance

### 2. **STIR/SHAKEN Identity Verification** (US Carriers)
**Use Case**: FCC-compliant identity verification for US carriers
**Binary**: `stir-shaken-b2bua-test`
**Features**: PASSporT tokens, certificate validation, attestation levels

### 3. **Enhanced Multi-RFC Compliance** (General Purpose)
**Use Case**: Standards-compliant SIP processing
**Binary**: `improved-b2bua-test`
**Features**: Core SIP, error handling, header manipulation

## 🔧 Configuration Options

### **Environment Variables**

#### **SIP-I Configuration**
```bash
# Enable SIP-I for PSTN interconnection
export ENABLE_SIP_I=true
export ENABLE_SIP_T=true

# ISUP Parameters
export ISUP_VARIANT=ITU_T  # or ANSI, ETSI
export ORIGINATING_POINT_CODE=0x001234
export DESTINATION_POINT_CODE=0x005678
export CIC_RANGE_START=1
export CIC_RANGE_END=1000

# Network Configuration
export SIP_LISTEN_PORT=5064
export TERMINATION_HOST=192.168.1.100
export TERMINATION_PORT=5070
```

#### **STIR/SHAKEN Configuration**
```bash
# Enable STIR/SHAKEN authentication
export ENABLE_STIR_SHAKEN=true
export CERTIFICATE_PATH=/etc/ssl/certs/carrier.pem
export PRIVATE_KEY_PATH=/etc/ssl/private/carrier.key
export ATTESTATION_LEVEL=A  # A, B, or C
export TOKEN_EXPIRY_SECONDS=300
```

#### **General Configuration**
```bash
# Logging Level
export RUST_LOG=info  # trace, debug, info, warn, error

# Performance Tuning
export TOKIO_WORKER_THREADS=8
export MAX_CONCURRENT_CALLS=10000
export SOCKET_BUFFER_SIZE=65536
```

### **Configuration Files**

#### **SIP-I Configuration** (`sipi-config.toml`)
```toml
[sipi]
enabled = true
sipt_enabled = true
isup_variant = "ITU-T"
originating_point_code = 0x001234
destination_point_code = 0x005678
cic_range_start = 1
cic_range_end = 1000
validate_isup = true
multipart_support = true
max_isup_size = 4096

[network]
listen_address = "0.0.0.0:5064"
termination_host = "192.168.1.100"
termination_port = 5070
```

#### **STIR/SHAKEN Configuration** (`stir-shaken-config.toml`)
```toml
[stir_shaken]
enabled = true
certificate_path = "/etc/ssl/certs/carrier.pem"
private_key_path = "/etc/ssl/private/carrier.key"
attestation_level = "A"
token_expiry_seconds = 300
validate_certificates = true

[identity]
authority = "https://carrier.example.com"
service_provider_code = "SP001"
originating_tn_validation = true
```

## 🐳 Docker Deployment

### **Dockerfile**
```dockerfile
FROM rust:1.75-slim as builder

WORKDIR /app
COPY . .
RUN cargo build --release --bin sipi-b2bua-test

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    openssl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/sipi-b2bua-test /usr/local/bin/
COPY config/ /etc/redfire-switch/

EXPOSE 5064/udp
CMD ["sipi-b2bua-test"]
```

### **Docker Compose**
```yaml
version: '3.8'

services:
  sipi-b2bua:
    build: .
    ports:
      - "5064:5064/udp"
    environment:
      - ENABLE_SIP_I=true
      - ENABLE_SIP_T=true
      - RUST_LOG=info
    volumes:
      - ./config:/etc/redfire-switch
      - ./ssl:/etc/ssl/private
    restart: unless-stopped
    
  monitoring:
    image: prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
      
  logging:
    image: grafana/loki:latest
    ports:
      - "3100:3100"
```

## ☸️ Kubernetes Deployment

### **Kubernetes Manifest**
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: sipi-b2bua
  labels:
    app: sipi-b2bua
spec:
  replicas: 3
  selector:
    matchLabels:
      app: sipi-b2bua
  template:
    metadata:
      labels:
        app: sipi-b2bua
    spec:
      containers:
      - name: sipi-b2bua
        image: redfire-switch:latest
        ports:
        - containerPort: 5064
          protocol: UDP
        env:
        - name: ENABLE_SIP_I
          value: "true"
        - name: RUST_LOG
          value: "info"
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        volumeMounts:
        - name: config
          mountPath: /etc/redfire-switch
        - name: ssl-certs
          mountPath: /etc/ssl/private
      volumes:
      - name: config
        configMap:
          name: sipi-config
      - name: ssl-certs
        secret:
          secretName: carrier-certificates
---
apiVersion: v1
kind: Service
metadata:
  name: sipi-b2bua-service
spec:
  selector:
    app: sipi-b2bua
  ports:
  - port: 5064
    targetPort: 5064
    protocol: UDP
  type: LoadBalancer
```

## 📊 Monitoring & Observability

### **Prometheus Metrics**
```yaml
# prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'sipi-b2bua'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: /metrics
```

### **Grafana Dashboards**

#### **SIP-I Metrics Dashboard**
- **Call Volume**: Calls per second, concurrent calls
- **ISUP Statistics**: IAM/ACM/ANM/REL message rates
- **CIC Utilization**: Circuit usage percentage
- **Error Rates**: Failed calls, ISUP parsing errors
- **Latency**: Call setup time, response times

#### **STIR/SHAKEN Dashboard**
- **Identity Verification**: Verification success rate
- **Attestation Levels**: A/B/C distribution
- **Certificate Status**: Validity, expiration warnings
- **Token Generation**: Success/failure rates

### **Logging Configuration**
```toml
# logging.toml
[logging]
level = "info"
format = "json"
output = "stdout"

[tracing]
jaeger_endpoint = "http://jaeger:14268/api/traces"
service_name = "sipi-b2bua"
```

## 🔒 Security Considerations

### **Network Security**
```bash
# Firewall Rules (iptables)
# Allow SIP traffic
iptables -A INPUT -p udp --dport 5064 -j ACCEPT

# Allow management interface
iptables -A INPUT -p tcp --dport 8080 -s 10.0.0.0/8 -j ACCEPT

# Block all other traffic
iptables -A INPUT -j DROP
```

### **Certificate Management**
```bash
# Generate carrier certificate
openssl genrsa -out carrier.key 2048
openssl req -new -key carrier.key -out carrier.csr
openssl x509 -req -in carrier.csr -signkey carrier.key -out carrier.pem

# Set proper permissions
chmod 600 /etc/ssl/private/carrier.key
chmod 644 /etc/ssl/certs/carrier.pem
```

### **Access Control**
```yaml
# RBAC for Kubernetes
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: sipi-b2bua-operator
rules:
- apiGroups: [""]
  resources: ["pods", "services", "configmaps"]
  verbs: ["get", "list", "create", "update", "patch"]
```

## 🚀 High Availability Setup

### **Load Balancer Configuration**
```nginx
# nginx.conf
upstream sipi_b2bua {
    least_conn;
    server 10.0.1.10:5064;
    server 10.0.1.11:5064;
    server 10.0.1.12:5064;
}

server {
    listen 5064 udp;
    proxy_pass sipi_b2bua;
    proxy_timeout 1s;
    proxy_responses 1;
}
```

### **Database Clustering** (for call state)
```yaml
# Redis Cluster for call state
apiVersion: v1
kind: ConfigMap
metadata:
  name: redis-config
data:
  redis.conf: |
    cluster-enabled yes
    cluster-config-file nodes.conf
    cluster-node-timeout 5000
    appendonly yes
```

## 📈 Performance Tuning

### **System Optimization**
```bash
# Increase UDP buffer sizes
echo 'net.core.rmem_max = 134217728' >> /etc/sysctl.conf
echo 'net.core.wmem_max = 134217728' >> /etc/sysctl.conf

# Increase file descriptor limits
echo '* soft nofile 65536' >> /etc/security/limits.conf
echo '* hard nofile 65536' >> /etc/security/limits.conf

# Apply changes
sysctl -p
```

### **Application Tuning**
```bash
# Environment variables for performance
export TOKIO_WORKER_THREADS=16
export RUST_MIN_STACK=8388608
export MALLOC_CONF="background_thread:true,narenas:4"
```

## 🧪 Testing & Validation

### **Pre-deployment Testing**
```bash
# Run compliance tests
./target/debug/enhanced-rfc-compliance-test

# Run SIP-I validation
./target/debug/sipi-compliance-test

# Performance testing with SIPp
sipp -sn uac -r 100 -l 1000 192.168.1.100:5064
```

### **Health Checks**
```bash
#!/bin/bash
# health-check.sh
curl -f http://localhost:8080/health || exit 1
netstat -ul | grep :5064 || exit 1
```

### **Monitoring Scripts**
```bash
#!/bin/bash
# monitor-cic-usage.sh
while true; do
    curl -s http://localhost:8080/metrics | grep cic_utilization
    sleep 30
done
```

## 📱 Carrier Integration Examples

### **AT&T Integration**
```toml
[carrier.att]
name = "AT&T"
point_code = 0x00FF01
isup_variant = "ANSI"
stir_shaken_required = true
attestation_level = "A"
```

### **Verizon Integration**
```toml
[carrier.verizon]
name = "Verizon"
point_code = 0x00FF02
isup_variant = "ANSI"
stir_shaken_required = true
attestation_level = "A"
```

### **International Carrier**
```toml
[carrier.international]
name = "Deutsche Telekom"
point_code = 0x45DE01
isup_variant = "ITU-T"
stir_shaken_required = false
```

## 🔄 Maintenance & Operations

### **Rolling Updates**
```bash
# Zero-downtime deployment
kubectl set image deployment/sipi-b2bua sipi-b2bua=redfire-switch:v2.0.0
kubectl rollout status deployment/sipi-b2bua
```

### **Backup Procedures**
```bash
# Backup configuration
kubectl get configmap sipi-config -o yaml > sipi-config-backup.yaml

# Backup certificates
tar -czf certificates-backup.tar.gz /etc/ssl/private/
```

### **Log Rotation**
```bash
# logrotate configuration
/var/log/sipi-b2bua/*.log {
    daily
    rotate 30
    compress
    delaycompress
    missingok
    notifempty
    create 644 sipi sipi
}
```

## 📞 Support & Escalation

### **Monitoring Alerts**
```yaml
# Alert Rules (Prometheus)
groups:
- name: sipi-b2bua
  rules:
  - alert: HighCallFailureRate
    expr: call_failure_rate > 0.05
    for: 5m
    annotations:
      summary: "High call failure rate detected"
      
  - alert: CICExhaustion
    expr: cic_utilization > 0.9
    for: 2m
    annotations:
      summary: "CIC pool nearly exhausted"
```

### **Troubleshooting Commands**
```bash
# Check call statistics
curl http://localhost:8080/stats

# View active calls
curl http://localhost:8080/calls

# Check ISUP message stats
curl http://localhost:8080/isup-stats

# Validate STIR/SHAKEN certificates
curl http://localhost:8080/certificates
```

## 🎯 Production Readiness Checklist

### **Pre-deployment**
- [ ] Configuration validated
- [ ] Certificates installed and verified
- [ ] Compliance tests passed (100%)
- [ ] Performance testing completed
- [ ] Security audit conducted
- [ ] Monitoring configured
- [ ] Backup procedures tested

### **Post-deployment**
- [ ] Health checks passing
- [ ] Metrics collection active
- [ ] Alerting configured
- [ ] Log aggregation working
- [ ] Carrier interconnection tested
- [ ] Failover procedures verified
- [ ] Documentation updated

---

**🚀 Your Class 4 carrier-grade B2BUA is now ready for production deployment!**

This comprehensive guide ensures enterprise-grade deployment with high availability, monitoring, and carrier compliance.