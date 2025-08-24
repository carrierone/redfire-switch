-- Enhanced deck management for space cleanup and operational safety
-- Addresses: 1) Safe cleanup of old deck data, 2) Protection of active routing decks

-- Add usage tracking to decks
ALTER TABLE vendor_rate_decks ADD COLUMN last_used_at TIMESTAMP WITH TIME ZONE;
ALTER TABLE vendor_rate_decks ADD COLUMN usage_count BIGINT DEFAULT 0;
ALTER TABLE vendor_rate_decks ADD COLUMN is_protected BOOLEAN DEFAULT false;

ALTER TABLE client_rate_decks ADD COLUMN last_used_at TIMESTAMP WITH TIME ZONE;
ALTER TABLE client_rate_decks ADD COLUMN usage_count BIGINT DEFAULT 0;
ALTER TABLE client_rate_decks ADD COLUMN is_protected BOOLEAN DEFAULT false;

-- Create archived rates tables for historical data
CREATE TABLE vendor_nanpa_rates_archive (
    LIKE vendor_nanpa_rates INCLUDING ALL
);

CREATE TABLE client_nanpa_rates_archive (
    LIKE client_nanpa_rates INCLUDING ALL
);

-- Add archival timestamps
ALTER TABLE vendor_nanpa_rates_archive ADD COLUMN archived_at TIMESTAMP WITH TIME ZONE DEFAULT NOW();
ALTER TABLE client_nanpa_rates_archive ADD COLUMN archived_at TIMESTAMP WITH TIME ZONE DEFAULT NOW();

-- Function to check if a deck is actively being used for routing
CREATE OR REPLACE FUNCTION is_deck_actively_used(deck_id INTEGER, deck_type TEXT)
RETURNS BOOLEAN AS $$
DECLARE
    trunk_count INTEGER;
    active_call_count INTEGER;
BEGIN
    -- Check if deck is associated with any active trunks
    IF deck_type = 'vendor' THEN
        SELECT COUNT(*) INTO trunk_count
        FROM lcr_route_trunks lrt
        JOIN egress_trunks et ON et.id = lrt.egress_trunk_id
        WHERE lrt.vendor_deck_id = deck_id AND et.active = true;
    ELSIF deck_type = 'client' THEN
        SELECT COUNT(*) INTO trunk_count
        FROM trunk_rate_associations tra
        JOIN ingress_trunks it ON it.id = tra.ingress_trunk_id
        WHERE tra.client_deck_id = deck_id AND it.active = true;
    ELSE
        RAISE EXCEPTION 'Invalid deck_type: %', deck_type;
    END IF;
    
    -- For now, we'll assume no active calls check (would need call session tracking)
    -- In production, this would query active call sessions
    active_call_count := 0;
    
    RETURN trunk_count > 0 OR active_call_count > 0;
END;
$$ LANGUAGE plpgsql;

-- Safe deletion function with active usage check
CREATE OR REPLACE FUNCTION safe_delete_vendor_deck(deck_id_to_delete INTEGER, force BOOLEAN DEFAULT false)
RETURNS TEXT AS $$
DECLARE
    deck_name TEXT;
    deck_version INTEGER;
    is_active BOOLEAN;
    child_count INTEGER;
    result TEXT;
BEGIN
    -- Get deck info
    SELECT name, deck_version, active INTO deck_name, deck_version, is_active
    FROM vendor_rate_decks 
    WHERE id = deck_id_to_delete AND deleted = false;
    
    IF NOT FOUND THEN
        RETURN 'ERROR: Deck not found or already deleted';
    END IF;
    
    -- Check for children
    SELECT COUNT(*) INTO child_count
    FROM vendor_rate_decks 
    WHERE parent_deck_id = deck_id_to_delete AND deleted = false;
    
    IF child_count > 0 THEN
        RETURN format('ERROR: Cannot delete deck %s v%s - has %s active child versions', 
                     deck_name, deck_version, child_count);
    END IF;
    
    -- Check if actively used in routing
    IF is_deck_actively_used(deck_id_to_delete, 'vendor') THEN
        IF NOT force THEN
            RETURN format('ERROR: Deck %s v%s is actively used in routing. Use force=true to override (DANGEROUS)', 
                         deck_name, deck_version);
        ELSE
            result := format('WARNING: Force deleted actively used deck %s v%s', deck_name, deck_version);
        END IF;
    END IF;
    
    -- Mark as protected if it's currently active
    IF is_active THEN
        UPDATE vendor_rate_decks 
        SET is_protected = true 
        WHERE id = deck_id_to_delete;
        
        IF NOT force THEN
            RETURN format('ERROR: Deck %s v%s is currently active. Protected from deletion. Use force=true to override', 
                         deck_name, deck_version);
        END IF;
    END IF;
    
    -- Perform soft deletion
    UPDATE vendor_rate_decks 
    SET deleted = true, 
        deleted_at = NOW(),
        active = false
    WHERE id = deck_id_to_delete;
    
    IF result IS NULL THEN
        result := format('SUCCESS: Soft deleted deck %s v%s', deck_name, deck_version);
    END IF;
    
    RETURN result;
