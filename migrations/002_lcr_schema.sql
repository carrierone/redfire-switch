-- LCR (Least Cost Routing) schema.
--
-- This is a single, self-contained migration that supersedes the old, never-run
-- lcr_schema.sql / complete_lcr_schema.sql pair (which used MySQL-style inline
-- `INDEX` clauses, psql `\i` includes, and Postgres ENUM types for columns that
-- the Rust loaders read as plain strings). It creates every table the `lcr`
-- module touches at startup (LcrEngine::new -> ensure_default_routing_plans +
-- LcrCache::load_from_database) plus the supporting tables used at runtime.
--
-- Design notes / reconciliations with src/lcr/database.rs:
--   * rate_type / route_type / jurisdiction / special_service jurisdiction_override
--     are VARCHAR, not ENUM: the loaders do `row.get::<String, _>(...)`, which
--     cannot decode a Postgres enum.
--   * rate decks expose the expiry column as `expires_date` (the loaders SELECT
--     `expires_date`, not `end_date`).
--   * ingress_trunks.ip_address is VARCHAR, not INET: the loader reads it as a
--     String and parses it with IpAddr::from_str.
--   * All CREATE statements are idempotent so the migration can be re-applied.

-- ---------------------------------------------------------------------------
-- Rate decks
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS vendor_rate_decks (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    vendor_id INTEGER NOT NULL,
    rate_type VARCHAR(10) NOT NULL DEFAULT 'DNIS',
    effective_date TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    expires_date TIMESTAMP WITH TIME ZONE,
    deck_version INTEGER NOT NULL DEFAULT 1,
    parent_deck_id INTEGER REFERENCES vendor_rate_decks(id),
    effective_time TIME DEFAULT '00:00:00',
    preload_minutes INTEGER DEFAULT 30,
    loaded_at TIMESTAMP WITH TIME ZONE,
    is_staged BOOLEAN DEFAULT false,
    active BOOLEAN DEFAULT true,
    deleted BOOLEAN DEFAULT false,
    deleted_at TIMESTAMP WITH TIME ZONE,
    last_used_at TIMESTAMP WITH TIME ZONE,
    usage_count BIGINT DEFAULT 0,
    is_protected BOOLEAN DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT vendor_deck_version_unique UNIQUE(vendor_id, name, deck_version)
);

CREATE TABLE IF NOT EXISTS client_rate_decks (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    client_id INTEGER NOT NULL,
    rate_type VARCHAR(10) NOT NULL DEFAULT 'DNIS',
    effective_date TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    expires_date TIMESTAMP WITH TIME ZONE,
    deck_version INTEGER NOT NULL DEFAULT 1,
    parent_deck_id INTEGER REFERENCES client_rate_decks(id),
    effective_time TIME DEFAULT '00:00:00',
    preload_minutes INTEGER DEFAULT 30,
    loaded_at TIMESTAMP WITH TIME ZONE,
    is_staged BOOLEAN DEFAULT false,
    active BOOLEAN DEFAULT true,
    deleted BOOLEAN DEFAULT false,
    deleted_at TIMESTAMP WITH TIME ZONE,
    last_used_at TIMESTAMP WITH TIME ZONE,
    usage_count BIGINT DEFAULT 0,
    is_protected BOOLEAN DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT client_deck_version_unique UNIQUE(client_id, name, deck_version)
);

-- ---------------------------------------------------------------------------
-- NANPA rates (cost / sell)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS vendor_nanpa_rates (
    id SERIAL PRIMARY KEY,
    deck_id INTEGER NOT NULL REFERENCES vendor_rate_decks(id) ON DELETE CASCADE,
    code VARCHAR(20) NOT NULL,                 -- 1NPANXX (or a more specific prefix)
    inter_rate DECIMAL(10, 7) NOT NULL,        -- Interstate
    intra_rate DECIMAL(10, 7) NOT NULL,        -- Intrastate
    ij_rate DECIMAL(10, 7) NOT NULL,           -- Indeterminate jurisdiction
    local_rate DECIMAL(10, 7),                 -- Optional local rate (falls back to intra_rate)
    min_increment INTEGER NOT NULL DEFAULT 6,
    interval INTEGER NOT NULL DEFAULT 6,
    setup_fee DECIMAL(10, 7) DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT vendor_nanpa_rates_unique UNIQUE(deck_id, code)
);

