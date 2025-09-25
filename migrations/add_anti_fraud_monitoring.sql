-- Add anti-fraud monitoring fields to ingress trunks
-- This supports ECPA-compliant call content monitoring for fraud detection

-- Add anti-fraud monitoring configuration to ingress trunks
ALTER TABLE ingress_trunks ADD COLUMN anti_fraud_monitoring_enabled BOOLEAN DEFAULT false;
ALTER TABLE ingress_trunks ADD COLUMN monitoring_sample_percentage DECIMAL(5,2) DEFAULT 0.0; -- Percentage of calls to monitor (0.0-100.0)
ALTER TABLE ingress_trunks ADD COLUMN legal_authorization_reference VARCHAR(255); -- Legal basis for monitoring
ALTER TABLE ingress_trunks ADD COLUMN ecpa_compliance_enabled BOOLEAN DEFAULT true; -- ECPA compliance safeguards

-- Storage type enumeration for recordings
CREATE TYPE storage_type AS ENUM ('memory', 'disk');

-- Call recording storage and transcription tables
CREATE TABLE call_recordings (
    id SERIAL PRIMARY KEY,
    call_id VARCHAR(255) NOT NULL,
    ingress_trunk_id INTEGER NOT NULL REFERENCES ingress_trunks(id) ON DELETE CASCADE,
    session_id VARCHAR(255) NOT NULL,
    recording_path VARCHAR(512) NOT NULL, -- Path to audio file
    storage_type storage_type NOT NULL DEFAULT 'memory', -- Storage location type
    file_size_bytes BIGINT NOT NULL,
    duration_seconds INTEGER NOT NULL,
    sample_rate INTEGER NOT NULL DEFAULT 8000,
    channels INTEGER NOT NULL DEFAULT 1,
    codec VARCHAR(20) NOT NULL DEFAULT 'PCMU',
    recorded_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    processed_at TIMESTAMP WITH TIME ZONE,
    archived_at TIMESTAMP WITH TIME ZONE,
    retention_expires_at TIMESTAMP WITH TIME ZONE,
    legal_hold BOOLEAN DEFAULT false, -- Flag for legal hold preventing deletion
    legal_authorization_ref VARCHAR(255) -- Reference to legal authorization document
);

