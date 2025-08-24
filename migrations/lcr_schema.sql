-- Least Cost Routing (LCR) Schema for NANPA
-- Supports both LRN and DNIS rating with high precision decimal rates

-- Rate deck types
CREATE TYPE rate_type AS ENUM ('LRN', 'DNIS');
CREATE TYPE route_type AS ENUM ('NANPA', 'A-Z', 'OTHER');
CREATE TYPE call_jurisdiction AS ENUM ('INTER', 'INTRA', 'IJ', 'LOCAL');

-- Vendor rate decks (cost)
CREATE TABLE vendor_rate_decks (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    vendor_id INTEGER NOT NULL,
    rate_type rate_type NOT NULL DEFAULT 'DNIS',
    effective_date TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    end_date TIMESTAMP WITH TIME ZONE,
    deck_version INTEGER NOT NULL DEFAULT 1,
    parent_deck_id INTEGER REFERENCES vendor_rate_decks(id),
    effective_time TIME DEFAULT '00:00:00',
    preload_minutes INTEGER DEFAULT 30,
    loaded_at TIMESTAMP WITH TIME ZONE,
    is_staged BOOLEAN DEFAULT false,
    active BOOLEAN DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT vendor_deck_version_unique UNIQUE(vendor_id, name, deck_version)
);

-- Client rate decks (selling)
CREATE TABLE client_rate_decks (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    client_id INTEGER NOT NULL,
    rate_type rate_type NOT NULL DEFAULT 'DNIS',
    effective_date TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    end_date TIMESTAMP WITH TIME ZONE,
    deck_version INTEGER NOT NULL DEFAULT 1,
    parent_deck_id INTEGER REFERENCES client_rate_decks(id),
    effective_time TIME DEFAULT '00:00:00',
    preload_minutes INTEGER DEFAULT 30,
    loaded_at TIMESTAMP WITH TIME ZONE,
    is_staged BOOLEAN DEFAULT false,
    active BOOLEAN DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT client_deck_version_unique UNIQUE(client_id, name, deck_version)
);

-- NANPA vendor rates (cost)
CREATE TABLE vendor_nanpa_rates (
    id SERIAL PRIMARY KEY,
    deck_id INTEGER NOT NULL REFERENCES vendor_rate_decks(id) ON DELETE CASCADE,
    code VARCHAR(20) NOT NULL, -- 1NPANXX or more specific
    inter_rate DECIMAL(10, 7) NOT NULL, -- Interstate rate
    intra_rate DECIMAL(10, 7) NOT NULL, -- Intrastate rate
    ij_rate DECIMAL(10, 7) NOT NULL, -- Indeterminate jurisdiction rate
    local_rate DECIMAL(10, 7), -- Optional local rate
    min_increment INTEGER NOT NULL DEFAULT 6, -- Minimum increment in seconds
    interval INTEGER NOT NULL DEFAULT 6, -- Billing interval in seconds
    setup_fee DECIMAL(10, 7) DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT vendor_nanpa_rates_unique UNIQUE(deck_id, code),
    INDEX idx_vendor_nanpa_code (code),
    INDEX idx_vendor_nanpa_deck (deck_id)
);

-- NANPA client rates (selling)
CREATE TABLE client_nanpa_rates (
    id SERIAL PRIMARY KEY,
    deck_id INTEGER NOT NULL REFERENCES client_rate_decks(id) ON DELETE CASCADE,
    code VARCHAR(20) NOT NULL, -- 1NPANXX or more specific
    inter_rate DECIMAL(10, 7) NOT NULL, -- Interstate rate
    intra_rate DECIMAL(10, 7) NOT NULL, -- Intrastate rate
    ij_rate DECIMAL(10, 7) NOT NULL, -- Indeterminate jurisdiction rate
    local_rate DECIMAL(10, 7), -- Optional local rate
    min_increment INTEGER NOT NULL DEFAULT 6, -- Minimum increment in seconds
    interval INTEGER NOT NULL DEFAULT 6, -- Billing interval in seconds
    setup_fee DECIMAL(10, 7) DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT client_nanpa_rates_unique UNIQUE(deck_id, code),
    INDEX idx_client_nanpa_code (code),
    INDEX idx_client_nanpa_deck (deck_id)
);