CREATE TABLE IF NOT EXISTS client_nanpa_rates (
    id SERIAL PRIMARY KEY,
    deck_id INTEGER NOT NULL REFERENCES client_rate_decks(id) ON DELETE CASCADE,
    code VARCHAR(20) NOT NULL,
    inter_rate DECIMAL(10, 7) NOT NULL,
    intra_rate DECIMAL(10, 7) NOT NULL,
    ij_rate DECIMAL(10, 7) NOT NULL,
    local_rate DECIMAL(10, 7),
    min_increment INTEGER NOT NULL DEFAULT 6,
    interval INTEGER NOT NULL DEFAULT 6,
    setup_fee DECIMAL(10, 7) DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT client_nanpa_rates_unique UNIQUE(deck_id, code)
);

-- ---------------------------------------------------------------------------
-- International rates
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS vendor_international_rates (
    id SERIAL PRIMARY KEY,
    deck_id INTEGER NOT NULL REFERENCES vendor_rate_decks(id) ON DELETE CASCADE,
    country_code VARCHAR(10) NOT NULL,
    destination_code VARCHAR(20),
    destination_name VARCHAR(255) NOT NULL,
    jurisdiction VARCHAR(10) NOT NULL,          -- 'EEA' or 'ROW'
    rate DECIMAL(10, 7) NOT NULL,
    initial_increment INTEGER NOT NULL DEFAULT 30,
    subsequent_increment INTEGER NOT NULL DEFAULT 6,
    setup_fee DECIMAL(10, 7) DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT vendor_intl_rates_unique UNIQUE(deck_id, country_code, destination_code)
);

CREATE TABLE IF NOT EXISTS client_international_rates (
    id SERIAL PRIMARY KEY,
    deck_id INTEGER NOT NULL REFERENCES client_rate_decks(id) ON DELETE CASCADE,
    country_code VARCHAR(10) NOT NULL,
    destination_code VARCHAR(20),
    destination_name VARCHAR(255) NOT NULL,
    jurisdiction VARCHAR(10) NOT NULL,
    rate DECIMAL(10, 7) NOT NULL,
    initial_increment INTEGER NOT NULL DEFAULT 30,
    subsequent_increment INTEGER NOT NULL DEFAULT 6,
    setup_fee DECIMAL(10, 7) DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT client_intl_rates_unique UNIQUE(deck_id, country_code, destination_code)
);

