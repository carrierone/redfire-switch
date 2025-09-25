-- Legal Authorization System for Voice Integrity and Lawful Intercept
-- Extends existing anti-fraud monitoring with legal compliance framework

-- Legal authorization types and statuses
CREATE TYPE authorization_type AS ENUM (
    'court_order',
    'search_warrant',
    'wiretap_order',
    'pen_register',
    'emergency_request',
    'administrative_subpoena',
    'national_security_letter'
);

CREATE TYPE authorization_status AS ENUM (
    'pending',
    'approved',
    'active',
    'expired',
    'revoked',
    'appealed'
);

-- Legal authorizations for lawful intercept
CREATE TABLE legal_authorizations (
    id SERIAL PRIMARY KEY,
    authorization_number VARCHAR(255) UNIQUE NOT NULL, -- Court order number, warrant number, etc.
    authorization_type authorization_type NOT NULL,
    status authorization_status NOT NULL DEFAULT 'pending',

    -- Legal details
    issuing_authority VARCHAR(255) NOT NULL, -- Court name, agency, etc.
    case_number VARCHAR(255),
    investigating_agency VARCHAR(255) NOT NULL,
    investigating_officer VARCHAR(255) NOT NULL,
    contact_information JSONB NOT NULL, -- Phone, email, etc.

    -- Scope and targets
    target_identifiers JSONB NOT NULL, -- Phone numbers, IP addresses, etc.
    target_description TEXT,
    scope_description TEXT NOT NULL,

    -- Temporal constraints
    effective_date TIMESTAMP WITH TIME ZONE NOT NULL,
    expiration_date TIMESTAMP WITH TIME ZONE NOT NULL,
    service_date TIMESTAMP WITH TIME ZONE, -- When service provider was served

    -- Compliance tracking
    served_by VARCHAR(100), -- User ID from existing auth system
    legal_review_by VARCHAR(100), -- User ID from existing auth system
    compliance_notes TEXT,

    -- Document management
    authorization_document_path VARCHAR(512), -- Stored court order/warrant
    service_acknowledgment_path VARCHAR(512), -- Signed acknowledgment

    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_by VARCHAR(100) NOT NULL -- User ID from existing auth system
);

