-- Complete LCR Schema with All Features
-- This is the comprehensive migration for a production-ready LCR system
-- Includes: deck versioning, soft deletion, safety checks, and cleanup functions

-- Rate deck types
CREATE TYPE rate_type AS ENUM ('LRN', 'DNIS');
CREATE TYPE route_type AS ENUM ('NANPA', 'A-Z', 'OTHER');
CREATE TYPE call_jurisdiction AS ENUM ('inter', 'intra', 'indeterminate', 'local');
CREATE TYPE international_jurisdiction AS ENUM ('EEA', 'ROW');

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
    deleted BOOLEAN DEFAULT false,
    deleted_at TIMESTAMP WITH TIME ZONE,
    last_used_at TIMESTAMP WITH TIME ZONE,
    usage_count BIGINT DEFAULT 0,
    is_protected BOOLEAN DEFAULT false,
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
    deleted BOOLEAN DEFAULT false,
    deleted_at TIMESTAMP WITH TIME ZONE,
    last_used_at TIMESTAMP WITH TIME ZONE,
    usage_count BIGINT DEFAULT 0,
    is_protected BOOLEAN DEFAULT false,
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
    local_rate DECIMAL(10, 7), -- Optional local rate (falls back to intra_rate)
    min_increment INTEGER NOT NULL DEFAULT 6, -- Minimum increment in seconds
    interval INTEGER NOT NULL DEFAULT 6, -- Billing interval in seconds
    setup_fee DECIMAL(10, 7) DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT vendor_nanpa_rates_unique UNIQUE(deck_id, code)
);

-- NANPA client rates (selling)
CREATE TABLE client_nanpa_rates (
    id SERIAL PRIMARY KEY,
    deck_id INTEGER NOT NULL REFERENCES client_rate_decks(id) ON DELETE CASCADE,
    code VARCHAR(20) NOT NULL, -- 1NPANXX or more specific
    inter_rate DECIMAL(10, 7) NOT NULL, -- Interstate rate
    intra_rate DECIMAL(10, 7) NOT NULL, -- Intrastate rate
    ij_rate DECIMAL(10, 7) NOT NULL, -- Indeterminate jurisdiction rate
    local_rate DECIMAL(10, 7), -- Optional local rate (falls back to intra_rate)
    min_increment INTEGER NOT NULL DEFAULT 6, -- Minimum increment in seconds
    interval INTEGER NOT NULL DEFAULT 6, -- Billing interval in seconds
    setup_fee DECIMAL(10, 7) DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT client_nanpa_rates_unique UNIQUE(deck_id, code)
);

-- International vendor rates (cost)
CREATE TABLE vendor_international_rates (
    id SERIAL PRIMARY KEY,
    deck_id INTEGER NOT NULL REFERENCES vendor_rate_decks(id) ON DELETE CASCADE,
    country_code VARCHAR(10) NOT NULL, -- Country prefix (e.g., "44", "49", "33")
    destination_code VARCHAR(20), -- Optional more specific code (e.g., "44207", "4920")
    destination_name VARCHAR(255) NOT NULL, -- "United Kingdom", "Germany Mobile", etc.
    jurisdiction international_jurisdiction NOT NULL, -- EEA or ROW
    rate DECIMAL(10, 7) NOT NULL, -- Single rate for international
    initial_increment INTEGER NOT NULL DEFAULT 30, -- Initial billing increment in seconds
    subsequent_increment INTEGER NOT NULL DEFAULT 6, -- Subsequent billing increment
    setup_fee DECIMAL(10, 7) DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT vendor_intl_rates_unique UNIQUE(deck_id, country_code, destination_code)
);