-- ---------------------------------------------------------------------------
-- International routing plans
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS international_routing_plans (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    phone_validation_enabled BOOLEAN DEFAULT true,
    phone_validation_strict BOOLEAN DEFAULT false,
    phone_validation_default_region VARCHAR(2) DEFAULT 'US',
    phone_validation_use_country_detection BOOLEAN DEFAULT true,
    eea_routing_enabled BOOLEAN DEFAULT true,
    eea_priority_routing BOOLEAN DEFAULT true,
    eea_reduced_rates BOOLEAN DEFAULT true,
    eea_rate_reduction DECIMAL(5,4) DEFAULT 0.1000,
    default_jurisdiction VARCHAR(10) DEFAULT 'ROW',
    allow_unknown_destinations BOOLEAN DEFAULT true,
    max_rate_unknown_destinations DECIMAL(10,7) DEFAULT 1.0000,
    require_strict_validation_unknown BOOLEAN DEFAULT true,
    active BOOLEAN DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS country_routing_preferences (
    id SERIAL PRIMARY KEY,
    routing_plan_id INTEGER NOT NULL REFERENCES international_routing_plans(id) ON DELETE CASCADE,
    country_code VARCHAR(2) NOT NULL,
    country_name VARCHAR(100) NOT NULL,
    jurisdiction VARCHAR(10) NOT NULL,          -- 'EEA' or 'ROW'
    quality_score INTEGER DEFAULT 100,
    cost_multiplier DECIMAL(5,3) DEFAULT 1.000,
    require_validation BOOLEAN DEFAULT true,
    max_duration_minutes INTEGER DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT country_routing_unique UNIQUE(routing_plan_id, country_code)
);

-- ---------------------------------------------------------------------------
-- Trunks
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS egress_trunks (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    vendor_id INTEGER NOT NULL,
    host VARCHAR(255) NOT NULL,
    port INTEGER NOT NULL DEFAULT 5060,
    transport VARCHAR(10) DEFAULT 'UDP',
    capacity_limit INTEGER DEFAULT 1000,
    cps_limit DECIMAL(10, 2) DEFAULT 100.0,
    active BOOLEAN DEFAULT true,
    priority INTEGER DEFAULT 100,
    weight INTEGER DEFAULT 1,
    tech_prefix VARCHAR(20),
    supports_international BOOLEAN DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS ingress_trunks (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    client_id INTEGER NOT NULL,
    ip_address VARCHAR(45) NOT NULL,            -- read as String, parsed via IpAddr::from_str
    capacity_limit INTEGER DEFAULT 100,
    cps_limit DECIMAL(10, 2) DEFAULT 10.0,
    profit_protection BOOLEAN DEFAULT true,
    min_profit_margin DECIMAL(10, 7) DEFAULT 0.0001,
    active BOOLEAN DEFAULT true,
    auth_username VARCHAR(255),
    auth_password VARCHAR(255),
    supports_international BOOLEAN DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ---------------------------------------------------------------------------
-- Routing (dynamic + static)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS lcr_routes (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    route_type VARCHAR(10) NOT NULL,            -- 'NANPA' | 'A-Z' | 'OTHER'
    description TEXT,
    active BOOLEAN DEFAULT true,
    priority INTEGER DEFAULT 100,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS ingress_lcr_routes (
    id SERIAL PRIMARY KEY,
    ingress_trunk_id INTEGER NOT NULL REFERENCES ingress_trunks(id) ON DELETE CASCADE,
    lcr_route_id INTEGER NOT NULL REFERENCES lcr_routes(id) ON DELETE CASCADE,
    priority INTEGER DEFAULT 100,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT ingress_lcr_unique UNIQUE(ingress_trunk_id, lcr_route_id)
);

CREATE TABLE IF NOT EXISTS lcr_route_trunks (
    id SERIAL PRIMARY KEY,
    lcr_route_id INTEGER NOT NULL REFERENCES lcr_routes(id) ON DELETE CASCADE,
    egress_trunk_id INTEGER NOT NULL REFERENCES egress_trunks(id) ON DELETE CASCADE,
    vendor_deck_id INTEGER NOT NULL REFERENCES vendor_rate_decks(id) ON DELETE CASCADE,
    priority INTEGER DEFAULT 100,
    weight INTEGER DEFAULT 1,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT lcr_route_trunk_unique UNIQUE(lcr_route_id, egress_trunk_id, vendor_deck_id)
);

CREATE TABLE IF NOT EXISTS static_routes (
    id SERIAL PRIMARY KEY,
    ingress_trunk_id INTEGER REFERENCES ingress_trunks(id) ON DELETE CASCADE,
    egress_trunk_id INTEGER NOT NULL REFERENCES egress_trunks(id) ON DELETE CASCADE,
    pattern VARCHAR(255) NOT NULL,
    priority INTEGER DEFAULT 100,
    position VARCHAR(10) DEFAULT 'BEFORE',      -- 'BEFORE' | 'AFTER'
    description TEXT,
    active BOOLEAN DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS trunk_rate_associations (
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

-- ---------------------------------------------------------------------------
-- Configuration
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS route_advance_configs (
    id SERIAL PRIMARY KEY,
    scope VARCHAR(20) NOT NULL,                 -- GLOBAL | INGRESS_TRUNK | EGRESS_TRUNK
    scope_id INTEGER,                           -- NULL for GLOBAL
    advance_on_codes TEXT[] DEFAULT ARRAY['503', '504', '603', '606'],
    stop_on_codes TEXT[] DEFAULT ARRAY['404', '486', '600', '604'],
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT route_advance_scope_unique UNIQUE(scope, scope_id)
);

CREATE TABLE IF NOT EXISTS timer_configs (
    id SERIAL PRIMARY KEY,
    scope VARCHAR(20) NOT NULL,
    scope_id INTEGER,
    timer_100_to_183_ms INTEGER DEFAULT 30000,
    timer_max_call_duration_sec INTEGER DEFAULT 10800,
    timer_post_dial_delay_ms INTEGER DEFAULT 5000,
    timer_ringing_timeout_sec INTEGER DEFAULT 120,
    timer_transaction_timeout_ms INTEGER DEFAULT 32000,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT timer_config_scope_unique UNIQUE(scope, scope_id)
);

-- ---------------------------------------------------------------------------
-- NANPA / LERG reference data
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS nanpa_static (
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
    CONSTRAINT nanpa_static_unique UNIQUE(npa, nxx)
);

CREATE TABLE IF NOT EXISTS nanpa_npa_info (
    id SERIAL PRIMARY KEY,
    npa VARCHAR(4) NOT NULL UNIQUE,             -- 1NPA format (e.g. "1212", "1800")
    type_of_code VARCHAR(50),
    assignable BOOLEAN,
    reserved BOOLEAN,
    assigned BOOLEAN,
    assignment_date DATE,
    use_type CHAR(1),
    location VARCHAR(100),
    country VARCHAR(2),
    in_service BOOLEAN,
    in_service_date DATE,
    status VARCHAR(50),
    overlay BOOLEAN,
    service_type VARCHAR(10),
    time_zone VARCHAR(10),
    area_served VARCHAR(255),
    dialing_plan_notes TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS lerg_npanxx_info (
    id SERIAL PRIMARY KEY,
    npanxx VARCHAR(7) NOT NULL UNIQUE,          -- 1NPANXX format
    npa VARCHAR(4) NOT NULL,
    nxx VARCHAR(3) NOT NULL,
    company_type VARCHAR(20),
    ocn INTEGER,
    company_name VARCHAR(255),
    lata INTEGER,
    rate_center VARCHAR(50),
    state VARCHAR(2),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS special_service_codes (
    id SERIAL PRIMARY KEY,
    npa VARCHAR(4) NOT NULL UNIQUE,             -- 1NPA format
    service_type VARCHAR(50) NOT NULL,
    jurisdiction_override VARCHAR(20) DEFAULT 'IJ',  -- read as text; 'IJ' => Indeterminate
    description TEXT,
    active BOOLEAN DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ---------------------------------------------------------------------------
-- LRN dip cache + trunk usage stats
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS lrn_cache (
    id SERIAL PRIMARY KEY,
    tn VARCHAR(15) NOT NULL UNIQUE,
    lrn VARCHAR(15) NOT NULL,
    spid VARCHAR(10),
    ocn VARCHAR(10),
    lata VARCHAR(5),
    state VARCHAR(2),
    jurisdiction VARCHAR(20),                   -- read as String
    cached_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    expires_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() + INTERVAL '24 hours'
);

CREATE TABLE IF NOT EXISTS trunk_usage_stats (
    id SERIAL PRIMARY KEY,
    trunk_id INTEGER NOT NULL,
    trunk_type VARCHAR(10) NOT NULL,            -- INGRESS | EGRESS
    current_calls INTEGER DEFAULT 0,
    current_cps DECIMAL(10, 2) DEFAULT 0.0,
    total_calls BIGINT DEFAULT 0,
    total_minutes DECIMAL(20, 2) DEFAULT 0.0,
    last_call_at TIMESTAMP WITH TIME ZONE,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT trunk_usage_unique UNIQUE(trunk_id, trunk_type)
);

-- ---------------------------------------------------------------------------
-- Deck lifecycle bookkeeping
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS deck_cutover_schedule (
    id SERIAL PRIMARY KEY,
    deck_type VARCHAR(20) NOT NULL,             -- 'vendor' | 'client'
    current_deck_id INTEGER NOT NULL,
    new_deck_id INTEGER NOT NULL,
    cutover_date TIMESTAMP WITH TIME ZONE NOT NULL,
    preload_at TIMESTAMP WITH TIME ZONE NOT NULL,
    status VARCHAR(20) DEFAULT 'scheduled',
    preloaded_at TIMESTAMP WITH TIME ZONE,
    activated_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS deck_load_history (
    id SERIAL PRIMARY KEY,
    deck_type VARCHAR(20) NOT NULL,
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

-- ---------------------------------------------------------------------------
-- Indexes
-- ---------------------------------------------------------------------------

CREATE INDEX IF NOT EXISTS idx_vendor_nanpa_code ON vendor_nanpa_rates(code);
CREATE INDEX IF NOT EXISTS idx_vendor_nanpa_deck ON vendor_nanpa_rates(deck_id);
CREATE INDEX IF NOT EXISTS idx_client_nanpa_code ON client_nanpa_rates(code);
CREATE INDEX IF NOT EXISTS idx_client_nanpa_deck ON client_nanpa_rates(deck_id);
CREATE INDEX IF NOT EXISTS idx_vendor_rates_prefix ON vendor_nanpa_rates(deck_id, code varchar_pattern_ops);
CREATE INDEX IF NOT EXISTS idx_client_rates_prefix ON client_nanpa_rates(deck_id, code varchar_pattern_ops);
CREATE INDEX IF NOT EXISTS idx_vendor_intl_prefix ON vendor_international_rates(deck_id, country_code, destination_code);
CREATE INDEX IF NOT EXISTS idx_vendor_intl_country ON vendor_international_rates(country_code);
CREATE INDEX IF NOT EXISTS idx_client_intl_prefix ON client_international_rates(deck_id, country_code, destination_code);
CREATE INDEX IF NOT EXISTS idx_client_intl_country ON client_international_rates(country_code);
CREATE INDEX IF NOT EXISTS idx_vendor_decks_effective ON vendor_rate_decks(effective_date, expires_date);
CREATE INDEX IF NOT EXISTS idx_client_decks_effective ON client_rate_decks(effective_date, expires_date);
CREATE INDEX IF NOT EXISTS idx_vendor_rate_decks_active ON vendor_rate_decks(active) WHERE deleted = false;
CREATE INDEX IF NOT EXISTS idx_client_rate_decks_active ON client_rate_decks(active) WHERE deleted = false;
CREATE INDEX IF NOT EXISTS idx_intl_routing_plans_active ON international_routing_plans(active);
CREATE INDEX IF NOT EXISTS idx_country_preferences_plan ON country_routing_preferences(routing_plan_id);
CREATE INDEX IF NOT EXISTS idx_country_preferences_country ON country_routing_preferences(country_code);
CREATE INDEX IF NOT EXISTS idx_nanpa_npa ON nanpa_static(npa);
CREATE INDEX IF NOT EXISTS idx_nanpa_npanxx ON nanpa_static(npa, nxx);
CREATE INDEX IF NOT EXISTS idx_ingress_ip ON ingress_trunks(ip_address);
CREATE INDEX IF NOT EXISTS idx_static_routes_ingress ON static_routes(ingress_trunk_id);
CREATE INDEX IF NOT EXISTS idx_static_routes_priority ON static_routes(priority);
CREATE INDEX IF NOT EXISTS idx_lrn_cache_tn ON lrn_cache(tn);
CREATE INDEX IF NOT EXISTS idx_lrn_cache_expires ON lrn_cache(expires_at);
CREATE INDEX IF NOT EXISTS idx_trunk_stats_update ON trunk_usage_stats(trunk_id, trunk_type, updated_at);
CREATE INDEX IF NOT EXISTS idx_nanpa_npa_info_npa ON nanpa_npa_info(npa);
CREATE INDEX IF NOT EXISTS idx_nanpa_npa_info_country ON nanpa_npa_info(country);
CREATE INDEX IF NOT EXISTS idx_lerg_npanxx_npa_nxx ON lerg_npanxx_info(npa, nxx);
CREATE INDEX IF NOT EXISTS idx_lerg_npanxx_state ON lerg_npanxx_info(state);
CREATE INDEX IF NOT EXISTS idx_special_service_codes_npa ON special_service_codes(npa);
CREATE INDEX IF NOT EXISTS idx_cutover_schedule_status ON deck_cutover_schedule(status, preload_at);

-- ---------------------------------------------------------------------------
-- Triggers: keep updated_at fresh and auto-expire superseded deck versions.
-- ---------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION update_deck_expiry_dates()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.parent_deck_id IS NOT NULL THEN
        IF TG_TABLE_NAME = 'vendor_rate_decks' THEN
            UPDATE vendor_rate_decks
               SET expires_date = NEW.effective_date - INTERVAL '1 second',
                   updated_at = NOW()
             WHERE id = NEW.parent_deck_id
               AND expires_date IS NULL;
        ELSIF TG_TABLE_NAME = 'client_rate_decks' THEN
            UPDATE client_rate_decks
               SET expires_date = NEW.effective_date - INTERVAL '1 second',
                   updated_at = NOW()
             WHERE id = NEW.parent_deck_id
               AND expires_date IS NULL;
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS update_vendor_rate_decks_updated_at ON vendor_rate_decks;
CREATE TRIGGER update_vendor_rate_decks_updated_at BEFORE UPDATE ON vendor_rate_decks
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_client_rate_decks_updated_at ON client_rate_decks;
CREATE TRIGGER update_client_rate_decks_updated_at BEFORE UPDATE ON client_rate_decks
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_egress_trunks_updated_at ON egress_trunks;
CREATE TRIGGER update_egress_trunks_updated_at BEFORE UPDATE ON egress_trunks
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_ingress_trunks_updated_at ON ingress_trunks;
CREATE TRIGGER update_ingress_trunks_updated_at BEFORE UPDATE ON ingress_trunks
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_lcr_routes_updated_at ON lcr_routes;
CREATE TRIGGER update_lcr_routes_updated_at BEFORE UPDATE ON lcr_routes
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_route_advance_configs_updated_at ON route_advance_configs;
CREATE TRIGGER update_route_advance_configs_updated_at BEFORE UPDATE ON route_advance_configs
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_timer_configs_updated_at ON timer_configs;
CREATE TRIGGER update_timer_configs_updated_at BEFORE UPDATE ON timer_configs
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_trunk_usage_stats_updated_at ON trunk_usage_stats;
CREATE TRIGGER update_trunk_usage_stats_updated_at BEFORE UPDATE ON trunk_usage_stats
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS vendor_deck_expiry_trigger ON vendor_rate_decks;
CREATE TRIGGER vendor_deck_expiry_trigger AFTER INSERT ON vendor_rate_decks
    FOR EACH ROW EXECUTE FUNCTION update_deck_expiry_dates();

DROP TRIGGER IF EXISTS client_deck_expiry_trigger ON client_rate_decks;
CREATE TRIGGER client_deck_expiry_trigger AFTER INSERT ON client_rate_decks
    FOR EACH ROW EXECUTE FUNCTION update_deck_expiry_dates();

-- ---------------------------------------------------------------------------
-- Seed data
-- ---------------------------------------------------------------------------

INSERT INTO route_advance_configs (scope, scope_id, advance_on_codes, stop_on_codes)
VALUES ('GLOBAL', NULL,
        ARRAY['503', '504', '603', '606', '480', '487', '502', '500'],
        ARRAY['404', '486', '600', '604', '403', '401', '402'])
ON CONFLICT (scope, scope_id) DO NOTHING;

INSERT INTO timer_configs (scope, scope_id)
VALUES ('GLOBAL', NULL)
ON CONFLICT (scope, scope_id) DO NOTHING;

INSERT INTO special_service_codes (npa, service_type, jurisdiction_override, description, active)
VALUES
    ('1800', 'TOLL_FREE', 'IJ', 'Toll-free service', true),
    ('1833', 'TOLL_FREE', 'IJ', 'Toll-free service', true),
    ('1844', 'TOLL_FREE', 'IJ', 'Toll-free service', true),
    ('1855', 'TOLL_FREE', 'IJ', 'Toll-free service', true),
    ('1866', 'TOLL_FREE', 'IJ', 'Toll-free service', true),
    ('1877', 'TOLL_FREE', 'IJ', 'Toll-free service', true),
    ('1888', 'TOLL_FREE', 'IJ', 'Toll-free service', true),
    ('1900', 'PREMIUM', 'IJ', 'Premium rate service', true),
    ('1976', 'PREMIUM', 'IJ', 'Premium rate audiotext', true),
    ('1500', 'PERSONAL_COMM', 'IJ', 'Personal communication service', true),
    ('1700', 'IC_SERVICES', 'IJ', 'Interexchange carrier services', true),
    ('1710', 'GOVERNMENT', 'IJ', 'Government services', true),
    ('1720', 'SPECIAL_VOIP', 'IJ', 'Special/VoIP services', true)
ON CONFLICT (npa) DO NOTHING;
