-- LCR Test Setup SQL
-- Configures routing for test call: ANI 17028880001 → DNIS 18002255288 → 173.193.144.207:5060

-- Insert test vendor (egress)
INSERT INTO vendors (id, name, description, active) VALUES 
(999, 'Test Egress Provider', 'Test provider for SIPp testing', true)
ON CONFLICT (id) DO UPDATE SET 
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    active = EXCLUDED.active;

-- Insert test client (ingress)  
INSERT INTO clients (id, name, description, active) VALUES
(999, 'Test Ingress Client', 'Test client for SIPp testing', true)
ON CONFLICT (id) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    active = EXCLUDED.active;

-- Insert egress trunk pointing to 173.193.144.207:5060
INSERT INTO egress_trunks (id, name, vendor_id, host, port, transport, capacity_limit, cps_limit, active, priority, weight)
VALUES (999, 'Test-Egress-173.193.144.207', 999, '173.193.144.207', 5060, 'UDP', 100, 10.0, true, 100, 1)
ON CONFLICT (id) DO UPDATE SET
    name = EXCLUDED.name,
    vendor_id = EXCLUDED.vendor_id,
    host = EXCLUDED.host,
    port = EXCLUDED.port,
    transport = EXCLUDED.transport,
    capacity_limit = EXCLUDED.capacity_limit,
    cps_limit = EXCLUDED.cps_limit,
    active = EXCLUDED.active,
    priority = EXCLUDED.priority,
    weight = EXCLUDED.weight;

-- Insert ingress trunk for test client
INSERT INTO ingress_trunks (id, name, client_id, ip_address, capacity_limit, cps_limit, profit_protection, min_profit_margin, active)
VALUES (999, 'Test-Ingress-SIPp', 999, '127.0.0.1'::inet, 10, 5.0, false, 0.0000, true)
ON CONFLICT (id) DO UPDATE SET
    name = EXCLUDED.name,
    client_id = EXCLUDED.client_id,
    ip_address = EXCLUDED.ip_address,
    capacity_limit = EXCLUDED.capacity_limit,
    cps_limit = EXCLUDED.cps_limit,
    profit_protection = EXCLUDED.profit_protection,
    min_profit_margin = EXCLUDED.min_profit_margin,
    active = EXCLUDED.active;

-- Create vendor rate deck for toll-free
INSERT INTO vendor_rate_decks (id, name, vendor_id, rate_type, effective_date, active)
VALUES (999, 'Test Vendor Toll-Free Rates', 999, 'DNIS', NOW(), true)
ON CONFLICT (id) DO UPDATE SET
    name = EXCLUDED.name,
    vendor_id = EXCLUDED.vendor_id,
    rate_type = EXCLUDED.rate_type,
    effective_date = EXCLUDED.effective_date,
    active = EXCLUDED.active;

-- Create client rate deck
INSERT INTO client_rate_decks (id, name, client_id, rate_type, effective_date, active)  
VALUES (999, 'Test Client Rates', 999, 'DNIS', NOW(), true)
ON CONFLICT (id) DO UPDATE SET
    name = EXCLUDED.name,
    client_id = EXCLUDED.client_id,
    rate_type = EXCLUDED.rate_type,
    effective_date = EXCLUDED.effective_date,
    active = EXCLUDED.active;

-- Insert toll-free vendor rates (cost - what we pay)
INSERT INTO vendor_nanpa_rates (deck_id, code, inter_rate, intra_rate, ij_rate, local_rate, min_increment, interval, setup_fee)
VALUES 
-- Specific toll-free number
(999, '18002255288', 0.0015, 0.0015, 0.0015, 0.0015, 6, 6, 0.0050),
-- General 800 toll-free  
(999, '1800', 0.0020, 0.0020, 0.0020, 0.0020, 6, 6, 0.0050),
-- Las Vegas rates (for ANI context)
(999, '1702', 0.0045, 0.0040, 0.0042, 0.0035, 6, 6, 0.0100)
ON CONFLICT (deck_id, code) DO UPDATE SET
    inter_rate = EXCLUDED.inter_rate,
    intra_rate = EXCLUDED.intra_rate,
    ij_rate = EXCLUDED.ij_rate,
    local_rate = EXCLUDED.local_rate,
    min_increment = EXCLUDED.min_increment,
    interval = EXCLUDED.interval,
    setup_fee = EXCLUDED.setup_fee;