END;
$$ LANGUAGE plpgsql;

-- Similar function for client decks
CREATE OR REPLACE FUNCTION safe_delete_client_deck(deck_id_to_delete INTEGER, force BOOLEAN DEFAULT false)
RETURNS TEXT AS $$
DECLARE
    deck_name TEXT;
    deck_version INTEGER;
    is_active BOOLEAN;
    child_count INTEGER;
    result TEXT;
BEGIN
    SELECT name, deck_version, active INTO deck_name, deck_version, is_active
    FROM client_rate_decks 
    WHERE id = deck_id_to_delete AND deleted = false;
    
    IF NOT FOUND THEN
        RETURN 'ERROR: Deck not found or already deleted';
    END IF;
    
    SELECT COUNT(*) INTO child_count
    FROM client_rate_decks 
    WHERE parent_deck_id = deck_id_to_delete AND deleted = false;
    
    IF child_count > 0 THEN
        RETURN format('ERROR: Cannot delete deck %s v%s - has %s active child versions', 
                     deck_name, deck_version, child_count);
    END IF;
    
    IF is_deck_actively_used(deck_id_to_delete, 'client') THEN
        IF NOT force THEN
            RETURN format('ERROR: Deck %s v%s is actively used in routing. Use force=true to override (DANGEROUS)', 
                         deck_name, deck_version);
        ELSE
            result := format('WARNING: Force deleted actively used deck %s v%s', deck_name, deck_version);
        END IF;
    END IF;
    
    IF is_active THEN
        UPDATE client_rate_decks 
        SET is_protected = true 
        WHERE id = deck_id_to_delete;
        
        IF NOT force THEN
            RETURN format('ERROR: Deck %s v%s is currently active. Protected from deletion. Use force=true to override', 
                         deck_name, deck_version);
        END IF;
    END IF;
    
    UPDATE client_rate_decks 
    SET deleted = true, 
        deleted_at = NOW(),
        active = false
    WHERE id = deck_id_to_delete;
    
    IF result IS NULL THEN
        result := format('SUCCESS: Soft deleted deck %s v%s', deck_name, deck_version);
    END IF;
    
    RETURN result;
END;
$$ LANGUAGE plpgsql;

-- Archive and hard delete old deck data to free space
CREATE OR REPLACE FUNCTION archive_and_cleanup_deck_data(
    older_than_days INTEGER DEFAULT 90,
    dry_run BOOLEAN DEFAULT true
)
RETURNS TABLE(
    action TEXT,
    deck_type TEXT,
    deck_id INTEGER,
    deck_name TEXT,
    deck_version INTEGER,
    size_freed_mb NUMERIC
) AS $$
DECLARE
    cleanup_date TIMESTAMP WITH TIME ZONE;
    vendor_record RECORD;
    client_record RECORD;
    rate_count BIGINT;
    size_estimate NUMERIC;
BEGIN
    cleanup_date := NOW() - INTERVAL '1 day' * older_than_days;
    
    -- Process vendor decks
    FOR vendor_record IN 
        SELECT vrd.id, vrd.name, vrd.deck_version, vrd.deleted_at
        FROM vendor_rate_decks vrd
        WHERE vrd.deleted = true 
          AND vrd.deleted_at < cleanup_date
          AND vrd.is_protected = false
        ORDER BY vrd.deleted_at ASC
    LOOP
        -- Count rates to estimate size
        SELECT COUNT(*) INTO rate_count
        FROM vendor_nanpa_rates 
        WHERE deck_id = vendor_record.id;
        
        -- Rough estimate: 100 bytes per rate record
        size_estimate := rate_count * 100.0 / 1024.0 / 1024.0;
        
        IF NOT dry_run THEN
            -- Move rates to archive
            INSERT INTO vendor_nanpa_rates_archive 
            SELECT *, NOW() FROM vendor_nanpa_rates WHERE deck_id = vendor_record.id;
            
            -- Delete rates
            DELETE FROM vendor_nanpa_rates WHERE deck_id = vendor_record.id;
            
            -- Hard delete deck
            DELETE FROM vendor_rate_decks WHERE id = vendor_record.id;
        END IF;
        
        RETURN QUERY SELECT 
            CASE WHEN dry_run THEN 'DRY_RUN' ELSE 'ARCHIVED_AND_DELETED' END,
            'vendor'::TEXT,
            vendor_record.id,
            vendor_record.name,
            vendor_record.deck_version,
            size_estimate;
    END LOOP;
    
    -- Process client decks
    FOR client_record IN 
        SELECT crd.id, crd.name, crd.deck_version, crd.deleted_at
        FROM client_rate_decks crd
        WHERE crd.deleted = true 
          AND crd.deleted_at < cleanup_date
          AND crd.is_protected = false
        ORDER BY crd.deleted_at ASC
    LOOP
        SELECT COUNT(*) INTO rate_count
        FROM client_nanpa_rates 
        WHERE deck_id = client_record.id;
        
        size_estimate := rate_count * 100.0 / 1024.0 / 1024.0;
        
        IF NOT dry_run THEN
            INSERT INTO client_nanpa_rates_archive 
            SELECT *, NOW() FROM client_nanpa_rates WHERE deck_id = client_record.id;
            
            DELETE FROM client_nanpa_rates WHERE deck_id = client_record.id;
            DELETE FROM client_rate_decks WHERE id = client_record.id;
        END IF;
        
        RETURN QUERY SELECT 
            CASE WHEN dry_run THEN 'DRY_RUN' ELSE 'ARCHIVED_AND_DELETED' END,
            'client'::TEXT,
            client_record.id,
            client_record.name,
            client_record.deck_version,
            size_estimate;
    END LOOP;
