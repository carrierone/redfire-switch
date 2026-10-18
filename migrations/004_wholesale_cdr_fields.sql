-- Wholesale CDR fields: LRN dip result, LERG jurisdiction, and independent
-- client (sell) / vendor (buy) rate-deck costing, populated by
-- class4_b2bua.rs's apply_lcr_route_costs at call teardown. Previously the
-- LCR engine computed these at call setup and they were discarded before
-- any CDR was ever persisted (CDR persistence itself was unwired until now).
ALTER TABLE call_detail_records
    ADD COLUMN IF NOT EXISTS lrn VARCHAR(20),
    ADD COLUMN IF NOT EXISTS jurisdiction VARCHAR(20),
    ADD COLUMN IF NOT EXISTS client_rate DECIMAL(10,6),
    ADD COLUMN IF NOT EXISTS client_billed_seconds BIGINT,
    ADD COLUMN IF NOT EXISTS client_cost DECIMAL(10,6),
    ADD COLUMN IF NOT EXISTS client_rate_deck_version VARCHAR(50),
    ADD COLUMN IF NOT EXISTS vendor_rate DECIMAL(10,6),
    ADD COLUMN IF NOT EXISTS vendor_billed_seconds BIGINT,
    ADD COLUMN IF NOT EXISTS vendor_cost DECIMAL(10,6),
    ADD COLUMN IF NOT EXISTS vendor_rate_deck_version VARCHAR(50);

-- Idempotency key for the switch -> boss webhook sender (external_id).
ALTER TABLE call_detail_records
    ADD COLUMN IF NOT EXISTS boss_sync_status VARCHAR(20) DEFAULT 'PENDING',
    ADD COLUMN IF NOT EXISTS boss_synced_at TIMESTAMP WITH TIME ZONE;

CREATE INDEX IF NOT EXISTS idx_cdr_boss_sync_status ON call_detail_records(boss_sync_status)
    WHERE boss_sync_status = 'PENDING';

-- 001_initial_schema.sql only created monthly partitions for 2025 (through
-- 2025-12-31) -- any insert for a 2026+ call date hits "no partition found"
-- and fails outright. Add a DEFAULT partition so inserts never fail
-- regardless of date; this also silently absorbs anything that lands outside
-- the explicit monthly partitions going forward (worth watching via
-- pg_stat for DEFAULT-partition growth, since in that case explicit
-- partitions for the actual date range aren't being created).
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_inherits
        JOIN pg_class child ON pg_inherits.inhrelid = child.oid
        WHERE child.relname = 'call_detail_records_default'
    ) THEN
        EXECUTE 'CREATE TABLE call_detail_records_default PARTITION OF call_detail_records DEFAULT';
    END IF;
END $$;