-- International client rates (selling)
CREATE TABLE client_international_rates (
    id SERIAL PRIMARY KEY,
    deck_id INTEGER NOT NULL REFERENCES client_rate_decks(id) ON DELETE CASCADE,
    country_code VARCHAR(10) NOT NULL, -- Country prefix (e.g., "44", "49", "33")
    destination_code VARCHAR(20), -- Optional more specific code (e.g., "44207", "4920")
    destination_name VARCHAR(255) NOT NULL, -- "United Kingdom", "Germany Mobile", etc.
    jurisdiction international_jurisdiction NOT NULL, -- EEA or ROW
    rate DECIMAL(10, 7) NOT NULL, -- Single rate for international
    initial_increment INTEGER NOT NULL DEFAULT 30, -- Initial billing increment in seconds
    subsequent_increment INTEGER NOT NULL DEFAULT 6, -- Subsequent billing increment
    setup_fee DECIMAL(10, 7) DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT client_intl_rates_unique UNIQUE(deck_id, country_code, destination_code)
);

-- Additional tables (trunks, routing, etc.)
CREATE TABLE egress_trunks (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    vendor_id INTEGER NOT NULL,
    host VARCHAR(255) NOT NULL,
    port INTEGER NOT NULL DEFAULT 5060,
    transport VARCHAR(10) NOT NULL DEFAULT 'UDP',
    capacity_limit INTEGER DEFAULT 1000,
    cps_limit DECIMAL(8, 2) DEFAULT 10.0,
    active BOOLEAN DEFAULT true,
    priority INTEGER DEFAULT 1,
    weight INTEGER DEFAULT 1,
    tech_prefix VARCHAR(20),
    supports_international BOOLEAN DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE ingress_trunks (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    client_id INTEGER NOT NULL,
    ip_address INET NOT NULL,
    capacity_limit INTEGER DEFAULT 1000,
    cps_limit DECIMAL(8, 2) DEFAULT 10.0,
    profit_protection BOOLEAN DEFAULT false,
    min_profit_margin DECIMAL(8, 7) DEFAULT 0.001,
    active BOOLEAN DEFAULT true,
    auth_username VARCHAR(255),
    auth_password VARCHAR(255),
    supports_international BOOLEAN DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE lcr_route_trunks (
    id SERIAL PRIMARY KEY,
    lcr_route_id INTEGER NOT NULL,
    egress_trunk_id INTEGER NOT NULL REFERENCES egress_trunks(id),
    vendor_deck_id INTEGER NOT NULL REFERENCES vendor_rate_decks(id),
    priority INTEGER DEFAULT 1,
    weight INTEGER DEFAULT 1,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE trunk_rate_associations (
    id SERIAL PRIMARY KEY,
    egress_trunk_id INTEGER REFERENCES egress_trunks(id),
    ingress_trunk_id INTEGER REFERENCES ingress_trunks(id),
    vendor_deck_id INTEGER REFERENCES vendor_rate_decks(id),
    client_deck_id INTEGER REFERENCES client_rate_decks(id),
    priority INTEGER DEFAULT 1,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE static_routes (
    id SERIAL PRIMARY KEY,
    ingress_trunk_id INTEGER REFERENCES ingress_trunks(id),
    egress_trunk_id INTEGER NOT NULL REFERENCES egress_trunks(id),
    pattern VARCHAR(255) NOT NULL, -- Regex pattern
    priority INTEGER DEFAULT 1,
    position VARCHAR(10) DEFAULT 'AFTER', -- BEFORE or AFTER
    description TEXT,
    active BOOLEAN DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE deck_cutover_schedule (
    id SERIAL PRIMARY KEY,
    deck_type VARCHAR(10) NOT NULL, -- 'vendor' or 'client'
    current_deck_id INTEGER NOT NULL,
    new_deck_id INTEGER NOT NULL,
    cutover_date TIMESTAMP WITH TIME ZONE NOT NULL,
    preload_at TIMESTAMP WITH TIME ZONE NOT NULL,
    status VARCHAR(20) DEFAULT 'scheduled', -- scheduled, preloading, preloaded, active, completed
    preloaded_at TIMESTAMP WITH TIME ZONE,
    activated_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE deck_load_history (
    id SERIAL PRIMARY KEY,
    deck_type VARCHAR(10) NOT NULL,
    deck_id INTEGER NOT NULL,
    deck_version INTEGER NOT NULL,
    effective_date TIMESTAMP WITH TIME ZONE NOT NULL,
    rate_count INTEGER DEFAULT 0,
    loaded_by VARCHAR(255),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Archive tables for cleanup
CREATE TABLE vendor_nanpa_rates_archive (
    LIKE vendor_nanpa_rates INCLUDING ALL,
    archived_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE client_nanpa_rates_archive (
    LIKE client_nanpa_rates INCLUDING ALL,
    archived_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Performance indexes
CREATE INDEX idx_vendor_nanpa_code ON vendor_nanpa_rates(code);
CREATE INDEX idx_vendor_nanpa_deck ON vendor_nanpa_rates(deck_id);
CREATE INDEX idx_client_nanpa_code ON client_nanpa_rates(code);
CREATE INDEX idx_client_nanpa_deck ON client_nanpa_rates(deck_id);
-- International rate indexes for longest-to-shortest matching
CREATE INDEX idx_vendor_intl_prefix ON vendor_international_rates(deck_id, country_code, destination_code);
CREATE INDEX idx_vendor_intl_country ON vendor_international_rates(country_code);
CREATE INDEX idx_client_intl_prefix ON client_international_rates(deck_id, country_code, destination_code);
CREATE INDEX idx_client_intl_country ON client_international_rates(country_code);
CREATE INDEX idx_vendor_decks_effective ON vendor_rate_decks(effective_date, end_date);
CREATE INDEX idx_client_decks_effective ON client_rate_decks(effective_date, end_date);
CREATE INDEX idx_vendor_rate_decks_active ON vendor_rate_decks(active) WHERE deleted = false;
CREATE INDEX idx_client_rate_decks_active ON client_rate_decks(active) WHERE deleted = false;
CREATE INDEX idx_vendor_rate_decks_parent ON vendor_rate_decks(parent_deck_id) WHERE deleted = false;
CREATE INDEX idx_client_rate_decks_parent ON client_rate_decks(parent_deck_id) WHERE deleted = false;
CREATE INDEX idx_vendor_rate_decks_deleted_at ON vendor_rate_decks(deleted_at) WHERE deleted = true;
CREATE INDEX idx_client_rate_decks_deleted_at ON client_rate_decks(deleted_at) WHERE deleted = true;

-- Function to automatically set end_date when loading new version
CREATE OR REPLACE FUNCTION update_deck_end_dates()
RETURNS TRIGGER AS $$
BEGIN
    -- Only update if this is a new deck version with a parent
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
CREATE TRIGGER vendor_deck_end_date_trigger
    AFTER INSERT ON vendor_rate_decks
    FOR EACH ROW
    EXECUTE FUNCTION update_deck_end_dates();

CREATE TRIGGER client_deck_end_date_trigger
    AFTER INSERT ON client_rate_decks
    FOR EACH ROW
    EXECUTE FUNCTION update_deck_end_dates();

-- Include all safety and cleanup functions from previous migrations
\i add_soft_deletion.sql
\i add_deck_safety_cleanup.sql

-- Comments for documentation
COMMENT ON TABLE vendor_rate_decks IS 'Vendor rate decks with versioning and soft deletion support';
COMMENT ON TABLE client_rate_decks IS 'Client rate decks with versioning and soft deletion support';
COMMENT ON COLUMN vendor_rate_decks.local_rate IS 'Optional local rate - falls back to intra_rate when NULL';
COMMENT ON COLUMN client_rate_decks.local_rate IS 'Optional local rate - falls back to intra_rate when NULL';
COMMENT ON COLUMN vendor_rate_decks.deleted IS 'Soft deletion flag - prevents ID reuse issues';
COMMENT ON COLUMN vendor_rate_decks.is_protected IS 'Prevents deletion of decks currently in active routing';