-- Insert client rates (selling - what client pays, usually $0 for toll-free)
INSERT INTO client_nanpa_rates (deck_id, code, inter_rate, intra_rate, ij_rate, local_rate, min_increment, interval, setup_fee)
VALUES
-- Toll-free is usually free for caller
(999, '18002255288', 0.0000, 0.0000, 0.0000, 0.0000, 6, 6, 0.0000),
(999, '1800', 0.0000, 0.0000, 0.0000, 0.0000, 6, 6, 0.0000)
ON CONFLICT (deck_id, code) DO UPDATE SET
    inter_rate = EXCLUDED.inter_rate,
    intra_rate = EXCLUDED.intra_rate,
    ij_rate = EXCLUDED.ij_rate,
    local_rate = EXCLUDED.local_rate,
    min_increment = EXCLUDED.min_increment,
    interval = EXCLUDED.interval,
    setup_fee = EXCLUDED.setup_fee;

-- Create LCR route for toll-free
INSERT INTO lcr_routes (id, name, route_type, description, active, priority)
VALUES (999, 'Test Toll-Free Route', 'NANPA', 'Test route for toll-free calls via SIPp', true, 100)
ON CONFLICT (id) DO UPDATE SET
    name = EXCLUDED.name,
    route_type = EXCLUDED.route_type,
    description = EXCLUDED.description,
    active = EXCLUDED.active,
    priority = EXCLUDED.priority;

-- Link ingress trunk to LCR route
INSERT INTO ingress_lcr_routes (ingress_trunk_id, lcr_route_id, priority)
VALUES (999, 999, 100)
ON CONFLICT (ingress_trunk_id, lcr_route_id) DO UPDATE SET
    priority = EXCLUDED.priority;

-- Link egress trunk to LCR route with rate deck
INSERT INTO lcr_route_trunks (lcr_route_id, egress_trunk_id, vendor_deck_id, priority, weight)
VALUES (999, 999, 999, 100, 1)
ON CONFLICT (lcr_route_id, egress_trunk_id, vendor_deck_id) DO UPDATE SET
    priority = EXCLUDED.priority,
    weight = EXCLUDED.weight;

-- Associate rate decks with trunks
INSERT INTO trunk_rate_associations (egress_trunk_id, vendor_deck_id, priority)
VALUES (999, 999, 100)
ON CONFLICT (egress_trunk_id, vendor_deck_id) DO NOTHING;

INSERT INTO trunk_rate_associations (ingress_trunk_id, client_deck_id, priority)  
VALUES (999, 999, 100)
ON CONFLICT (ingress_trunk_id, client_deck_id) DO NOTHING;

-- Add NANPA data for testing (ensure Las Vegas and toll-free are in database)
INSERT INTO nanpa_static (npa, nxx, state, country, lata, rate_center)
VALUES 
('702', '888', 'NV', 'US', '722', 'LAS VEGAS'),
('800', '225', 'XX', 'US', '999', 'TOLL FREE')
ON CONFLICT (npa, nxx) DO NOTHING;

-- Initialize trunk statistics
INSERT INTO trunk_usage_stats (trunk_id, trunk_type, current_calls, current_cps, total_calls, total_minutes)
VALUES 
(999, 'EGRESS', 0, 0.0, 0, 0.0),
(999, 'INGRESS', 0, 0.0, 0, 0.0)
ON CONFLICT (trunk_id, trunk_type) DO UPDATE SET
    current_calls = 0,
    current_cps = 0.0;

-- Show configuration summary
SELECT 'LCR Test Configuration Summary' as status;

SELECT 
    'Egress Trunk' as component,
    name as name,
    host || ':' || port as destination,
    CASE WHEN active THEN 'Active' ELSE 'Inactive' END as status
FROM egress_trunks WHERE id = 999;

SELECT 
    'Ingress Trunk' as component,
    name as name, 
    ip_address::text as source,
    CASE WHEN active THEN 'Active' ELSE 'Inactive' END as status
FROM ingress_trunks WHERE id = 999;

SELECT 
    'Vendor Rates' as component,
    code as destination,
    ij_rate as cost_per_minute,
    setup_fee as setup_cost
FROM vendor_nanpa_rates WHERE deck_id = 999 ORDER BY LENGTH(code) DESC;

SELECT 
    'Client Rates' as component,
    code as destination,
    ij_rate as rate_per_minute,
    setup_fee as setup_cost  
FROM client_nanpa_rates WHERE deck_id = 999 ORDER BY LENGTH(code) DESC;