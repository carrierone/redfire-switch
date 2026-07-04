-- Sample data for LCR testing
-- This populates the database with example NANPA rates and trunk configurations

-- Insert sample NANPA static data (US states and area codes)
INSERT INTO nanpa_static (npa, nxx, state, country, lata, ocn, rate_center) VALUES
('212', NULL, 'NY', 'US', '132', '7421', 'MANHATTAN'),
('213', NULL, 'CA', 'US', '730', '7421', 'LOS ANGELES'),
('312', NULL, 'IL', 'US', '358', '7421', 'CHICAGO'),
('415', NULL, 'CA', 'US', '722', '7421', 'SAN FRANCISCO'),
('305', NULL, 'FL', 'US', '460', '7421', 'MIAMI'),
('404', NULL, 'GA', 'US', '438', '7421', 'ATLANTA'),
('202', NULL, 'DC', 'US', '236', '7421', 'WASHINGTON'),
('617', NULL, 'MA', 'US', '128', '7421', 'BOSTON'),
('512', NULL, 'TX', 'US', '566', '7421', 'AUSTIN'),
('713', NULL, 'TX', 'US', '560', '7421', 'HOUSTON'),
('206', NULL, 'WA', 'US', '674', '7421', 'SEATTLE');

-- More specific NPANXX data (npa is 3 digits, nxx is 3 digits; the cache keys
-- these as the concatenated NPANXX string, e.g. "212555").
INSERT INTO nanpa_static (npa, nxx, state, country, lata, ocn, rate_center) VALUES
('212', '555', 'NY', 'US', '132', '7421', 'MANHATTAN'),
('213', '555', 'CA', 'US', '730', '7421', 'LOS ANGELES'),
('415', '555', 'CA', 'US', '722', '7421', 'SAN FRANCISCO'),
('713', '555', 'TX', 'US', '560', '7421', 'HOUSTON');

-- Insert vendor rate decks
INSERT INTO vendor_rate_decks (name, vendor_id, rate_type, effective_date) VALUES
('Vendor A - NANPA Standard', 1, 'DNIS', NOW()),
('Vendor B - NANPA Premium', 2, 'DNIS', NOW()),
('Vendor C - LRN Based', 3, 'LRN', NOW());

-- Insert client rate decks
INSERT INTO client_rate_decks (name, client_id, rate_type, effective_date) VALUES
('Client X - Retail', 1, 'DNIS', NOW()),
('Client Y - Wholesale', 2, 'DNIS', NOW()),
('Client Z - LRN Based', 3, 'LRN', NOW());

-- Insert vendor NANPA rates (costs)
-- Vendor A rates
INSERT INTO vendor_nanpa_rates (deck_id, code, inter_rate, intra_rate, ij_rate, local_rate, min_increment, interval) VALUES
(1, '1', 0.0045, 0.0055, 0.0050, 0.0025, 6, 6), -- Default US rate
(1, '1212', 0.0035, 0.0040, 0.0038, 0.0020, 6, 6), -- NYC
(1, '1213', 0.0040, 0.0045, 0.0042, 0.0022, 6, 6), -- LA
(1, '1415', 0.0042, 0.0048, 0.0045, 0.0024, 6, 6), -- SF
(1, '1312', 0.0038, 0.0043, 0.0040, 0.0021, 6, 6), -- Chicago
(1, '1305', 0.0041, 0.0046, 0.0043, 0.0023, 6, 6); -- Miami

-- Vendor B rates (slightly higher)
INSERT INTO vendor_nanpa_rates (deck_id, code, inter_rate, intra_rate, ij_rate, local_rate, min_increment, interval) VALUES
(2, '1', 0.0055, 0.0065, 0.0060, 0.0030, 6, 6), -- Default US rate
(2, '1212', 0.0045, 0.0050, 0.0048, 0.0025, 6, 6), -- NYC
(2, '1213', 0.0050, 0.0055, 0.0052, 0.0027, 6, 6), -- LA
(2, '1415', 0.0052, 0.0058, 0.0055, 0.0029, 6, 6), -- SF
(2, '1312', 0.0048, 0.0053, 0.0050, 0.0026, 6, 6), -- Chicago
(2, '1305', 0.0051, 0.0056, 0.0053, 0.0028, 6, 6); -- Miami

-- Vendor C LRN-based rates
INSERT INTO vendor_nanpa_rates (deck_id, code, inter_rate, intra_rate, ij_rate, local_rate, min_increment, interval) VALUES
(3, '1', 0.0040, 0.0050, 0.0045, 0.0020, 6, 6), -- Default US rate
(3, '1212555', 0.0030, 0.0035, 0.0033, 0.0015, 6, 6), -- Specific NPANXX
(3, '1213555', 0.0035, 0.0040, 0.0037, 0.0017, 6, 6),
(3, '1415555', 0.0037, 0.0043, 0.0040, 0.0019, 6, 6);

-- Insert client NANPA rates (selling)
-- Client X retail rates
INSERT INTO client_nanpa_rates (deck_id, code, inter_rate, intra_rate, ij_rate, local_rate, min_increment, interval) VALUES
(1, '1', 0.0120, 0.0140, 0.0130, 0.0080, 6, 6), -- Default US rate
(1, '1212', 0.0100, 0.0110, 0.0105, 0.0070, 6, 6), -- NYC
(1, '1213', 0.0110, 0.0120, 0.0115, 0.0075, 6, 6), -- LA
(1, '1415', 0.0115, 0.0125, 0.0120, 0.0078, 6, 6), -- SF
(1, '1312', 0.0105, 0.0115, 0.0110, 0.0072, 6, 6), -- Chicago
(1, '1305', 0.0112, 0.0122, 0.0117, 0.0076, 6, 6); -- Miami