-- Lawful intercept targets and monitoring
CREATE TABLE lawful_intercept_targets (
    id SERIAL PRIMARY KEY,
    authorization_id INTEGER NOT NULL REFERENCES legal_authorizations(id) ON DELETE CASCADE,

    -- Target identification
    target_type VARCHAR(50) NOT NULL, -- 'phone_number', 'ip_address', 'trunk_id'
    target_value VARCHAR(255) NOT NULL, -- Actual phone number, IP, etc.
    target_description TEXT,

    -- Monitoring configuration
    monitoring_enabled BOOLEAN DEFAULT true,
    content_intercept_enabled BOOLEAN DEFAULT true, -- Full content vs metadata only
    retention_days INTEGER DEFAULT 365, -- Override default retention for legal case

    -- Status tracking
    first_activity_date TIMESTAMP WITH TIME ZONE,
    last_activity_date TIMESTAMP WITH TIME ZONE,
    total_calls_intercepted INTEGER DEFAULT 0,
    total_data_collected_bytes BIGINT DEFAULT 0,

    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Link call recordings to legal authorizations
CREATE TABLE recording_legal_authorizations (
    id SERIAL PRIMARY KEY,
    recording_id INTEGER NOT NULL REFERENCES call_recordings(id) ON DELETE CASCADE,
    authorization_id INTEGER NOT NULL REFERENCES legal_authorizations(id) ON DELETE CASCADE,
    target_id INTEGER REFERENCES lawful_intercept_targets(id),

    -- Intercept metadata
    intercept_reason TEXT,
    collection_method VARCHAR(100), -- 'real_time', 'retroactive', 'targeted'
    legal_authority_notified BOOLEAN DEFAULT false,
    notification_date TIMESTAMP WITH TIME ZONE,

    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_by VARCHAR(100) NOT NULL, -- User ID from existing auth system

    UNIQUE(recording_id, authorization_id)
);

-- Voice integrity audit log for compliance tracking
CREATE TABLE voice_integrity_audit_log (
    id SERIAL PRIMARY KEY,
    user_id VARCHAR(100), -- Consistent with existing audit_log table
    session_id VARCHAR(255),

    -- Action details
    action_type VARCHAR(100) NOT NULL, -- 'view_recording', 'download_audio', 'legal_hold', etc.
    resource_type VARCHAR(50) NOT NULL, -- 'recording', 'authorization', 'user'
    resource_id VARCHAR(255) NOT NULL, -- ID of the resource accessed

    -- Context and metadata
    authorization_id INTEGER REFERENCES legal_authorizations(id),
    legal_basis VARCHAR(255), -- Legal justification for access
    business_justification TEXT,

    -- Technical details
    ip_address INET,
    user_agent TEXT,
    request_details JSONB,
    response_summary JSONB,

    -- Compliance tracking
    ecpa_compliant BOOLEAN DEFAULT true,
    calea_notification_required BOOLEAN DEFAULT false,
    data_minimization_applied BOOLEAN DEFAULT true,

    timestamp TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Legal authorization workflow states
CREATE TABLE authorization_workflow_log (
    id SERIAL PRIMARY KEY,
    authorization_id INTEGER NOT NULL REFERENCES legal_authorizations(id) ON DELETE CASCADE,

    -- State change details
    previous_status authorization_status,
    new_status authorization_status NOT NULL,
    change_reason TEXT NOT NULL,
    supporting_documentation TEXT,

    -- Approval chain
    changed_by VARCHAR(100) NOT NULL, -- User ID from existing auth system
    approved_by VARCHAR(100), -- User ID from existing auth system
    legal_review_completed BOOLEAN DEFAULT false,

    -- Notification tracking
    law_enforcement_notified BOOLEAN DEFAULT false,
    notification_method VARCHAR(100), -- 'email', 'secure_portal', 'phone'
    notification_date TIMESTAMP WITH TIME ZONE,

    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Compliance reporting and statistics
CREATE TABLE voice_integrity_statistics (
    id SERIAL PRIMARY KEY,
    report_date DATE NOT NULL DEFAULT CURRENT_DATE,

    -- Legal authorization metrics
    active_authorizations INTEGER DEFAULT 0,
    pending_authorizations INTEGER DEFAULT 0,
    expired_authorizations INTEGER DEFAULT 0,

    -- Intercept metrics
    total_targets_monitored INTEGER DEFAULT 0,
    calls_intercepted_today INTEGER DEFAULT 0,
    data_collected_bytes_today BIGINT DEFAULT 0,

    -- Compliance metrics
    compliance_violations INTEGER DEFAULT 0,
    overdue_notifications INTEGER DEFAULT 0,
    expired_authorizations_past_due INTEGER DEFAULT 0,

    -- Access metrics
    authorized_access_events INTEGER DEFAULT 0,
    unauthorized_access_attempts INTEGER DEFAULT 0,
    data_exports_today INTEGER DEFAULT 0,

    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CONSTRAINT voice_integrity_stats_unique UNIQUE(report_date)
);

-- Create indexes for performance
CREATE INDEX idx_legal_auth_number ON legal_authorizations(authorization_number);
CREATE INDEX idx_legal_auth_status ON legal_authorizations(status);
CREATE INDEX idx_legal_auth_dates ON legal_authorizations(effective_date, expiration_date);
CREATE INDEX idx_legal_auth_targets ON legal_authorizations USING GIN(target_identifiers);

CREATE INDEX idx_intercept_targets_auth ON lawful_intercept_targets(authorization_id);
CREATE INDEX idx_intercept_targets_type_value ON lawful_intercept_targets(target_type, target_value);
CREATE INDEX idx_intercept_targets_enabled ON lawful_intercept_targets(monitoring_enabled) WHERE monitoring_enabled = true;

CREATE INDEX idx_recording_legal_auth_recording ON recording_legal_authorizations(recording_id);
CREATE INDEX idx_recording_legal_auth_authorization ON recording_legal_authorizations(authorization_id);

CREATE INDEX idx_vi_audit_log_user ON voice_integrity_audit_log(user_id);
CREATE INDEX idx_vi_audit_log_timestamp ON voice_integrity_audit_log(timestamp);
CREATE INDEX idx_vi_audit_log_action ON voice_integrity_audit_log(action_type);
CREATE INDEX idx_vi_audit_log_resource ON voice_integrity_audit_log(resource_type, resource_id);
CREATE INDEX idx_vi_audit_log_authorization ON voice_integrity_audit_log(authorization_id);

CREATE INDEX idx_workflow_log_auth ON authorization_workflow_log(authorization_id);
CREATE INDEX idx_workflow_log_timestamp ON authorization_workflow_log(created_at);

CREATE INDEX idx_vi_stats_date ON voice_integrity_statistics(report_date);

-- Add voice integrity fields to existing call_recordings table
ALTER TABLE call_recordings ADD COLUMN voice_integrity_officer_id VARCHAR(100); -- User ID from existing auth system
ALTER TABLE call_recordings ADD COLUMN legal_review_required BOOLEAN DEFAULT false;
ALTER TABLE call_recordings ADD COLUMN legal_review_completed BOOLEAN DEFAULT false;
ALTER TABLE call_recordings ADD COLUMN legal_review_date TIMESTAMP WITH TIME ZONE;
ALTER TABLE call_recordings ADD COLUMN data_classification VARCHAR(50) DEFAULT 'unclassified'; -- unclassified, restricted, confidential

-- Add lawful intercept flag to anti_fraud_events
ALTER TABLE anti_fraud_events ADD COLUMN lawful_intercept_case BOOLEAN DEFAULT false;
ALTER TABLE anti_fraud_events ADD COLUMN authorization_id INTEGER REFERENCES legal_authorizations(id);

-- Create trigger to update voice integrity statistics
CREATE OR REPLACE FUNCTION update_voice_integrity_stats()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        -- Update daily statistics when new events occur
        IF TG_TABLE_NAME = 'recording_legal_authorizations' THEN
            INSERT INTO voice_integrity_statistics (calls_intercepted_today)
            VALUES (1)
            ON CONFLICT (report_date)
            DO UPDATE SET
                calls_intercepted_today = voice_integrity_statistics.calls_intercepted_today + 1;
        ELSIF TG_TABLE_NAME = 'voice_integrity_audit_log' THEN
            IF NEW.action_type = 'download_audio' THEN
                INSERT INTO voice_integrity_statistics (data_exports_today)
                VALUES (1)
                ON CONFLICT (report_date)
                DO UPDATE SET
                    data_exports_today = voice_integrity_statistics.data_exports_today + 1;
            END IF;

            INSERT INTO voice_integrity_statistics (authorized_access_events)
            VALUES (1)
            ON CONFLICT (report_date)
            DO UPDATE SET
                authorized_access_events = voice_integrity_statistics.authorized_access_events + 1;
        END IF;
    END IF;
    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

-- Create triggers for automatic statistics updates
CREATE TRIGGER update_vi_stats_intercepts
    AFTER INSERT ON recording_legal_authorizations
    FOR EACH ROW EXECUTE FUNCTION update_voice_integrity_stats();

CREATE TRIGGER update_vi_stats_access
    AFTER INSERT ON voice_integrity_audit_log
    FOR EACH ROW EXECUTE FUNCTION update_voice_integrity_stats();

-- Create view for active legal authorizations
CREATE VIEW v_active_legal_authorizations AS
SELECT
    la.*,
    COUNT(lit.id) as target_count,
    COUNT(rla.id) as recordings_count,
    CASE
        WHEN la.expiration_date < NOW() THEN 'expired'
        WHEN la.effective_date > NOW() THEN 'not_yet_effective'
        ELSE 'active'
    END as effective_status
FROM legal_authorizations la
LEFT JOIN lawful_intercept_targets lit ON la.id = lit.authorization_id
LEFT JOIN recording_legal_authorizations rla ON la.id = rla.authorization_id
WHERE la.status IN ('approved', 'active')
GROUP BY la.id;

-- Create view for compliance monitoring
CREATE VIEW v_compliance_summary AS
SELECT
    report_date,
    active_authorizations,
    pending_authorizations,
    expired_authorizations,
    calls_intercepted_today,
    data_collected_bytes_today,
    compliance_violations,
    authorized_access_events,
    unauthorized_access_attempts
FROM voice_integrity_statistics
WHERE report_date >= CURRENT_DATE - INTERVAL '30 days'
ORDER BY report_date DESC;

-- Comments for compliance documentation
COMMENT ON TABLE legal_authorizations IS 'Legal authorizations for lawful intercept under CALEA, ECPA, and other applicable laws';
COMMENT ON TABLE lawful_intercept_targets IS 'Specific targets under legal authorization for content and metadata collection';
COMMENT ON TABLE voice_integrity_audit_log IS 'Comprehensive audit trail for all voice integrity and legal compliance activities';
COMMENT ON COLUMN legal_authorizations.authorization_number IS 'Court order number, warrant number, or other legal document identifier';
COMMENT ON COLUMN legal_authorizations.effective_date IS 'Date when authorization becomes legally effective';
COMMENT ON COLUMN legal_authorizations.expiration_date IS 'Date when authorization expires and must be renewed';
COMMENT ON COLUMN lawful_intercept_targets.content_intercept_enabled IS 'Whether full content is authorized vs metadata only';
COMMENT ON COLUMN voice_integrity_audit_log.ecpa_compliant IS 'Whether the action complies with ECPA requirements';
COMMENT ON COLUMN voice_integrity_audit_log.calea_notification_required IS 'Whether CALEA notification to law enforcement is required';

-- Insert sample legal authorization for testing (REMOVE IN PRODUCTION)
INSERT INTO legal_authorizations (
    authorization_number,
    authorization_type,
    status,
    issuing_authority,
    investigating_agency,
    investigating_officer,
    contact_information,
    target_identifiers,
    scope_description,
    effective_date,
    expiration_date,
    created_by
) VALUES (
    'DEMO-2024-001',
    'court_order',
    'active',
    'District Court of Example County',
    'Example Police Department',
    'Detective John Smith',
    '{"phone": "+1-555-0123", "email": "jsmith@example.pd.gov"}',
    '{"phone_numbers": ["+1-555-0199"], "trunk_ids": [1]}',
    'Investigation of suspected telecommunications fraud',
    NOW(),
    NOW() + INTERVAL '90 days',
    'admin'
);