-- Static NANPA database for jurisdiction determination
CREATE TABLE nanpa_static (
    id SERIAL PRIMARY KEY,
    npa VARCHAR(3) NOT NULL,
    nxx VARCHAR(3),
    state VARCHAR(2) NOT NULL,
    country VARCHAR(2) NOT NULL DEFAULT 'US',
    lata VARCHAR(5),
    ocn VARCHAR(4),
    rate_center VARCHAR(100),
    switch_clli VARCHAR(11),
    effective_date DATE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT nanpa_static_unique UNIQUE(npa, nxx),
    INDEX idx_nanpa_npa (npa),
    INDEX idx_nanpa_npanxx (npa, nxx)
);

-- Egress trunks (vendors)
CREATE TABLE egress_trunks (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    vendor_id INTEGER NOT NULL,
    host VARCHAR(255) NOT NULL,
    port INTEGER NOT NULL DEFAULT 5060,
    transport VARCHAR(10) DEFAULT 'UDP', -- UDP, TCP, TLS
    capacity_limit INTEGER DEFAULT 1000, -- Max concurrent calls
    cps_limit DECIMAL(10, 2) DEFAULT 100.0, -- Calls per second limit
    active BOOLEAN DEFAULT true,
    priority INTEGER DEFAULT 100, -- Lower is higher priority
    weight INTEGER DEFAULT 1, -- For load balancing
    tech_prefix VARCHAR(20), -- Optional tech prefix
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Ingress trunks (clients)
CREATE TABLE ingress_trunks (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    client_id INTEGER NOT NULL,
    ip_address INET NOT NULL,
    capacity_limit INTEGER DEFAULT 100, -- Max concurrent calls
    cps_limit DECIMAL(10, 2) DEFAULT 10.0, -- Calls per second limit
    profit_protection BOOLEAN DEFAULT true,
    min_profit_margin DECIMAL(10, 7) DEFAULT 0.0001, -- Minimum profit per minute
    active BOOLEAN DEFAULT true,
    auth_username VARCHAR(255),
    auth_password VARCHAR(255),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    INDEX idx_ingress_ip (ip_address)
);

-- Associate rate decks with trunks
CREATE TABLE trunk_rate_associations (
    id SERIAL PRIMARY KEY,
    egress_trunk_id INTEGER REFERENCES egress_trunks(id) ON DELETE CASCADE,
    ingress_trunk_id INTEGER REFERENCES ingress_trunks(id) ON DELETE CASCADE,
    vendor_deck_id INTEGER REFERENCES vendor_rate_decks(id) ON DELETE CASCADE,
    client_deck_id INTEGER REFERENCES client_rate_decks(id) ON DELETE CASCADE,
    priority INTEGER DEFAULT 100,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT trunk_rate_check CHECK (
        (egress_trunk_id IS NOT NULL AND vendor_deck_id IS NOT NULL AND ingress_trunk_id IS NULL AND client_deck_id IS NULL) OR
        (ingress_trunk_id IS NOT NULL AND client_deck_id IS NOT NULL AND egress_trunk_id IS NULL AND vendor_deck_id IS NULL)
    )
);

-- Dynamic LCR routes
CREATE TABLE lcr_routes (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    route_type route_type NOT NULL,
    description TEXT,
    active BOOLEAN DEFAULT true,
    priority INTEGER DEFAULT 100,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Link ingress trunks to LCR routes
CREATE TABLE ingress_lcr_routes (
    id SERIAL PRIMARY KEY,
    ingress_trunk_id INTEGER NOT NULL REFERENCES ingress_trunks(id) ON DELETE CASCADE,
    lcr_route_id INTEGER NOT NULL REFERENCES lcr_routes(id) ON DELETE CASCADE,
    priority INTEGER DEFAULT 100,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT ingress_lcr_unique UNIQUE(ingress_trunk_id, lcr_route_id)
);

-- Link egress trunks to LCR routes with deck associations
CREATE TABLE lcr_route_trunks (
    id SERIAL PRIMARY KEY,
    lcr_route_id INTEGER NOT NULL REFERENCES lcr_routes(id) ON DELETE CASCADE,
    egress_trunk_id INTEGER NOT NULL REFERENCES egress_trunks(id) ON DELETE CASCADE,
    vendor_deck_id INTEGER NOT NULL REFERENCES vendor_rate_decks(id) ON DELETE CASCADE,
    priority INTEGER DEFAULT 100,
    weight INTEGER DEFAULT 1,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT lcr_route_trunk_unique UNIQUE(lcr_route_id, egress_trunk_id, vendor_deck_id)
);

-- Static routes for special case handling
CREATE TABLE static_routes (
    id SERIAL PRIMARY KEY,
    ingress_trunk_id INTEGER REFERENCES ingress_trunks(id) ON DELETE CASCADE,
    egress_trunk_id INTEGER NOT NULL REFERENCES egress_trunks(id) ON DELETE CASCADE,
    pattern VARCHAR(255) NOT NULL, -- Regex pattern
    priority INTEGER DEFAULT 100, -- Lower is higher priority
    position VARCHAR(10) DEFAULT 'BEFORE', -- BEFORE or AFTER dynamic routes
    description TEXT,
    active BOOLEAN DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    INDEX idx_static_routes_ingress (ingress_trunk_id),
    INDEX idx_static_routes_priority (priority)
);

-- Route advance configurations
CREATE TABLE route_advance_configs (
    id SERIAL PRIMARY KEY,
    scope VARCHAR(20) NOT NULL, -- GLOBAL, INGRESS_TRUNK
    scope_id INTEGER, -- NULL for global, trunk_id for specific trunk
    advance_on_codes TEXT[] DEFAULT ARRAY['503', '504', '603', '606'], -- SIP codes to advance on
    stop_on_codes TEXT[] DEFAULT ARRAY['404', '486', '600', '604'], -- SIP codes to stop on
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT route_advance_scope_unique UNIQUE(scope, scope_id)
);

-- Timer configurations
CREATE TABLE timer_configs (
    id SERIAL PRIMARY KEY,
    scope VARCHAR(20) NOT NULL, -- GLOBAL, INGRESS_TRUNK, EGRESS_TRUNK
    scope_id INTEGER, -- NULL for global, trunk_id for specific trunk
    timer_100_to_183_ms INTEGER DEFAULT 30000, -- Max time between 100 and 183 (30 seconds)
    timer_max_call_duration_sec INTEGER DEFAULT 10800, -- Max call duration (3 hours)
    timer_post_dial_delay_ms INTEGER DEFAULT 5000, -- Post dial delay
    timer_ringing_timeout_sec INTEGER DEFAULT 120, -- Max ringing time
    timer_transaction_timeout_ms INTEGER DEFAULT 32000, -- SIP transaction timeout
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT timer_config_scope_unique UNIQUE(scope, scope_id)
);

-- Call statistics for tracking trunk usage
CREATE TABLE trunk_usage_stats (
    id SERIAL PRIMARY KEY,
    trunk_id INTEGER NOT NULL,
    trunk_type VARCHAR(10) NOT NULL, -- INGRESS or EGRESS
    current_calls INTEGER DEFAULT 0,
    current_cps DECIMAL(10, 2) DEFAULT 0.0,
    total_calls BIGINT DEFAULT 0,
    total_minutes DECIMAL(20, 2) DEFAULT 0.0,
    last_call_at TIMESTAMP WITH TIME ZONE,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT trunk_usage_unique UNIQUE(trunk_id, trunk_type)
);

-- LRN dip cache for performance
CREATE TABLE lrn_cache (
    id SERIAL PRIMARY KEY,
    tn VARCHAR(15) NOT NULL UNIQUE,
    lrn VARCHAR(15) NOT NULL,
    spid VARCHAR(10),
    ocn VARCHAR(10),
    lata VARCHAR(5),
    state VARCHAR(2),
    jurisdiction call_jurisdiction,
    cached_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    expires_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() + INTERVAL '24 hours',
    INDEX idx_lrn_cache_tn (tn),
    INDEX idx_lrn_cache_expires (expires_at)
);

-- Indexes for performance and LCR longest match optimization
CREATE INDEX idx_vendor_rates_lookup ON vendor_nanpa_rates(deck_id, code);
CREATE INDEX idx_client_rates_lookup ON client_nanpa_rates(deck_id, code);

-- Optimized indexes for longest prefix matching
CREATE INDEX idx_vendor_rates_prefix ON vendor_nanpa_rates (deck_id, code varchar_pattern_ops);
CREATE INDEX idx_client_rates_prefix ON client_nanpa_rates (deck_id, code varchar_pattern_ops);
CREATE INDEX idx_trunk_stats_update ON trunk_usage_stats(trunk_id, trunk_type, updated_at);

-- Function to update timestamps
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Create triggers for updated_at
CREATE TRIGGER update_vendor_rate_decks_updated_at BEFORE UPDATE ON vendor_rate_decks
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_client_rate_decks_updated_at BEFORE UPDATE ON client_rate_decks
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_egress_trunks_updated_at BEFORE UPDATE ON egress_trunks
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_ingress_trunks_updated_at BEFORE UPDATE ON ingress_trunks
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_lcr_routes_updated_at BEFORE UPDATE ON lcr_routes
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_route_advance_configs_updated_at BEFORE UPDATE ON route_advance_configs
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_timer_configs_updated_at BEFORE UPDATE ON timer_configs
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_trunk_usage_stats_updated_at BEFORE UPDATE ON trunk_usage_stats
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Insert default global configurations
INSERT INTO route_advance_configs (scope, scope_id, advance_on_codes, stop_on_codes)
VALUES ('GLOBAL', NULL, 
        ARRAY['503', '504', '603', '606', '480', '487', '502', '500'], 
        ARRAY['404', '486', '600', '604', '403', '401', '402']);

INSERT INTO timer_configs (scope, scope_id)
VALUES ('GLOBAL', NULL);

-- NANPA NPA (Area Code) classification from files/npa_report.csv
-- This replaces hardcoded area codes in jurisdiction.rs
-- Store as 1NPA format to avoid confusion with international country codes
CREATE TABLE nanpa_npa_info (
    id SERIAL PRIMARY KEY,
    npa VARCHAR(4) NOT NULL UNIQUE, -- 1NPA format (e.g., "1212", "1800")
    type_of_code VARCHAR(50), -- 'General Purpose Code', 'Easily Recognizable Code', etc.
    assignable BOOLEAN,
    reserved BOOLEAN,
    assigned BOOLEAN,
    assignment_date DATE,
    use_type CHAR(1), -- 'G'=General, 'N'=Non-assignable, etc.
    location VARCHAR(100),
    country VARCHAR(2), -- 'US', 'CANADA'
    in_service BOOLEAN,
    in_service_date DATE,
    status VARCHAR(50),
    overlay BOOLEAN,
    service_type VARCHAR(10), -- Used for jurisdiction determination
    time_zone VARCHAR(10),
    area_served VARCHAR(255),
    dialing_plan_notes TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- LERG NPA-NXX data from files/npa-nxx-companytype-ocn.csv  
-- This provides the 1NPANXX data for LERG-based routing
CREATE TABLE lerg_npanxx_info (
    id SERIAL PRIMARY KEY,
    npanxx VARCHAR(7) NOT NULL UNIQUE, -- 1NPANXX format (e.g., "1212555")
    npa VARCHAR(4) NOT NULL, -- 1NPA format (e.g., "1212")
    nxx VARCHAR(3) NOT NULL,
    company_type VARCHAR(20), -- 'WIRELESS', 'CLEC', 'PCS', etc.
    ocn INTEGER, -- Operating Company Number
    company_name VARCHAR(255),
    lata INTEGER,
    rate_center VARCHAR(50),
    state VARCHAR(2),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Special service codes classification (replaces hardcoded values!)
-- This table defines which NPAs should be treated as Indeterminate
CREATE TABLE special_service_codes (
    id SERIAL PRIMARY KEY,
    npa VARCHAR(4) NOT NULL UNIQUE, -- 1NPA format (e.g., "1800")
    service_type VARCHAR(50) NOT NULL, -- 'TOLL_FREE', 'PREMIUM', 'INTERNATIONAL_ACCESS', etc.
    jurisdiction_override call_jurisdiction DEFAULT 'IJ', -- Most special services are IJ
    description TEXT,
    active BOOLEAN DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Indexes for fast lookups
CREATE INDEX idx_nanpa_npa_info_npa ON nanpa_npa_info(npa);
CREATE INDEX idx_nanpa_npa_info_country ON nanpa_npa_info(country);
CREATE INDEX idx_nanpa_npa_info_service_type ON nanpa_npa_info(service_type);
CREATE INDEX idx_lerg_npanxx_npa_nxx ON lerg_npanxx_info(npa, nxx);
CREATE INDEX idx_lerg_npanxx_state ON lerg_npanxx_info(state);
CREATE INDEX idx_special_service_codes_npa ON special_service_codes(npa);
CREATE INDEX idx_special_service_codes_service_type ON special_service_codes(service_type);

-- Insert known special service codes to replace hardcoded values (in 1NPA format)
-- Toll-free numbers
INSERT INTO special_service_codes (npa, service_type, jurisdiction_override, description, active)
VALUES 
    ('1800', 'TOLL_FREE', 'IJ', 'Toll-free service', true),
    ('1833', 'TOLL_FREE', 'IJ', 'Toll-free service', true),
    ('1844', 'TOLL_FREE', 'IJ', 'Toll-free service', true),
    ('1855', 'TOLL_FREE', 'IJ', 'Toll-free service', true),
    ('1866', 'TOLL_FREE', 'IJ', 'Toll-free service', true),
    ('1877', 'TOLL_FREE', 'IJ', 'Toll-free service', true),
    ('1888', 'TOLL_FREE', 'IJ', 'Toll-free service', true);

-- Premium rate services
INSERT INTO special_service_codes (npa, service_type, jurisdiction_override, description, active)
VALUES 
    ('1900', 'PREMIUM', 'IJ', 'Premium rate service', true),
    ('1976', 'PREMIUM', 'IJ', 'Premium rate audiotext', true);

-- Special services
INSERT INTO special_service_codes (npa, service_type, jurisdiction_override, description, active)
VALUES 
    ('1500', 'PERSONAL_COMM', 'IJ', 'Personal communication service', true),
    ('1700', 'IC_SERVICES', 'IJ', 'Interexchange carrier services', true),
    ('1710', 'GOVERNMENT', 'IJ', 'Government services', true),
    ('1720', 'SPECIAL_VOIP', 'IJ', 'Special/VoIP services', true);

-- Additional indexes for deck versioning
CREATE INDEX IF NOT EXISTS idx_vendor_decks_effective ON vendor_rate_decks(effective_date, end_date);
CREATE INDEX IF NOT EXISTS idx_vendor_decks_staged ON vendor_rate_decks(is_staged, effective_date) WHERE is_staged = true;
CREATE INDEX IF NOT EXISTS idx_client_decks_effective ON client_rate_decks(effective_date, end_date);
CREATE INDEX IF NOT EXISTS idx_client_decks_staged ON client_rate_decks(is_staged, effective_date) WHERE is_staged = true;

-- Table to track deck loading history
CREATE TABLE IF NOT EXISTS deck_load_history (
    id SERIAL PRIMARY KEY,
    deck_type VARCHAR(20) NOT NULL, -- 'vendor' or 'client'
    deck_id INTEGER NOT NULL,
    deck_version INTEGER NOT NULL,
    loaded_by VARCHAR(255),
    loaded_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    effective_date TIMESTAMP WITH TIME ZONE NOT NULL,
    end_date TIMESTAMP WITH TIME ZONE,
    rate_count INTEGER,
    load_duration_ms INTEGER,
    notes TEXT
);

-- Table for deck cutover scheduling
CREATE TABLE IF NOT EXISTS deck_cutover_schedule (
    id SERIAL PRIMARY KEY,
    deck_type VARCHAR(20) NOT NULL,
    current_deck_id INTEGER NOT NULL,
    new_deck_id INTEGER NOT NULL,
    cutover_date TIMESTAMP WITH TIME ZONE NOT NULL,
    preload_at TIMESTAMP WITH TIME ZONE NOT NULL,
    status VARCHAR(20) DEFAULT 'scheduled', -- scheduled, preloading, preloaded, active, completed
    preloaded_at TIMESTAMP WITH TIME ZONE,
    activated_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_cutover_schedule_status ON deck_cutover_schedule(status, preload_at);

-- Function to get active deck at a specific time
CREATE OR REPLACE FUNCTION get_active_vendor_deck(
    p_vendor_id INTEGER,
    p_deck_name VARCHAR,
    p_timestamp TIMESTAMP WITH TIME ZONE DEFAULT NOW()
) RETURNS INTEGER AS $$
DECLARE
    v_deck_id INTEGER;
BEGIN
    SELECT id INTO v_deck_id
    FROM vendor_rate_decks
    WHERE vendor_id = p_vendor_id
      AND name = p_deck_name
      AND effective_date <= p_timestamp
      AND (end_date IS NULL OR end_date > p_timestamp)
      AND active = true
    ORDER BY deck_version DESC
    LIMIT 1;
    
    RETURN v_deck_id;
END;
$$ LANGUAGE plpgsql;

-- Function to get active client deck at a specific time
CREATE OR REPLACE FUNCTION get_active_client_deck(
    p_client_id INTEGER,
    p_deck_name VARCHAR,
    p_timestamp TIMESTAMP WITH TIME ZONE DEFAULT NOW()
) RETURNS INTEGER AS $$
DECLARE
    v_deck_id INTEGER;
BEGIN
    SELECT id INTO v_deck_id
    FROM client_rate_decks
    WHERE client_id = p_client_id
      AND name = p_deck_name
      AND effective_date <= p_timestamp
      AND (end_date IS NULL OR end_date > p_timestamp)
      AND active = true
    ORDER BY deck_version DESC
    LIMIT 1;
    
    RETURN v_deck_id;
END;
$$ LANGUAGE plpgsql;

-- Function to automatically set end_date when loading new version
CREATE OR REPLACE FUNCTION update_deck_end_dates()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.parent_deck_id IS NOT NULL THEN
        -- Update end_date of previous version
        IF TG_TABLE_NAME = 'vendor_rate_decks' THEN
            UPDATE vendor_rate_decks
            SET end_date = NEW.effective_date - INTERVAL '1 second',
                updated_at = NOW()
            WHERE id = NEW.parent_deck_id
              AND end_date IS NULL;
        ELSIF TG_TABLE_NAME = 'client_rate_decks' THEN
            UPDATE client_rate_decks
            SET end_date = NEW.effective_date - INTERVAL '1 second',
                updated_at = NOW()
            WHERE id = NEW.parent_deck_id
              AND end_date IS NULL;
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create triggers for automatic end_date management
DROP TRIGGER IF EXISTS vendor_deck_end_date_trigger ON vendor_rate_decks;
CREATE TRIGGER vendor_deck_end_date_trigger
AFTER INSERT ON vendor_rate_decks
FOR EACH ROW
EXECUTE FUNCTION update_deck_end_dates();

DROP TRIGGER IF EXISTS client_deck_end_date_trigger ON client_rate_decks;
CREATE TRIGGER client_deck_end_date_trigger
AFTER INSERT ON client_rate_decks
FOR EACH ROW
EXECUTE FUNCTION update_deck_end_dates();

-- Notification system for deck changes
CREATE OR REPLACE FUNCTION notify_deck_change()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify('deck_change', json_build_object(
        'action', TG_OP,
        'table', TG_TABLE_NAME,
        'deck_id', NEW.id,
        'effective_date', NEW.effective_date,
        'deck_version', NEW.deck_version
    )::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER vendor_deck_notify_trigger
AFTER INSERT OR UPDATE ON vendor_rate_decks
FOR EACH ROW
EXECUTE FUNCTION notify_deck_change();

CREATE TRIGGER client_deck_notify_trigger
AFTER INSERT OR UPDATE ON client_rate_decks
FOR EACH ROW
EXECUTE FUNCTION notify_deck_change();