-- Client Y wholesale rates
INSERT INTO client_nanpa_rates (deck_id, code, inter_rate, intra_rate, ij_rate, local_rate, min_increment, interval) VALUES
(2, '1', 0.0080, 0.0090, 0.0085, 0.0050, 6, 6), -- Default US rate
(2, '1212', 0.0070, 0.0075, 0.0072, 0.0040, 6, 6), -- NYC
(2, '1213', 0.0075, 0.0080, 0.0077, 0.0042, 6, 6), -- LA
(2, '1415', 0.0077, 0.0083, 0.0080, 0.0044, 6, 6); -- SF

-- Insert egress trunks (vendors)
INSERT INTO egress_trunks (name, vendor_id, host, port, transport, capacity_limit, cps_limit, priority) VALUES
('Vendor-A-Primary', 1, 'sip.vendor-a.com', 5060, 'UDP', 1000, 100.0, 100),
('Vendor-A-Secondary', 1, 'sip2.vendor-a.com', 5060, 'UDP', 1000, 100.0, 110),
('Vendor-B-Primary', 2, 'gateway.vendor-b.net', 5060, 'TCP', 500, 50.0, 120),
('Vendor-C-LRN', 3, 'lrn.vendor-c.com', 5061, 'TLS', 2000, 200.0, 90);

-- Insert ingress trunks (clients)
INSERT INTO ingress_trunks (name, client_id, ip_address, capacity_limit, cps_limit, profit_protection, min_profit_margin) VALUES
('Client-X-Retail', 1, '192.168.1.10', 100, 10.0, true, 0.0020),
('Client-Y-Wholesale', 2, '10.0.0.20', 500, 50.0, true, 0.0010),
('Client-Z-Premium', 3, '172.16.0.30', 200, 20.0, false, 0.0000);

-- Create LCR routes
INSERT INTO lcr_routes (name, route_type, description, priority) VALUES
('NANPA-Least-Cost', 'NANPA', 'Primary NANPA LCR route', 100),
('NANPA-Premium', 'NANPA', 'Premium quality NANPA route', 110),
('International-AZ', 'A-Z', 'International A-Z termination', 120);

-- Associate trunks with rate decks
INSERT INTO trunk_rate_associations (egress_trunk_id, vendor_deck_id) VALUES
(1, 1), -- Vendor-A-Primary uses deck 1
(2, 1), -- Vendor-A-Secondary uses deck 1
(3, 2), -- Vendor-B-Primary uses deck 2
(4, 3); -- Vendor-C-LRN uses deck 3

INSERT INTO trunk_rate_associations (ingress_trunk_id, client_deck_id) VALUES
(1, 1), -- Client-X-Retail uses deck 1
(2, 2), -- Client-Y-Wholesale uses deck 2
(3, 3); -- Client-Z-Premium uses deck 3

-- Link ingress trunks to LCR routes
INSERT INTO ingress_lcr_routes (ingress_trunk_id, lcr_route_id, priority) VALUES
(1, 1, 100), -- Client-X uses NANPA-Least-Cost
(2, 1, 100), -- Client-Y uses NANPA-Least-Cost
(3, 2, 100); -- Client-Z uses NANPA-Premium

-- Link egress trunks to LCR routes
INSERT INTO lcr_route_trunks (lcr_route_id, egress_trunk_id, vendor_deck_id, priority, weight) VALUES
(1, 1, 1, 100, 1), -- NANPA-Least-Cost -> Vendor-A-Primary
(1, 2, 1, 110, 1), -- NANPA-Least-Cost -> Vendor-A-Secondary
(1, 3, 2, 120, 1), -- NANPA-Least-Cost -> Vendor-B-Primary
(2, 4, 3, 100, 1); -- NANPA-Premium -> Vendor-C-LRN

-- Add some static routes for special handling
INSERT INTO static_routes (ingress_trunk_id, egress_trunk_id, pattern, priority, position, description) VALUES
(NULL, 1, '^1911$', 1, 'BEFORE', 'Emergency 911 calls'),
(NULL, 1, '^1411$', 2, 'BEFORE', 'Directory assistance'),
(1, 3, '^1800', 10, 'BEFORE', 'Toll-free for Client-X'),
(NULL, 2, '^1900', 999, 'AFTER', 'Premium rate fallback');

-- Sample LRN cache entries
INSERT INTO lrn_cache (tn, lrn, spid, ocn, lata, state, jurisdiction, cached_at, expires_at) VALUES
('12125551234', '12125550000', 'VZN', '7421', '132', 'NY', 'INTER', NOW(), NOW() + INTERVAL '24 hours'),
('14155555678', '14155550000', 'ATT', '7422', '722', 'CA', 'INTRA', NOW(), NOW() + INTERVAL '24 hours');

-- Override route advance codes for specific trunk
INSERT INTO route_advance_configs (scope, scope_id, advance_on_codes, stop_on_codes) 
VALUES ('INGRESS_TRUNK', 1, 
        ARRAY['503', '504', '480', '487', '603'], 
        ARRAY['404', '486', '600', '604'])
ON CONFLICT (scope, scope_id) DO UPDATE 
SET advance_on_codes = EXCLUDED.advance_on_codes,
    stop_on_codes = EXCLUDED.stop_on_codes;

-- Custom timers for premium client
INSERT INTO timer_configs (scope, scope_id, timer_100_to_183_ms, timer_max_call_duration_sec) 
VALUES ('INGRESS_TRUNK', 3, 20000, 14400) -- 20 sec setup, 4 hour max
ON CONFLICT (scope, scope_id) DO UPDATE 
SET timer_100_to_183_ms = EXCLUDED.timer_100_to_183_ms,
    timer_max_call_duration_sec = EXCLUDED.timer_max_call_duration_sec;