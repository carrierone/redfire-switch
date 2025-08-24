-- Add soft deletion support to rate decks
-- This prevents ID reuse issues while allowing logical "deletion"

-- Add deleted flag to vendor_rate_decks
ALTER TABLE vendor_rate_decks ADD COLUMN deleted BOOLEAN DEFAULT false;
ALTER TABLE vendor_rate_decks ADD COLUMN deleted_at TIMESTAMP WITH TIME ZONE;

-- Add deleted flag to client_rate_decks  
ALTER TABLE client_rate_decks ADD COLUMN deleted BOOLEAN DEFAULT false;
ALTER TABLE client_rate_decks ADD COLUMN deleted_at TIMESTAMP WITH TIME ZONE;

-- Create indexes for performance on active decks
CREATE INDEX idx_vendor_rate_decks_active ON vendor_rate_decks(active) WHERE deleted = false;
CREATE INDEX idx_client_rate_decks_active ON client_rate_decks(active) WHERE deleted = false;

-- Create indexes for version chains
CREATE INDEX idx_vendor_rate_decks_parent ON vendor_rate_decks(parent_deck_id) WHERE deleted = false;
CREATE INDEX idx_client_rate_decks_parent ON client_rate_decks(parent_deck_id) WHERE deleted = false;

-- Function to safely "delete" a deck (soft delete)
CREATE OR REPLACE FUNCTION soft_delete_vendor_deck(deck_id_to_delete INTEGER)
RETURNS BOOLEAN AS $$
DECLARE
    has_children BOOLEAN;
    child_count INTEGER;
BEGIN
    -- Check if this deck has any children (newer versions)
    SELECT COUNT(*) INTO child_count
    FROM vendor_rate_decks 
    WHERE parent_deck_id = deck_id_to_delete AND deleted = false;
    
    has_children := child_count > 0;
    
    -- If it has children, we can only soft delete if all children are also being deleted
    -- For safety, we'll reject deletion of parent decks
    IF has_children THEN
        RAISE EXCEPTION 'Cannot delete deck ID % - it has % active child versions. Delete children first.', 
                        deck_id_to_delete, child_count;
        RETURN false;
    END IF;
    
    -- Safe to soft delete
    UPDATE vendor_rate_decks 
    SET deleted = true, 
        deleted_at = NOW(),
        active = false
    WHERE id = deck_id_to_delete AND deleted = false;
    
    -- Check if update affected any rows
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Deck ID % not found or already deleted', deck_id_to_delete;
        RETURN false;
    END IF;
    
    RETURN true;
END;
$$ LANGUAGE plpgsql;

-- Similar function for client decks
CREATE OR REPLACE FUNCTION soft_delete_client_deck(deck_id_to_delete INTEGER)
RETURNS BOOLEAN AS $$
DECLARE
    has_children BOOLEAN;
    child_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO child_count
    FROM client_rate_decks 
    WHERE parent_deck_id = deck_id_to_delete AND deleted = false;
    
    has_children := child_count > 0;
    
    IF has_children THEN
        RAISE EXCEPTION 'Cannot delete deck ID % - it has % active child versions. Delete children first.', 
                        deck_id_to_delete, child_count;
        RETURN false;
    END IF;
    
    UPDATE client_rate_decks 
    SET deleted = true, 
        deleted_at = NOW(),
        active = false
    WHERE id = deck_id_to_delete AND deleted = false;
    
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Deck ID % not found or already deleted', deck_id_to_delete;
        RETURN false;
    END IF;
    
    RETURN true;
END;
$$ LANGUAGE plpgsql;

-- Function to safely delete an entire version chain (newest to oldest)
CREATE OR REPLACE FUNCTION soft_delete_deck_chain(deck_name TEXT, owner_id INTEGER, deck_type TEXT)
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER := 0;
    deck_record RECORD;
BEGIN
    -- Delete in reverse order (newest versions first) to respect FK constraints
    IF deck_type = 'vendor' THEN
        FOR deck_record IN 
            SELECT id, deck_version 
            FROM vendor_rate_decks 
            WHERE name = deck_name AND vendor_id = owner_id AND deleted = false
            ORDER BY deck_version DESC
        LOOP
            PERFORM soft_delete_vendor_deck(deck_record.id);
            deleted_count := deleted_count + 1;
        END LOOP;
    ELSIF deck_type = 'client' THEN
        FOR deck_record IN 
            SELECT id, deck_version 
            FROM client_rate_decks 
            WHERE name = deck_name AND client_id = owner_id AND deleted = false
            ORDER BY deck_version DESC
        LOOP
            PERFORM soft_delete_client_deck(deck_record.id);
            deleted_count := deleted_count + 1;
        END LOOP;
    ELSE
        RAISE EXCEPTION 'Invalid deck_type: %. Must be ''vendor'' or ''client''', deck_type;
    END IF;
    
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- Update views to exclude soft-deleted decks by default
CREATE OR REPLACE VIEW active_vendor_rate_decks AS
SELECT * FROM vendor_rate_decks WHERE deleted = false;

CREATE OR REPLACE VIEW active_client_rate_decks AS
SELECT * FROM client_rate_decks WHERE deleted = false;

-- Add constraint to prevent hard deletion of decks with children
-- This adds an extra layer of protection beyond FK constraints
CREATE OR REPLACE FUNCTION prevent_parent_deletion()
RETURNS TRIGGER AS $$
BEGIN
    -- This trigger fires on DELETE
    IF EXISTS (
        SELECT 1 FROM vendor_rate_decks 
        WHERE parent_deck_id = OLD.id AND deleted = false
    ) OR EXISTS (
        SELECT 1 FROM client_rate_decks 
        WHERE parent_deck_id = OLD.id AND deleted = false
    ) THEN
        RAISE EXCEPTION 'Cannot hard delete deck ID % - use soft_delete functions instead', OLD.id;
    END IF;
    
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

-- Apply the trigger to both tables
CREATE TRIGGER prevent_vendor_parent_deletion
    BEFORE DELETE ON vendor_rate_decks
    FOR EACH ROW
    EXECUTE FUNCTION prevent_parent_deletion();

CREATE TRIGGER prevent_client_parent_deletion
    BEFORE DELETE ON client_rate_decks
    FOR EACH ROW
    EXECUTE FUNCTION prevent_parent_deletion();

-- Add comments for documentation
COMMENT ON COLUMN vendor_rate_decks.deleted IS 'Soft deletion flag - prevents ID reuse issues';
COMMENT ON COLUMN vendor_rate_decks.deleted_at IS 'Timestamp when deck was soft deleted';
COMMENT ON FUNCTION soft_delete_vendor_deck(INTEGER) IS 'Safely soft-delete a vendor deck, preventing ID reuse';
COMMENT ON FUNCTION soft_delete_deck_chain(TEXT, INTEGER, TEXT) IS 'Soft-delete entire version chain for a deck name';