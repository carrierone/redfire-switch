-- RedFire Switch Database Schema (SQLite Version)
-- Production-ready schema for Class 4 switching operations

-- Call Detail Records table
CREATE TABLE call_detail_records (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    call_id TEXT NOT NULL,
    session_id TEXT,
    from_number TEXT NOT NULL,
    to_number TEXT NOT NULL,
    from_ip TEXT,
    to_ip TEXT,
    start_time DATETIME NOT NULL,
    end_time DATETIME,
    duration_seconds INTEGER DEFAULT 0,
    disposition TEXT NOT NULL,
    hangup_cause INTEGER,
    trunk_id TEXT,
    route_id TEXT,
    codec_in TEXT,
    codec_out TEXT,
    recording_enabled INTEGER DEFAULT 0,
    cost REAL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for CDR table
CREATE INDEX idx_cdr_call_id ON call_detail_records(call_id);
CREATE INDEX idx_cdr_start_time ON call_detail_records(start_time);
CREATE INDEX idx_cdr_from_number ON call_detail_records(from_number);
CREATE INDEX idx_cdr_to_number ON call_detail_records(to_number);
CREATE INDEX idx_cdr_trunk_id ON call_detail_records(trunk_id);
CREATE INDEX idx_cdr_disposition ON call_detail_records(disposition);

-- SIP Profiles table
CREATE TABLE sip_profiles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    bind_ip TEXT NOT NULL,
    port INTEGER NOT NULL,
    transport TEXT NOT NULL DEFAULT 'udp',
    max_sessions INTEGER DEFAULT 10000,
    session_timer INTEGER DEFAULT 1800,
    enable_registration INTEGER DEFAULT 0,
    auth_calls INTEGER DEFAULT 0,
    transit_mode INTEGER DEFAULT 1,
    codec_negotiation TEXT DEFAULT 'transparent',
    dtmf_relay TEXT DEFAULT 'rfc2833',
    record_route INTEGER DEFAULT 1,
    proxy_media INTEGER DEFAULT 0,
    sip_i_support INTEGER DEFAULT 1,
    isup_interworking INTEGER DEFAULT 1,
    ss7_gateway_support INTEGER DEFAULT 1,
    cause_code_mapping TEXT DEFAULT 'itu_t',
    release_source_header INTEGER DEFAULT 1,
    enabled INTEGER DEFAULT 1,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Carrier Interconnects table
CREATE TABLE carrier_interconnects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    carrier_id TEXT NOT NULL,
    carrier_name TEXT NOT NULL,
    direction TEXT NOT NULL CHECK (direction IN ('termination', 'origination', 'bidirectional')),
    remote_ip TEXT NOT NULL,
    remote_port INTEGER NOT NULL,
    transport TEXT NOT NULL DEFAULT 'udp',
    lcr_group TEXT,
    quality_score INTEGER DEFAULT 50 CHECK (quality_score >= 0 AND quality_score <= 100),
    capacity_limit INTEGER DEFAULT 1000,
    codec_preference TEXT, -- JSON array as text
    sip_i_support INTEGER DEFAULT 1,
    isup_interworking INTEGER DEFAULT 1,
    ss7_protocol TEXT DEFAULT 'itu_t',
    circuit_group_support INTEGER DEFAULT 1,
    cic_range TEXT,
    authentication_type TEXT DEFAULT 'ip_auth',
    authentication_data TEXT, -- JSON as text
    enabled INTEGER DEFAULT 1,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Trunks table
CREATE TABLE trunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    tech_prefix TEXT,
    sip_profile_id INTEGER REFERENCES sip_profiles(id),
    carrier_interconnect_id INTEGER REFERENCES carrier_interconnects(id),
    vendor_customer_id INTEGER,
    trunk_type TEXT NOT NULL CHECK (trunk_type IN ('termination', 'origination', 'bidirectional')),
    digit_manipulation TEXT, -- JSON as text
    call_limits TEXT, -- JSON as text
    allowed_codecs TEXT, -- JSON array as text
    stir_shaken_config TEXT, -- JSON as text
    lcr_group TEXT,
    priority INTEGER DEFAULT 50,
    enabled INTEGER DEFAULT 1,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Vendors and Customers table
CREATE TABLE vendors_customers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    type TEXT NOT NULL CHECK (type IN ('vendor', 'customer', 'partner')),
    contact_info TEXT, -- JSON as text
    billing_info TEXT, -- JSON as text
    technical_contact TEXT,
    billing_contact TEXT,
    emergency_contact TEXT,
    enabled INTEGER DEFAULT 1,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- LCR Routes table
CREATE TABLE lcr_routes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    route_group TEXT NOT NULL,
    prefix TEXT NOT NULL,
    description TEXT,
    trunk_id INTEGER REFERENCES trunks(id),
    priority INTEGER DEFAULT 50,
    cost_per_minute REAL,
    effective_date DATETIME DEFAULT CURRENT_TIMESTAMP,
    expiry_date DATETIME,
    quality_score INTEGER DEFAULT 50,
    max_call_duration INTEGER DEFAULT 7200,
    enabled INTEGER DEFAULT 1,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for LCR routes
CREATE INDEX idx_lcr_routes_prefix ON lcr_routes(prefix);
CREATE INDEX idx_lcr_routes_group ON lcr_routes(route_group);
CREATE INDEX idx_lcr_routes_priority ON lcr_routes(priority);
CREATE INDEX idx_lcr_routes_cost ON lcr_routes(cost_per_minute);

-- Active Sessions table (for call tracking)
CREATE TABLE active_sessions (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    call_id TEXT NOT NULL,
    session_id TEXT,
    from_number TEXT NOT NULL,
    to_number TEXT NOT NULL,
    from_ip TEXT,
    to_ip TEXT,
    trunk_id INTEGER REFERENCES trunks(id),
    route_id INTEGER REFERENCES lcr_routes(id),
    start_time DATETIME NOT NULL,
    last_activity DATETIME NOT NULL,
    state TEXT NOT NULL,
    codec_in TEXT,
    codec_out TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for active sessions
CREATE INDEX idx_active_sessions_call_id ON active_sessions(call_id);
CREATE INDEX idx_active_sessions_state ON active_sessions(state);
CREATE INDEX idx_active_sessions_trunk_id ON active_sessions(trunk_id);

-- Security Events table
CREATE TABLE security_events (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    event_type TEXT NOT NULL,
    source_ip TEXT NOT NULL,
    severity TEXT NOT NULL,
    description TEXT,
    details TEXT, -- JSON as text
    action_taken TEXT,
    resolved INTEGER DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    resolved_at DATETIME
);

-- Create indexes for security events
CREATE INDEX idx_security_events_type ON security_events(event_type);
CREATE INDEX idx_security_events_source_ip ON security_events(source_ip);
CREATE INDEX idx_security_events_severity ON security_events(severity);
CREATE INDEX idx_security_events_created_at ON security_events(created_at);

-- Blacklist table
CREATE TABLE ip_blacklist (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ip_address TEXT NOT NULL UNIQUE,
    reason TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    expires_at DATETIME,
    violation_count INTEGER DEFAULT 1,
    last_violation DATETIME DEFAULT CURRENT_TIMESTAMP,
    automatic INTEGER DEFAULT 1
);

-- Create indexes for blacklist
CREATE INDEX idx_blacklist_ip ON ip_blacklist(ip_address);
CREATE INDEX idx_blacklist_expires ON ip_blacklist(expires_at);

-- System Configuration table
CREATE TABLE system_config (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    config_key TEXT NOT NULL UNIQUE,
    config_value TEXT NOT NULL, -- JSON as text
    description TEXT,
    config_type TEXT NOT NULL,
    editable INTEGER DEFAULT 1,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Health Check Results table
CREATE TABLE health_check_results (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    component TEXT NOT NULL,
    status TEXT NOT NULL,
    response_time_ms INTEGER,
    details TEXT, -- JSON as text
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for health checks
CREATE INDEX idx_health_check_component ON health_check_results(component);
CREATE INDEX idx_health_check_status ON health_check_results(status);
CREATE INDEX idx_health_check_created_at ON health_check_results(created_at);

-- Monitoring Metrics table
CREATE TABLE monitoring_metrics (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    metric_name TEXT NOT NULL,
    metric_value REAL NOT NULL,
    metric_type TEXT NOT NULL,
    labels TEXT, -- JSON as text
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for monitoring metrics
CREATE INDEX idx_monitoring_metrics_name ON monitoring_metrics(metric_name);
CREATE INDEX idx_monitoring_metrics_timestamp ON monitoring_metrics(timestamp);

-- STIR/SHAKEN Certificates table
CREATE TABLE stir_shaken_certificates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    certificate_name TEXT NOT NULL UNIQUE,
    certificate_data TEXT NOT NULL,
    private_key_data TEXT,
    issuer TEXT,
    subject TEXT,
    valid_from DATETIME,
    valid_until DATETIME,
    fingerprint TEXT,
    enabled INTEGER DEFAULT 1,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Audit Log table
CREATE TABLE audit_log (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    user_id TEXT,
    action TEXT NOT NULL,
    resource_type TEXT,
    resource_id TEXT,
    details TEXT, -- JSON as text
    ip_address TEXT,
    user_agent TEXT,
    success INTEGER NOT NULL,
    error_message TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for audit log
CREATE INDEX idx_audit_log_user_id ON audit_log(user_id);
CREATE INDEX idx_audit_log_action ON audit_log(action);
CREATE INDEX idx_audit_log_resource_type ON audit_log(resource_type);
CREATE INDEX idx_audit_log_created_at ON audit_log(created_at);

-- Insert default system configuration
INSERT INTO system_config (config_key, config_value, description, config_type) VALUES
('system.name', '"RedFire Switch"', 'System name', 'string'),
('system.version', '"1.0.0"', 'System version', 'string'),
('sip.max_concurrent_calls', '10000', 'Maximum concurrent calls', 'integer'),
('sip.default_session_timer', '1800', 'Default session timer in seconds', 'integer'),
('security.rate_limit_enabled', 'true', 'Enable rate limiting', 'boolean'),
('security.max_requests_per_minute', '60', 'Maximum requests per minute per IP', 'integer'),
('monitoring.health_check_interval', '30', 'Health check interval in seconds', 'integer'),
('cdr.retention_days', '365', 'CDR retention period in days', 'integer'),
('lcr.cache_ttl_seconds', '300', 'LCR cache TTL in seconds', 'integer');