END;
$$ LANGUAGE plpgsql;

-- Function to release a deck from trunk routing (with confirmation)
CREATE OR REPLACE FUNCTION release_deck_from_routing(
    deck_id INTEGER, 
    deck_type TEXT,
    confirm_token TEXT DEFAULT NULL
)
RETURNS TEXT AS $$
DECLARE
    expected_token TEXT;
    deck_info TEXT;
    trunk_count INTEGER;
BEGIN
    -- Get deck info for confirmation
    IF deck_type = 'vendor' THEN
        SELECT format('%s v%s (ID: %s)', name, deck_version, id) INTO deck_info
        FROM vendor_rate_decks WHERE id = deck_id;
    ELSIF deck_type = 'client' THEN
        SELECT format('%s v%s (ID: %s)', name, deck_version, id) INTO deck_info
        FROM client_rate_decks WHERE id = deck_id;
    ELSE
        RETURN 'ERROR: Invalid deck_type';
    END IF;
    
    IF deck_info IS NULL THEN
        RETURN 'ERROR: Deck not found';
    END IF;
    
    -- Generate expected confirmation token (simple hash of deck info)
    expected_token := 'CONFIRM_' || upper(substring(md5(deck_info) from 1 for 8));
    
    -- If no token provided, return the required token
    IF confirm_token IS NULL THEN
        RETURN format('To release %s deck %s from routing, provide confirmation token: %s', 
                     deck_type, deck_info, expected_token);
    END IF;
    
    -- Validate confirmation token
    IF confirm_token != expected_token THEN
        RETURN format('ERROR: Invalid confirmation token. Expected: %s', expected_token);
    END IF;
    
    -- Remove from trunk routing
    IF deck_type = 'vendor' THEN
        DELETE FROM lcr_route_trunks WHERE vendor_deck_id = deck_id;
        GET DIAGNOSTICS trunk_count = ROW_COUNT;
    ELSIF deck_type = 'client' THEN
        DELETE FROM trunk_rate_associations WHERE client_deck_id = deck_id;
        GET DIAGNOSTICS trunk_count = ROW_COUNT;
    END IF;
    
    -- Mark deck as no longer protected since it's not in routing
    IF deck_type = 'vendor' THEN
        UPDATE vendor_rate_decks SET is_protected = false WHERE id = deck_id;
    ELSIF deck_type = 'client' THEN
        UPDATE client_rate_decks SET is_protected = false WHERE id = deck_id;
    END IF;
    
    RETURN format('SUCCESS: Released %s deck %s from %s trunk associations', 
                 deck_type, deck_info, trunk_count);
END;
$$ LANGUAGE plpgsql;

-- Create indexes for performance
CREATE INDEX idx_vendor_rate_decks_deleted_at ON vendor_rate_decks(deleted_at) WHERE deleted = true;
CREATE INDEX idx_client_rate_decks_deleted_at ON client_rate_decks(deleted_at) WHERE deleted = true;
CREATE INDEX idx_vendor_rate_decks_protected ON vendor_rate_decks(is_protected) WHERE is_protected = true;
CREATE INDEX idx_client_rate_decks_protected ON client_rate_decks(is_protected) WHERE is_protected = true;

-- Add helpful comments
COMMENT ON FUNCTION safe_delete_vendor_deck(INTEGER, BOOLEAN) IS 'Safely delete vendor deck with active usage checks and force option';
COMMENT ON FUNCTION archive_and_cleanup_deck_data(INTEGER, BOOLEAN) IS 'Archive old deleted decks and free disk space';
COMMENT ON FUNCTION release_deck_from_routing(INTEGER, TEXT, TEXT) IS 'Remove deck from trunk routing with confirmation token';
COMMENT ON COLUMN vendor_rate_decks.is_protected IS 'Prevents deletion of decks currently in active routing';
COMMENT ON COLUMN vendor_rate_decks.last_used_at IS 'Timestamp of last routing usage (updated by routing engine)';