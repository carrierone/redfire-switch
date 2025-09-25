-- Add compliance tracking tables for lawful intercept operations
-- This migration extends the legal authorization system with comprehensive compliance tracking

-- Compliance violations table
CREATE TABLE IF NOT EXISTS compliance_violations (
    violation_id UUID PRIMARY KEY,
    violation_type TEXT NOT NULL,
    severity TEXT NOT NULL,
    description TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    user_id VARCHAR(100),
    authorization_id UUID REFERENCES legal_authorizations(authorization_id),
    detected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ,
    resolution_notes TEXT,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Chain of custody tracking
CREATE TABLE IF NOT EXISTS chain_of_custody (
    entry_id UUID PRIMARY KEY,
    resource_type TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    action TEXT NOT NULL,
    user_id VARCHAR(100) NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    previous_hash TEXT,
    current_hash TEXT NOT NULL,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Compliance reports
CREATE TABLE IF NOT EXISTS compliance_reports (
    report_id UUID PRIMARY KEY,
    report_type TEXT NOT NULL,
    generated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    period_start TIMESTAMPTZ NOT NULL,
    period_end TIMESTAMPTZ NOT NULL,
    total_authorizations BIGINT NOT NULL DEFAULT 0,
    active_authorizations BIGINT NOT NULL DEFAULT 0,
    total_violations BIGINT NOT NULL DEFAULT 0,
    critical_violations BIGINT NOT NULL DEFAULT 0,
    resolution_rate DECIMAL(5,4) NOT NULL DEFAULT 0.0,
    compliance_score DECIMAL(5,2) NOT NULL DEFAULT 0.0,
    recommendations JSONB DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Retention policies
CREATE TABLE IF NOT EXISTS retention_policies (
    policy_id UUID PRIMARY KEY,
    resource_type TEXT NOT NULL UNIQUE,
    retention_days INTEGER NOT NULL,
    auto_delete BOOLEAN NOT NULL DEFAULT false,
    notification_days INTEGER NOT NULL DEFAULT 30,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Add missing columns to legal_authorizations for enhanced compliance
ALTER TABLE legal_authorizations
ADD COLUMN IF NOT EXISTS requires_notification BOOLEAN DEFAULT false,
ADD COLUMN IF NOT EXISTS notification_sent_at TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS compliance_notes TEXT,
ADD COLUMN IF NOT EXISTS risk_level TEXT DEFAULT 'medium' CHECK (risk_level IN ('low', 'medium', 'high', 'critical'));

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_compliance_violations_detected_at ON compliance_violations(detected_at);
CREATE INDEX IF NOT EXISTS idx_compliance_violations_severity ON compliance_violations(severity);
CREATE INDEX IF NOT EXISTS idx_compliance_violations_resolved ON compliance_violations(resolved_at) WHERE resolved_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_compliance_violations_auth_id ON compliance_violations(authorization_id);

CREATE INDEX IF NOT EXISTS idx_chain_of_custody_resource ON chain_of_custody(resource_type, resource_id);
CREATE INDEX IF NOT EXISTS idx_chain_of_custody_timestamp ON chain_of_custody(timestamp);
CREATE INDEX IF NOT EXISTS idx_chain_of_custody_user ON chain_of_custody(user_id);

CREATE INDEX IF NOT EXISTS idx_compliance_reports_period ON compliance_reports(period_start, period_end);
CREATE INDEX IF NOT EXISTS idx_compliance_reports_generated ON compliance_reports(generated_at);

CREATE INDEX IF NOT EXISTS idx_legal_authorizations_notification ON legal_authorizations(requires_notification, notification_sent_at);
CREATE INDEX IF NOT EXISTS idx_legal_authorizations_risk ON legal_authorizations(risk_level);

-- Insert default retention policies
INSERT INTO retention_policies (policy_id, resource_type, retention_days, auto_delete, notification_days)
VALUES
    (gen_random_uuid(), 'voice_recordings', 730, false, 60),  -- 2 years for voice recordings
    (gen_random_uuid(), 'transcriptions', 730, false, 60),   -- 2 years for transcriptions
    (gen_random_uuid(), 'legal_authorizations', 2555, false, 90), -- 7 years for legal documents
    (gen_random_uuid(), 'audit_logs', 2555, false, 90)       -- 7 years for audit logs
ON CONFLICT (resource_type) DO NOTHING;

-- Function to automatically update updated_at timestamps
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Add triggers for updated_at columns
DROP TRIGGER IF EXISTS update_compliance_violations_updated_at ON compliance_violations;
CREATE TRIGGER update_compliance_violations_updated_at
    BEFORE UPDATE ON compliance_violations
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_retention_policies_updated_at ON retention_policies;
CREATE TRIGGER update_retention_policies_updated_at
    BEFORE UPDATE ON retention_policies
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Add constraints for data integrity
ALTER TABLE compliance_violations
ADD CONSTRAINT chk_violation_severity
CHECK (severity IN ('"Low"', '"Medium"', '"High"', '"Critical"'));

ALTER TABLE compliance_violations
ADD CONSTRAINT chk_resolved_at_after_detected
CHECK (resolved_at IS NULL OR resolved_at >= detected_at);

ALTER TABLE retention_policies
ADD CONSTRAINT chk_positive_retention_days
CHECK (retention_days > 0);

ALTER TABLE retention_policies
ADD CONSTRAINT chk_positive_notification_days
CHECK (notification_days >= 0);

-- Create a view for compliance dashboard
CREATE OR REPLACE VIEW compliance_dashboard AS
SELECT
    COUNT(CASE WHEN cv.resolved_at IS NULL THEN 1 END) as active_violations,
    COUNT(CASE WHEN cv.resolved_at IS NULL AND cv.severity = '"Critical"' THEN 1 END) as critical_violations,
    COUNT(CASE WHEN cv.resolved_at IS NULL AND cv.severity = '"High"' THEN 1 END) as high_violations,
    COUNT(CASE WHEN cv.detected_at > NOW() - INTERVAL '24 hours' THEN 1 END) as violations_24h,
    COUNT(CASE WHEN cv.resolved_at IS NOT NULL THEN 1 END) as resolved_violations,
    ROUND(
        CASE
            WHEN COUNT(*) > 0
            THEN (COUNT(CASE WHEN cv.resolved_at IS NOT NULL THEN 1 END)::numeric / COUNT(*)::numeric) * 100
            ELSE 100
        END, 2
    ) as resolution_rate_percent,
    COUNT(DISTINCT la.authorization_id) as total_authorizations,
    COUNT(CASE WHEN la.status = 'active' THEN 1 END) as active_authorizations,
    COUNT(CASE WHEN la.expires_at < NOW() AND la.status = 'active' THEN 1 END) as expired_active_auths
FROM compliance_violations cv
FULL OUTER JOIN legal_authorizations la ON true
WHERE cv.detected_at > NOW() - INTERVAL '30 days' OR cv.detected_at IS NULL;

-- Comment on tables for documentation
COMMENT ON TABLE compliance_violations IS 'Tracks all compliance violations detected by the monitoring system';
COMMENT ON TABLE chain_of_custody IS 'Maintains tamper-evident chain of custody for all voice integrity resources';
COMMENT ON TABLE compliance_reports IS 'Stores periodic compliance reports for regulatory requirements';
COMMENT ON TABLE retention_policies IS 'Defines data retention policies for different resource types';
COMMENT ON VIEW compliance_dashboard IS 'Real-time compliance dashboard statistics';