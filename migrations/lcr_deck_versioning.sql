-- LCR Deck Versioning Migration
-- Adds version tracking, effective/end dates, and lazy loading support

-- Add version columns to vendor rate decks
ALTER TABLE vendor_rate_decks 
ADD COLUMN IF NOT EXISTS deck_version INTEGER NOT NULL DEFAULT 1,
ADD COLUMN IF NOT EXISTS end_date TIMESTAMP WITH TIME ZONE,
ADD COLUMN IF NOT EXISTS parent_deck_id INTEGER REFERENCES vendor_rate_decks(id),
ADD COLUMN IF NOT EXISTS effective_time TIME DEFAULT '00:00:00',
ADD COLUMN IF NOT EXISTS preload_minutes INTEGER DEFAULT 30,
ADD COLUMN IF NOT EXISTS loaded_at TIMESTAMP WITH TIME ZONE,
ADD COLUMN IF NOT EXISTS is_staged BOOLEAN DEFAULT false;

-- Add version columns to client rate decks  
ALTER TABLE client_rate_decks
ADD COLUMN IF NOT EXISTS deck_version INTEGER NOT NULL DEFAULT 1,
ADD COLUMN IF NOT EXISTS end_date TIMESTAMP WITH TIME ZONE,
ADD COLUMN IF NOT EXISTS parent_deck_id INTEGER REFERENCES client_rate_decks(id),
ADD COLUMN IF NOT EXISTS effective_time TIME DEFAULT '00:00:00',
ADD COLUMN IF NOT EXISTS preload_minutes INTEGER DEFAULT 30,
ADD COLUMN IF NOT EXISTS loaded_at TIMESTAMP WITH TIME ZONE,
ADD COLUMN IF NOT EXISTS is_staged BOOLEAN DEFAULT false;

-- Create unique constraint on deck identifier and version
ALTER TABLE vendor_rate_decks 
DROP CONSTRAINT IF EXISTS vendor_rate_decks_name_vendor_id_effective_date_key;

ALTER TABLE vendor_rate_decks
ADD CONSTRAINT vendor_deck_version_unique UNIQUE(vendor_id, name, deck_version);

ALTER TABLE client_rate_decks
DROP CONSTRAINT IF EXISTS client_rate_decks_name_client_id_effective_date_key;

ALTER TABLE client_rate_decks  
ADD CONSTRAINT client_deck_version_unique UNIQUE(client_id, name, deck_version);

-- Create indexes for efficient date-based queries
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

-- Function to get upcoming deck cutovers that need preloading
CREATE OR REPLACE FUNCTION get_decks_to_preload(
    p_lookahead_minutes INTEGER DEFAULT 60
) RETURNS TABLE (
    deck_type VARCHAR,
    deck_id INTEGER,
    cutover_date TIMESTAMP WITH TIME ZONE,
    preload_at TIMESTAMP WITH TIME ZONE
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        dcs.deck_type,
        dcs.new_deck_id,
        dcs.cutover_date,
        dcs.preload_at
    FROM deck_cutover_schedule dcs
    WHERE dcs.status IN ('scheduled', 'preloading')
      AND dcs.preload_at <= NOW() + (p_lookahead_minutes || ' minutes')::INTERVAL
      AND dcs.preload_at > NOW()
    ORDER BY dcs.preload_at;
END;
$$ LANGUAGE plpgsql;

-- View for active and upcoming decks
CREATE OR REPLACE VIEW active_vendor_decks AS
SELECT 
    vrd.*,
    CASE 
        WHEN vrd.effective_date > NOW() THEN 'future'
        WHEN vrd.end_date IS NULL OR vrd.end_date > NOW() THEN 'active'
        ELSE 'expired'
    END as deck_status,
    CASE
        WHEN vrd.is_staged AND vrd.effective_date <= NOW() + INTERVAL '1 hour' THEN true
        ELSE false
    END as needs_loading
FROM vendor_rate_decks vrd
WHERE vrd.active = true;

CREATE OR REPLACE VIEW active_client_decks AS
SELECT 
    crd.*,
    CASE 
        WHEN crd.effective_date > NOW() THEN 'future'
        WHEN crd.end_date IS NULL OR crd.end_date > NOW() THEN 'active'
        ELSE 'expired'
    END as deck_status,
    CASE
        WHEN crd.is_staged AND crd.effective_date <= NOW() + INTERVAL '1 hour' THEN true
        ELSE false
    END as needs_loading
FROM client_rate_decks crd
WHERE crd.active = true;

-- Function to schedule deck cutover
CREATE OR REPLACE FUNCTION schedule_deck_cutover(
    p_deck_type VARCHAR,
    p_current_deck_id INTEGER,
    p_new_deck_id INTEGER,
    p_cutover_date TIMESTAMP WITH TIME ZONE,
    p_preload_minutes INTEGER DEFAULT 30
) RETURNS INTEGER AS $$
DECLARE
    v_schedule_id INTEGER;
BEGIN
    INSERT INTO deck_cutover_schedule (
        deck_type,
        current_deck_id,
        new_deck_id,
        cutover_date,
        preload_at,
        status
    ) VALUES (
        p_deck_type,
        p_current_deck_id,
        p_new_deck_id,
        p_cutover_date,
        p_cutover_date - (p_preload_minutes || ' minutes')::INTERVAL,
        'scheduled'
    ) RETURNING id INTO v_schedule_id;
    
    RETURN v_schedule_id;
END;
$$ LANGUAGE plpgsql;

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