-- ASR transcriptions with banned word analysis
CREATE TABLE call_transcriptions (
    id SERIAL PRIMARY KEY,
    recording_id INTEGER NOT NULL REFERENCES call_recordings(id) ON DELETE CASCADE,
    transcription_text TEXT NOT NULL,
    confidence_score DECIMAL(5,4), -- ASR confidence (0.0-1.0)
    language_detected VARCHAR(10),
    processing_engine VARCHAR(50) DEFAULT 'vosk',
    banned_words_detected INTEGER DEFAULT 0,
    banned_words_list TEXT[], -- Array of detected banned words
    risk_score DECIMAL(5,2) DEFAULT 0.0, -- Calculated fraud risk score (0.0-100.0)
    requires_review BOOLEAN DEFAULT false,
    reviewed_by VARCHAR(255),
    reviewed_at TIMESTAMP WITH TIME ZONE,
    review_notes TEXT,
    transcribed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Banned words configuration
CREATE TABLE banned_words_config (
    id SERIAL PRIMARY KEY,
    word_pattern VARCHAR(255) NOT NULL, -- Word or regex pattern
    category VARCHAR(100) NOT NULL, -- e.g., 'fraud', 'harassment', 'threat'
    risk_weight DECIMAL(5,2) DEFAULT 1.0, -- Weight factor for risk calculation
    case_sensitive BOOLEAN DEFAULT false,
    is_regex BOOLEAN DEFAULT false,
    enabled BOOLEAN DEFAULT true,
    description TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Anti-fraud monitoring events and alerts
CREATE TABLE anti_fraud_events (
    id SERIAL PRIMARY KEY,
    event_type VARCHAR(50) NOT NULL, -- 'BANNED_WORD_DETECTED', 'HIGH_RISK_CALL', 'THRESHOLD_EXCEEDED'
    call_id VARCHAR(255) NOT NULL,
    recording_id INTEGER REFERENCES call_recordings(id) ON DELETE SET NULL,
    transcription_id INTEGER REFERENCES call_transcriptions(id) ON DELETE SET NULL,
    ingress_trunk_id INTEGER NOT NULL REFERENCES ingress_trunks(id) ON DELETE CASCADE,
    risk_score DECIMAL(5,2) NOT NULL,
    details JSONB, -- Additional event details
    alert_sent BOOLEAN DEFAULT false,
    alert_sent_at TIMESTAMP WITH TIME ZONE,
    acknowledged_by VARCHAR(255),
    acknowledged_at TIMESTAMP WITH TIME ZONE,
    resolution_notes TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Monitoring statistics and metrics
CREATE TABLE monitoring_statistics (
    id SERIAL PRIMARY KEY,
    ingress_trunk_id INTEGER NOT NULL REFERENCES ingress_trunks(id) ON DELETE CASCADE,
    date_recorded DATE NOT NULL DEFAULT CURRENT_DATE,
    total_calls BIGINT DEFAULT 0,
    monitored_calls BIGINT DEFAULT 0,
    recordings_processed BIGINT DEFAULT 0,
    banned_words_detected BIGINT DEFAULT 0,
    high_risk_calls BIGINT DEFAULT 0,
    alerts_generated BIGINT DEFAULT 0,
    average_risk_score DECIMAL(5,2) DEFAULT 0.0,
    processing_time_ms_avg BIGINT DEFAULT 0,
    storage_used_bytes BIGINT DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT monitoring_stats_unique UNIQUE(ingress_trunk_id, date_recorded)
);

-- Insert default banned words categories for common fraud patterns
INSERT INTO banned_words_config (word_pattern, category, risk_weight, case_sensitive, description) VALUES
-- Financial fraud keywords
('credit card', 'financial_fraud', 8.0, false, 'Credit card related fraud'),
('bank account', 'financial_fraud', 8.0, false, 'Bank account fraud'),
('social security', 'financial_fraud', 9.0, false, 'Social security fraud'),
('wire transfer', 'financial_fraud', 7.0, false, 'Wire transfer fraud'),
('bitcoin', 'financial_fraud', 6.0, false, 'Cryptocurrency fraud'),
('urgent payment', 'financial_fraud', 7.5, false, 'Urgent payment scams'),

-- Identity theft
('date of birth', 'identity_theft', 8.5, false, 'Identity information harvesting'),
('mothers maiden', 'identity_theft', 9.0, false, 'Security question harvesting'),
('pin number', 'identity_theft', 9.5, false, 'PIN harvesting'),
('password', 'identity_theft', 8.0, false, 'Password harvesting'),

-- Threatening language
('bomb', 'threat', 10.0, false, 'Bomb threat'),
('kill', 'threat', 9.5, false, 'Death threat'),
('hurt', 'threat', 7.0, false, 'Threat of violence'),

-- Robocall indicators
('press 1', 'robocall', 6.0, false, 'Automated call indicator'),
('this is your final notice', 'robocall', 7.0, false, 'Final notice scam'),
('you have won', 'robocall', 6.5, false, 'Prize scam'),
('congratulations', 'robocall', 5.0, false, 'Prize/lottery scam');

-- Create trigger to update monitoring statistics
CREATE OR REPLACE FUNCTION update_monitoring_stats()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        -- Update daily statistics when new recordings or transcriptions are created
        IF TG_TABLE_NAME = 'call_recordings' THEN
            INSERT INTO monitoring_statistics (ingress_trunk_id, total_calls, monitored_calls)
            VALUES (NEW.ingress_trunk_id, 1, 1)
            ON CONFLICT (ingress_trunk_id, date_recorded)
            DO UPDATE SET
                monitored_calls = monitoring_statistics.monitored_calls + 1,
                updated_at = NOW();
        ELSIF TG_TABLE_NAME = 'call_transcriptions' THEN
            INSERT INTO monitoring_statistics (ingress_trunk_id, recordings_processed, banned_words_detected)
            SELECT r.ingress_trunk_id, 1, NEW.banned_words_detected
            FROM call_recordings r WHERE r.id = NEW.recording_id
            ON CONFLICT (ingress_trunk_id, date_recorded)
            DO UPDATE SET
                recordings_processed = monitoring_statistics.recordings_processed + 1,
                banned_words_detected = monitoring_statistics.banned_words_detected + NEW.banned_words_detected,
                updated_at = NOW();
        ELSIF TG_TABLE_NAME = 'anti_fraud_events' THEN
            INSERT INTO monitoring_statistics (ingress_trunk_id, alerts_generated)
            VALUES (NEW.ingress_trunk_id, 1)
            ON CONFLICT (ingress_trunk_id, date_recorded)
            DO UPDATE SET
                alerts_generated = monitoring_statistics.alerts_generated + 1,
                updated_at = NOW();
        END IF;
    END IF;
    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

-- Create triggers for automatic statistics updates
CREATE TRIGGER update_monitoring_stats_recordings
    AFTER INSERT ON call_recordings
    FOR EACH ROW EXECUTE FUNCTION update_monitoring_stats();

CREATE TRIGGER update_monitoring_stats_transcriptions
    AFTER INSERT ON call_transcriptions
    FOR EACH ROW EXECUTE FUNCTION update_monitoring_stats();

CREATE TRIGGER update_monitoring_stats_events
    AFTER INSERT ON anti_fraud_events
    FOR EACH ROW EXECUTE FUNCTION update_monitoring_stats();

-- Add indexes for performance
CREATE INDEX idx_ingress_trunks_monitoring ON ingress_trunks(anti_fraud_monitoring_enabled)
    WHERE anti_fraud_monitoring_enabled = true;

-- Create indexes for anti-fraud tables
CREATE INDEX idx_call_recordings_call_id ON call_recordings(call_id);
CREATE INDEX idx_call_recordings_trunk ON call_recordings(ingress_trunk_id);
CREATE INDEX idx_call_recordings_recorded_at ON call_recordings(recorded_at);
CREATE INDEX idx_call_recordings_processed ON call_recordings(processed_at);
CREATE INDEX idx_call_recordings_retention ON call_recordings(retention_expires_at);
CREATE INDEX idx_call_recordings_storage_type ON call_recordings(storage_type);
CREATE INDEX idx_call_recordings_legal_hold ON call_recordings(legal_hold);

CREATE INDEX idx_transcriptions_recording ON call_transcriptions(recording_id);
CREATE INDEX idx_transcriptions_risk_score ON call_transcriptions(risk_score);
CREATE INDEX idx_transcriptions_requires_review ON call_transcriptions(requires_review);
CREATE INDEX idx_transcriptions_banned_words ON call_transcriptions(banned_words_detected);

CREATE INDEX idx_banned_words_category ON banned_words_config(category);
CREATE INDEX idx_banned_words_enabled ON banned_words_config(enabled);

CREATE INDEX idx_anti_fraud_events_type ON anti_fraud_events(event_type);
CREATE INDEX idx_anti_fraud_events_call_id ON anti_fraud_events(call_id);
CREATE INDEX idx_anti_fraud_events_trunk ON anti_fraud_events(ingress_trunk_id);
CREATE INDEX idx_anti_fraud_events_risk_score ON anti_fraud_events(risk_score);
CREATE INDEX idx_anti_fraud_events_created_at ON anti_fraud_events(created_at);
CREATE INDEX idx_anti_fraud_events_alert_sent ON anti_fraud_events(alert_sent);

CREATE INDEX idx_monitoring_stats_trunk_date ON monitoring_statistics(ingress_trunk_id, date_recorded);
CREATE INDEX idx_monitoring_stats_date ON monitoring_statistics(date_recorded);

CREATE INDEX idx_ingress_trunks_monitoring ON ingress_trunks(anti_fraud_monitoring_enabled)
    WHERE anti_fraud_monitoring_enabled = true;

-- Comments for ECPA compliance
COMMENT ON TABLE call_recordings IS 'ECPA-compliant call recordings for anti-fraud monitoring. Requires proper legal authorization.';
COMMENT ON TABLE call_transcriptions IS 'ASR transcriptions for fraud detection. Contains personally identifiable information.';
COMMENT ON COLUMN ingress_trunks.legal_authorization_reference IS 'Legal basis reference for call monitoring (court order, warrant, etc.)';
COMMENT ON COLUMN ingress_trunks.ecpa_compliance_enabled IS 'Enables ECPA compliance safeguards and audit logging';