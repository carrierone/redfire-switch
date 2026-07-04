-- RedFire Switch Database Schema
-- Production-ready schema for Class 4 switching operations

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Call Detail Records table
CREATE TABLE call_detail_records (
    id UUID DEFAULT uuid_generate_v4(),
    call_id VARCHAR(255) NOT NULL,
    session_id UUID,
    from_number VARCHAR(50) NOT NULL,
    to_number VARCHAR(50) NOT NULL,
    from_ip INET,
    to_ip INET,
    start_time TIMESTAMP WITH TIME ZONE NOT NULL,
    end_time TIMESTAMP WITH TIME ZONE,
    duration_seconds BIGINT DEFAULT 0,
    disposition VARCHAR(50) NOT NULL,
    hangup_cause INTEGER,
    trunk_id VARCHAR(100),
    route_id VARCHAR(100),
    codec_in VARCHAR(20),
    codec_out VARCHAR(20),
    recording_enabled BOOLEAN DEFAULT FALSE,
    cost DECIMAL(10,6),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    -- Range partitioning requires the partition key to be part of any
    -- primary/unique key, so the PK is composite over (id, start_time).
    PRIMARY KEY (id, start_time)
) PARTITION BY RANGE (start_time);

-- Create indexes for CDR table
CREATE INDEX idx_cdr_call_id ON call_detail_records(call_id);
CREATE INDEX idx_cdr_start_time ON call_detail_records(start_time);
CREATE INDEX idx_cdr_from_number ON call_detail_records(from_number);
CREATE INDEX idx_cdr_to_number ON call_detail_records(to_number);
CREATE INDEX idx_cdr_trunk_id ON call_detail_records(trunk_id);
CREATE INDEX idx_cdr_disposition ON call_detail_records(disposition);

-- Partition CDR table by month for performance
CREATE TABLE call_detail_records_y2025m01 PARTITION OF call_detail_records
    FOR VALUES FROM ('2025-01-01') TO ('2025-02-01');
CREATE TABLE call_detail_records_y2025m02 PARTITION OF call_detail_records
    FOR VALUES FROM ('2025-02-01') TO ('2025-03-01');
CREATE TABLE call_detail_records_y2025m03 PARTITION OF call_detail_records
    FOR VALUES FROM ('2025-03-01') TO ('2025-04-01');
CREATE TABLE call_detail_records_y2025m04 PARTITION OF call_detail_records
    FOR VALUES FROM ('2025-04-01') TO ('2025-05-01');
CREATE TABLE call_detail_records_y2025m05 PARTITION OF call_detail_records
    FOR VALUES FROM ('2025-05-01') TO ('2025-06-01');
CREATE TABLE call_detail_records_y2025m06 PARTITION OF call_detail_records
    FOR VALUES FROM ('2025-06-01') TO ('2025-07-01');
CREATE TABLE call_detail_records_y2025m07 PARTITION OF call_detail_records
    FOR VALUES FROM ('2025-07-01') TO ('2025-08-01');
CREATE TABLE call_detail_records_y2025m08 PARTITION OF call_detail_records
    FOR VALUES FROM ('2025-08-01') TO ('2025-09-01');
CREATE TABLE call_detail_records_y2025m09 PARTITION OF call_detail_records
    FOR VALUES FROM ('2025-09-01') TO ('2025-10-01');
CREATE TABLE call_detail_records_y2025m10 PARTITION OF call_detail_records
    FOR VALUES FROM ('2025-10-01') TO ('2025-11-01');
CREATE TABLE call_detail_records_y2025m11 PARTITION OF call_detail_records
    FOR VALUES FROM ('2025-11-01') TO ('2025-12-01');
CREATE TABLE call_detail_records_y2025m12 PARTITION OF call_detail_records
    FOR VALUES FROM ('2025-12-01') TO ('2026-01-01');

-- SIP Profiles table
CREATE TABLE sip_profiles (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,
    bind_ip INET NOT NULL,
    port INTEGER NOT NULL,
    transport VARCHAR(10) NOT NULL DEFAULT 'udp',
    max_sessions INTEGER DEFAULT 10000,
    session_timer INTEGER DEFAULT 1800,
    enable_registration BOOLEAN DEFAULT FALSE,
    auth_calls BOOLEAN DEFAULT FALSE,
    transit_mode BOOLEAN DEFAULT TRUE,
    codec_negotiation VARCHAR(20) DEFAULT 'transparent',
    dtmf_relay VARCHAR(20) DEFAULT 'rfc2833',
    record_route BOOLEAN DEFAULT TRUE,
    proxy_media BOOLEAN DEFAULT FALSE,
    sip_i_support BOOLEAN DEFAULT TRUE,
    isup_interworking BOOLEAN DEFAULT TRUE,
    ss7_gateway_support BOOLEAN DEFAULT TRUE,
    cause_code_mapping VARCHAR(20) DEFAULT 'itu_t',
    release_source_header BOOLEAN DEFAULT TRUE,
    enabled BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Carrier Interconnects table
CREATE TABLE carrier_interconnects (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,
    carrier_id VARCHAR(50) NOT NULL,
    carrier_name VARCHAR(200) NOT NULL,
    direction VARCHAR(20) NOT NULL CHECK (direction IN ('termination', 'origination', 'bidirectional')),
    remote_ip INET NOT NULL,
    remote_port INTEGER NOT NULL,
    transport VARCHAR(10) NOT NULL DEFAULT 'udp',
    lcr_group VARCHAR(50),
    quality_score INTEGER DEFAULT 50 CHECK (quality_score >= 0 AND quality_score <= 100),
    capacity_limit INTEGER DEFAULT 1000,
    codec_preference TEXT[], -- Array of codec names
    sip_i_support BOOLEAN DEFAULT TRUE,
    isup_interworking BOOLEAN DEFAULT TRUE,
    ss7_protocol VARCHAR(20) DEFAULT 'itu_t',
    circuit_group_support BOOLEAN DEFAULT TRUE,
    cic_range VARCHAR(100),
    authentication_type VARCHAR(20) DEFAULT 'ip_auth',
    authentication_data JSONB,
    enabled BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Trunks table
CREATE TABLE trunks (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,
    tech_prefix VARCHAR(20),
    sip_profile_id INTEGER REFERENCES sip_profiles(id),
    carrier_interconnect_id INTEGER REFERENCES carrier_interconnects(id),
    vendor_customer_id INTEGER,
    trunk_type VARCHAR(20) NOT NULL CHECK (trunk_type IN ('termination', 'origination', 'bidirectional')),
    digit_manipulation JSONB,
    call_limits JSONB,
    allowed_codecs TEXT[],
    stir_shaken_config JSONB,
    lcr_group VARCHAR(50),
    priority INTEGER DEFAULT 50,
    enabled BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Vendors and Customers table
CREATE TABLE vendors_customers (
    id SERIAL PRIMARY KEY,
    name VARCHAR(200) NOT NULL,
    type VARCHAR(20) NOT NULL CHECK (type IN ('vendor', 'customer', 'partner')),
    contact_info JSONB,
    billing_info JSONB,
    technical_contact VARCHAR(200),
    billing_contact VARCHAR(200),
    emergency_contact VARCHAR(50),
    enabled BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- NOTE: the LCR routing tables (lcr_routes, vendor_rate_decks, trunks-as-egress,
-- etc.) are owned by migration 002_lcr_schema.sql, which matches the schema the
-- `lcr` module actually reads. An earlier, unused `lcr_routes` definition lived
-- here with a completely different shape (route_group/prefix/trunk_id); it has
-- been removed to avoid a table-name collision with the real LCR schema.

-- Active Sessions table (for call tracking)
CREATE TABLE active_sessions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    call_id VARCHAR(255) NOT NULL,
    session_id UUID,
    from_number VARCHAR(50) NOT NULL,
    to_number VARCHAR(50) NOT NULL,
    from_ip INET,
    to_ip INET,
    trunk_id INTEGER REFERENCES trunks(id),
    start_time TIMESTAMP WITH TIME ZONE NOT NULL,
    last_activity TIMESTAMP WITH TIME ZONE NOT NULL,
    state VARCHAR(20) NOT NULL,
    codec_in VARCHAR(20),
    codec_out VARCHAR(20),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create indexes for active sessions
CREATE INDEX idx_active_sessions_call_id ON active_sessions(call_id);
CREATE INDEX idx_active_sessions_state ON active_sessions(state);
CREATE INDEX idx_active_sessions_trunk_id ON active_sessions(trunk_id);

-- Security Events table
CREATE TABLE security_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_type VARCHAR(50) NOT NULL,
    source_ip INET NOT NULL,
    severity VARCHAR(20) NOT NULL,
    description TEXT,
    details JSONB,
    action_taken VARCHAR(100),
    resolved BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    resolved_at TIMESTAMP WITH TIME ZONE
);

-- Create indexes for security events
CREATE INDEX idx_security_events_type ON security_events(event_type);
CREATE INDEX idx_security_events_source_ip ON security_events(source_ip);
CREATE INDEX idx_security_events_severity ON security_events(severity);
CREATE INDEX idx_security_events_created_at ON security_events(created_at);

-- Blacklist table
CREATE TABLE ip_blacklist (
    id SERIAL PRIMARY KEY,
    ip_address INET NOT NULL UNIQUE,
    reason VARCHAR(100) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    expires_at TIMESTAMP WITH TIME ZONE,
    violation_count INTEGER DEFAULT 1,
    last_violation TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    automatic BOOLEAN DEFAULT TRUE
);

-- Create indexes for blacklist
CREATE INDEX idx_blacklist_ip ON ip_blacklist(ip_address);
CREATE INDEX idx_blacklist_expires ON ip_blacklist(expires_at);

-- System Configuration table
CREATE TABLE system_config (
    id SERIAL PRIMARY KEY,
    config_key VARCHAR(100) NOT NULL UNIQUE,
    config_value JSONB NOT NULL,
    description TEXT,
    config_type VARCHAR(50) NOT NULL,
    editable BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Health Check Results table
CREATE TABLE health_check_results (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    component VARCHAR(50) NOT NULL,
    status VARCHAR(20) NOT NULL,
    response_time_ms INTEGER,
    details JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create indexes for health checks
CREATE INDEX idx_health_check_component ON health_check_results(component);
CREATE INDEX idx_health_check_status ON health_check_results(status);
CREATE INDEX idx_health_check_created_at ON health_check_results(created_at);

-- Monitoring Metrics table
CREATE TABLE monitoring_metrics (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    metric_name VARCHAR(100) NOT NULL,
    metric_value DECIMAL(15,6) NOT NULL,
    metric_type VARCHAR(20) NOT NULL,
    labels JSONB,
    timestamp TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create indexes for monitoring metrics
CREATE INDEX idx_monitoring_metrics_name ON monitoring_metrics(metric_name);
CREATE INDEX idx_monitoring_metrics_timestamp ON monitoring_metrics(timestamp);

-- STIR/SHAKEN Certificates table
CREATE TABLE stir_shaken_certificates (
    id SERIAL PRIMARY KEY,
    certificate_name VARCHAR(100) NOT NULL UNIQUE,
    certificate_data TEXT NOT NULL,
    private_key_data TEXT,
    issuer VARCHAR(200),
    subject VARCHAR(200),
    valid_from TIMESTAMP WITH TIME ZONE,
    valid_until TIMESTAMP WITH TIME ZONE,
    fingerprint VARCHAR(128),
    enabled BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Audit Log table
CREATE TABLE audit_log (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id VARCHAR(100),
    action VARCHAR(100) NOT NULL,
    resource_type VARCHAR(50),
    resource_id VARCHAR(100),
    details JSONB,
    ip_address INET,
    user_agent TEXT,
    success BOOLEAN NOT NULL,
    error_message TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create indexes for audit log
CREATE INDEX idx_audit_log_user_id ON audit_log(user_id);
CREATE INDEX idx_audit_log_action ON audit_log(action);
CREATE INDEX idx_audit_log_resource_type ON audit_log(resource_type);
CREATE INDEX idx_audit_log_created_at ON audit_log(created_at);

-- Create function to automatically update 'updated_at' timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Create triggers for updated_at columns
CREATE TRIGGER update_sip_profiles_updated_at BEFORE UPDATE ON sip_profiles
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_carrier_interconnects_updated_at BEFORE UPDATE ON carrier_interconnects
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_trunks_updated_at BEFORE UPDATE ON trunks
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_vendors_customers_updated_at BEFORE UPDATE ON vendors_customers
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_active_sessions_updated_at BEFORE UPDATE ON active_sessions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_system_config_updated_at BEFORE UPDATE ON system_config
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_stir_shaken_certificates_updated_at BEFORE UPDATE ON stir_shaken_certificates
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

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

-- Create views for common queries
CREATE VIEW v_active_call_summary AS
SELECT
    COUNT(*) as total_active_calls,
    COUNT(DISTINCT trunk_id) as active_trunks,
    AVG(EXTRACT(EPOCH FROM (NOW() - start_time))) as avg_call_duration_seconds
FROM active_sessions
WHERE state = 'Active';

CREATE VIEW v_trunk_utilization AS
SELECT
    t.id,
    t.name,
    t.trunk_type,
    COALESCE(active_calls.call_count, 0) as current_calls,
    (t.call_limits->>'max_concurrent_calls')::INTEGER as max_calls,
    CASE
        WHEN (t.call_limits->>'max_concurrent_calls')::INTEGER > 0
        THEN ROUND((COALESCE(active_calls.call_count, 0)::DECIMAL / (t.call_limits->>'max_concurrent_calls')::INTEGER) * 100, 2)
        ELSE 0
    END as utilization_percentage
FROM trunks t
LEFT JOIN (
    SELECT trunk_id, COUNT(*) as call_count
    FROM active_sessions
    WHERE state = 'Active'
    GROUP BY trunk_id
) active_calls ON t.id = active_calls.trunk_id
WHERE t.enabled = true;

CREATE VIEW v_security_event_summary AS
SELECT
    event_type,
    severity,
    COUNT(*) as event_count,
    COUNT(DISTINCT source_ip) as unique_ips,
    MAX(created_at) as last_occurrence
FROM security_events
WHERE created_at >= NOW() - INTERVAL '24 hours'
GROUP BY event_type, severity
ORDER BY event_count DESC;

-- Performance optimization: Create indexes.
-- NOTE: partial-index predicates must be IMMUTABLE, so a NOW()-based
-- "recent" predicate is not allowed. Index the full column instead.
CREATE INDEX idx_cdr_recent_calls ON call_detail_records(start_time);

CREATE INDEX idx_active_sessions_current ON active_sessions(state, start_time)
WHERE state IN ('Establishing', 'Active');

CREATE INDEX idx_security_events_recent ON security_events(created_at, severity);

-- Cleanup function for old data
CREATE OR REPLACE FUNCTION cleanup_old_data() RETURNS void AS $$
BEGIN
    -- Clean up old CDR records (older than retention period)
    DELETE FROM call_detail_records
    WHERE start_time < NOW() - INTERVAL '1 year';

    -- Clean up old health check results (older than 7 days)
    DELETE FROM health_check_results
    WHERE created_at < NOW() - INTERVAL '7 days';

    -- Clean up old monitoring metrics (older than 30 days)
    DELETE FROM monitoring_metrics
    WHERE timestamp < NOW() - INTERVAL '30 days';

    -- Clean up resolved security events (older than 90 days)
    DELETE FROM security_events
    WHERE resolved = true AND resolved_at < NOW() - INTERVAL '90 days';

    -- Clean up expired blacklist entries
    DELETE FROM ip_blacklist
    WHERE expires_at IS NOT NULL AND expires_at < NOW();

    -- Clean up old audit log entries (older than 1 year)
    DELETE FROM audit_log
    WHERE created_at < NOW() - INTERVAL '1 year';

    RAISE NOTICE 'Old data cleanup completed';
END;
$$ LANGUAGE plpgsql;