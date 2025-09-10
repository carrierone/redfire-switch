/**
 * RedFire Switch Configuration Manager
 * Advanced tree-based configuration system with template inheritance
 */

// Global state
let configData = {};
let templateData = {};
let currentConfig = null;
let currentPath = [];
let hasChanges = false;
let originalValues = {};

// Configuration tree structure
const configTree = {
    global: {
        name: 'Global Configuration',
        icon: '🌐',
        tooltip: 'System-wide configuration settings that apply across all modules',
        children: {
            system: {
                name: 'System Settings',
                icon: '⚙️',
                template: 'basic_sip',
                tooltip: 'Core system configuration including general settings and performance tuning',
                children: {
                    general: { name: 'General', icon: '📋', config: 'system.general', tooltip: 'Basic system settings: hostname, timezone, NTP servers, system resources' },
                    logging: { name: 'Logging', icon: '📝', config: 'system.logging', tooltip: 'Log levels, destinations, retention policies, and audit trails' },
                    performance: { name: 'Performance', icon: '⚡', config: 'system.performance', tooltip: 'Memory limits, CPU affinity, connection pools, thread allocation' },
                    maintenance: { name: 'Maintenance', icon: '🔧', config: 'system.maintenance', tooltip: 'Scheduled tasks, backup policies, system health monitoring' }
                }
            },
            network: {
                name: 'Network Configuration',
                icon: '🌐',
                template: 'security',
                tooltip: 'Network layer configuration including interfaces and security settings',
                children: {
                    interfaces: { name: 'Interfaces', icon: '🔌', config: 'network.interfaces', tooltip: 'Network interface bindings, IP addresses, VLANs, bonding' },
                    routing: { name: 'Routing', icon: '🛣️', config: 'network.routing', tooltip: 'Static routes, default gateways, routing policies, OSPF/BGP' },
                    firewall: { name: 'Firewall', icon: '🛡️', config: 'network.firewall', tooltip: 'IPTables rules, fail2ban, DDoS protection, rate limiting' },
                    dns: { name: 'DNS', icon: '🏷️', config: 'network.dns', tooltip: 'DNS servers, domain resolution, DNS caching, reverse lookups' }
                }
            },
            security: {
                name: 'Security Settings',
                icon: '🔐',
                template: 'security',
                tooltip: 'Security policies, authentication, encryption, and compliance settings',
                children: {
                    authentication: { name: 'Authentication', icon: '🔑', config: 'security.authentication', tooltip: 'User authentication, LDAP, SSO, password policies, MFA' },
                    certificates: { name: 'Certificates', icon: '📜', config: 'security.certificates', tooltip: 'TLS certificates, CA management, certificate rotation' },
                    encryption: { name: 'Encryption', icon: '🔒', config: 'security.encryption', tooltip: 'Encryption algorithms, key management, SRTP/TLS settings' },
                    compliance: { name: 'Compliance', icon: '⚖️', config: 'security.compliance', tooltip: 'STIR/SHAKEN, robocall prevention, regulatory compliance' }
                }
            },
            database: {
                name: 'Database Settings',
                icon: '🗄️',
                template: 'database',
                tooltip: 'Database configuration for CDR, configuration, and operational data',
                children: {
                    connections: { name: 'Connections', icon: '🔗', config: 'database.connections', tooltip: 'Database connection pools, timeouts, SSL/TLS settings' },
                    cdr: { name: 'CDR Storage', icon: '📊', config: 'database.cdr', tooltip: 'Call Detail Record storage, retention, archiving policies' },
                    backup: { name: 'Backup & Recovery', icon: '💾', config: 'database.backup', tooltip: 'Database backup schedules, recovery procedures, replication' }
                }
            },
            monitoring: {
                name: 'Monitoring & Alerting',
                icon: '📈',
                template: 'monitoring',
                tooltip: 'System monitoring, metrics collection, and alerting configuration',
                children: {
                    metrics: { name: 'Metrics Collection', icon: '📊', config: 'monitoring.metrics', tooltip: 'Prometheus, SNMP, call quality metrics, KPI thresholds' },
                    alerts: { name: 'Alert Rules', icon: '🚨', config: 'monitoring.alerts', tooltip: 'Alert thresholds, notification channels, escalation policies' },
                    health_checks: { name: 'Health Checks', icon: '🏥', config: 'monitoring.health', tooltip: 'Service health monitoring, dependency checks, SLA monitoring' }
                }
            },
            billing: {
                name: 'Billing & Rating',
                icon: '💰',
                template: 'billing',
                tooltip: 'Rate management and CDR processing - invoicing/reconciliation handled by B/OSS',
                children: {
                    rating: { name: 'Rate Tables', icon: '💱', config: 'billing.rating', tooltip: 'Rate deck management, least cost routing, margin calculation' },
                    cdr_processing: { name: 'CDR Processing', icon: '📊', config: 'billing.cdr_processing', tooltip: 'Call detail record formatting and export for B/OSS systems' }
                }
            }
        }
    },
    sip: {
        name: 'SIP Configuration',
        icon: '📞',
        tooltip: 'Session Initiation Protocol settings for Class 4 transit switch operations',
        children: {
            profiles: {
                name: 'SIP Profiles',
                icon: '⚙️',
                template: 'basic_sip',
                tooltip: 'SIP profile configurations for Class 4 switch transit operations',
                children: {
                    default: { name: 'Default Profile', icon: '🔧', config: 'sip.profiles.default', tooltip: 'Default SIP profile bound to 0.0.0.0 for all carrier traffic' },
                    create_new: { name: '+ Create New Profile', icon: '➕', config: 'sip.profiles.new', tooltip: 'Create additional SIP profile for specific carrier or network requirements' }
                }
            },
            carrier_interconnects: {
                name: 'Carrier Interconnects',
                icon: '🌐',
                tooltip: 'Inter-carrier connection definitions for Class 4 transit traffic',
                children: {
                    termination: {
                        name: 'Termination',
                        icon: '📤',
                        tooltip: 'Outbound traffic termination to other carriers',
                        children: {
                            tier1_termination: { name: 'Tier 1 Termination', icon: '🏭', config: 'carrier_interconnects.termination.tier1', template: 'basic_sip', tooltip: 'Tier 1 carrier termination interconnect for premium routing' },
                            add_termination: { name: '+ Add Termination', icon: '➕', config: 'carrier_interconnects.termination.new', template: 'basic_sip', tooltip: 'Create new termination carrier interconnect' }
                        }
                    },
                    origination: {
                        name: 'Origination',
                        icon: '📥',
                        tooltip: 'Inbound traffic origination from other carriers',
                        children: {
                            wholesale_origination: { name: 'Wholesale Origination', icon: '🤝', config: 'carrier_interconnects.origination.wholesale', template: 'basic_sip', tooltip: 'Wholesale partner origination interconnect for incoming traffic' },
                            add_origination: { name: '+ Add Origination', icon: '➕', config: 'carrier_interconnects.origination.new', template: 'basic_sip', tooltip: 'Create new origination carrier interconnect' }
                        }
                    }
                }
            },
            trunks: {
                name: 'Trunks',
                icon: '🚛',
                tooltip: 'Trunk configurations for call routing, digit manipulation, and vendor/customer association',
                children: {
                    termination: {
                        name: 'Termination Trunks',
                        icon: '📤',
                        tooltip: 'Outbound trunks for terminating calls to other carriers',
                        children: {
                            tier1_premium_trunk: { name: 'Tier1 Premium Trunk', icon: '🥇', config: 'trunks.termination.tier1_premium', template: 'trunk', tooltip: 'Premium termination trunk for Tier 1 carrier with tech prefix 1001' },
                            wholesale_premium_trunk: { name: 'Wholesale Premium Trunk', icon: '🥈', config: 'trunks.termination.wholesale_premium', template: 'trunk', tooltip: 'Premium termination trunk for wholesale customers with tech prefix 1002' },
                            add_termination_trunk: { name: '+ Add Termination Trunk', icon: '➕', config: 'trunks.termination.new', template: 'trunk', tooltip: 'Create new termination trunk configuration' }
                        }
                    },
                    origination: {
                        name: 'Origination Trunks',
                        icon: '📥',
                        tooltip: 'Inbound trunks for originating calls from other carriers',
                        children: {
                            wholesale_origination_trunk: { name: 'Wholesale Origination Trunk', icon: '🤝', config: 'trunks.origination.wholesale_origination', template: 'trunk', tooltip: 'Origination trunk for wholesale partner with tech prefix 2001' },
                            add_origination_trunk: { name: '+ Add Origination Trunk', icon: '➕', config: 'trunks.origination.new', template: 'trunk', tooltip: 'Create new origination trunk configuration' }
                        }
                    }
                }
            },
            vendors_customers: {
                name: 'Vendors & Customers',
                icon: '🏢',
                tooltip: 'Vendor, customer, and partner contact information and relationships',
                children: {
                    vendors: {
                        name: 'Vendors',
                        icon: '🏭',
                        tooltip: 'Carrier vendors and service providers',
                        children: {
                            tier1_vendor: { name: 'Tier1 Carrier Corp', icon: '🔧', config: 'vendors_customers.tier1_vendor', template: 'basic_sip', tooltip: 'Tier 1 carrier vendor contact and technical information' },
                            add_vendor: { name: '+ Add Vendor', icon: '➕', config: 'vendors_customers.vendors.new', template: 'basic_sip', tooltip: 'Add new vendor configuration' }
                        }
                    },
                    customers: {
                        name: 'Customers',
                        icon: '👥',
                        tooltip: 'Customer accounts and configurations',
                        children: {
                            wholesale_customer: { name: 'Wholesale Customer Inc', icon: '💼', config: 'vendors_customers.wholesale_customer', template: 'basic_sip', tooltip: 'Wholesale customer account and contact information' },
                            add_customer: { name: '+ Add Customer', icon: '➕', config: 'vendors_customers.customers.new', template: 'basic_sip', tooltip: 'Add new customer configuration' }
                        }
                    },
                    partners: {
                        name: 'Partners', 
                        icon: '🤝',
                        tooltip: 'Strategic partners and bilateral agreements',
                        children: {
                            wholesale_partner: { name: 'Wholesale Partner LLC', icon: '🔄', config: 'vendors_customers.wholesale_partner', template: 'basic_sip', tooltip: 'Wholesale partner bilateral agreement and contact information' },
                            add_partner: { name: '+ Add Partner', icon: '➕', config: 'vendors_customers.partners.new', template: 'basic_sip', tooltip: 'Add new partner configuration' }
                        }
                    }
                }
            },
            lcr_groups: {
                name: 'LCR Groups',
                icon: '💰',
                tooltip: 'Least Cost Routing group configurations',
                children: {
                    tier1: { name: 'Tier 1 Carriers', icon: '🥇', config: 'lcr_groups.tier1', template: 'routing_lcr', tooltip: 'Premium Tier 1 carrier group for high-quality termination' },
                    wholesale: { name: 'Wholesale Partners', icon: '🥈', config: 'lcr_groups.wholesale', template: 'routing_lcr', tooltip: 'Wholesale partner group for origination traffic' },
                    add_lcr_group: { name: '+ Add LCR Group', icon: '➕', config: 'lcr_groups.new', template: 'routing_lcr', tooltip: 'Create new LCR group configuration' }
                }
            }
        }
    },
    security: {
        name: 'Security & Authentication',
        icon: '🔒',
        tooltip: 'Security features including call authentication and fraud protection',
        children: {
            stirshaken: {
                name: 'STIR/SHAKEN',
                icon: '🔐',
                template: 'stir_shaken',
                tooltip: 'Secure Telephone Identity Revisited (STIR) and Signature-based Handling of Asserted information using toKENs (SHAKEN)',
                children: {
                    certificates: { name: 'Certificates', icon: '📜', config: 'security.stirshaken.certificates', tooltip: 'Digital certificates for call authentication and identity verification' },
                    validation: { name: 'Validation', icon: '✅', config: 'security.stirshaken.validation', tooltip: 'Call validation rules and policies for authenticated calls' }
                }
            },
            fraud: {
                name: 'Fraud Protection',
                icon: '🛡️',
                template: 'security',
                tooltip: 'Anti-fraud measures and call protection mechanisms',
                children: {
                    rules: { name: 'Protection Rules', icon: '📋', config: 'security.fraud.rules', tooltip: 'Fraud detection rules and automated response actions' },
                    blacklist: { name: 'Blacklist', icon: '🚫', config: 'security.fraud.blacklist', tooltip: 'Blocked numbers and IP addresses for fraud prevention' }
                }
            }
        }
    },
    routing: {
        name: 'Call Routing',
        icon: '🛤️',
        tooltip: 'Call routing algorithms and policies for optimal path selection',
        children: {
            lcr: {
                name: 'Least Cost Routing',
                icon: '💰',
                template: 'routing_lcr',
                tooltip: 'Cost-based routing to minimize call termination expenses',
                children: {
                    routes: { name: 'Route Tables', icon: '📊', config: 'routing.lcr.routes', tooltip: 'Destination-based routing tables with cost and quality metrics' },
                    policies: { name: 'Routing Policies', icon: '📋', config: 'routing.lcr.policies', tooltip: 'Routing decision policies and failover strategies' }
                }
            }
        }
    }
};

// Template definitions with field specifications
const templateDefinitions = {
    basic_sip: {
        name: 'Basic SIP Profile',
        fields: {
            name: { type: 'text', label: 'Profile Name', required: true, description: 'Unique name for this SIP profile' },
            ip: { type: 'text', label: 'IP Address', required: true, description: 'IP address to bind SIP traffic' },
            port: { type: 'number', label: 'Port', default: 5060, description: 'SIP listen port' },
            transport: { type: 'select', label: 'Transport', default: 'udp', options: ['udp', 'tcp', 'tls'], description: 'SIP transport protocol' },
            max_sessions: { type: 'number', label: 'Max Sessions', default: 1000, description: 'Maximum concurrent sessions' },
            session_timer: { type: 'number', label: 'Session Timer', default: 1800, description: 'Session timer in seconds' },
            auth_calls: { type: 'boolean', label: 'Authenticate Calls', default: true, description: 'Require authentication for calls' },
            use_rport: { type: 'boolean', label: 'Use RPort', default: true, description: 'Enable RPort support' }
        }
    },
    stir_shaken: {
        name: 'STIR/SHAKEN Configuration',
        fields: {
            enabled: { type: 'boolean', label: 'Enable STIR/SHAKEN', required: true, description: 'Enable STIR/SHAKEN authentication' },
            cert_path: { type: 'text', label: 'Certificate Path', required: true, description: 'Path to certificate file' },
            key_path: { type: 'text', label: 'Private Key Path', required: true, description: 'Path to private key file' },
            validation_cache_ttl: { type: 'number', label: 'Cache TTL', default: 300, description: 'Validation cache TTL in seconds' },
            verification_service: { type: 'text', label: 'Verification Service URL', description: 'External verification service URL' },
            attestation_level: { type: 'select', label: 'Attestation Level', default: 'A', options: ['A', 'B', 'C'], description: 'Default attestation level' }
        }
    },
    routing_lcr: {
        name: 'Least Cost Routing',
        fields: {
            enabled: { type: 'boolean', label: 'Enable LCR', required: true, description: 'Enable least cost routing' },
            database_url: { type: 'text', label: 'Database URL', default: 'postgresql://user:pass@localhost/lcr', description: 'Database connection string' },
            route_limit: { type: 'number', label: 'Route Limit', default: 10, description: 'Maximum routes to return' },
            failover_enabled: { type: 'boolean', label: 'Enable Failover', default: true, description: 'Enable automatic failover' },
            quality_threshold: { type: 'number', label: 'Quality Threshold', default: 0.95, description: 'Minimum route quality threshold' }
        }
    },
    security: {
        name: 'Security Configuration',
        fields: {
            enabled: { type: 'boolean', label: 'Enable Security', required: true, description: 'Enable security features' },
            max_call_rate: { type: 'number', label: 'Max Call Rate', default: 100, description: 'Maximum calls per minute per IP' },
            blacklist_enabled: { type: 'boolean', label: 'Enable Blacklist', default: true, description: 'Enable IP blacklisting' },
            whitelist_mode: { type: 'boolean', label: 'Whitelist Mode', default: false, description: 'Enable whitelist-only mode' },
            failed_auth_limit: { type: 'number', label: 'Failed Auth Limit', default: 5, description: 'Maximum failed authentication attempts' },
            ban_duration: { type: 'number', label: 'Ban Duration', default: 3600, description: 'IP ban duration in seconds' }
        }
    },
    trunk: {
        name: 'Trunk Configuration',
        fields: {
            name: { type: 'text', label: 'Trunk Name', required: true, description: 'Unique name for this trunk configuration' },
            carrier_interconnect: { type: 'association', label: 'Carrier Interconnect', required: true, description: 'Associated carrier interconnect for this trunk' },
            vendor_customer: { type: 'association', label: 'Vendor/Customer', required: true, description: 'Associated vendor or customer for this trunk' },
            sip_profile_id: { type: 'association', label: 'SIP Profile', default: 1, description: 'SIP Profile association for trunk' },
            tech_prefix: { type: 'text', label: 'Technology Prefix', default: '', description: 'Technology prefix for trunk identification' },
            trunk_type: { type: 'select', label: 'Trunk Type', default: 'termination', options: ['termination', 'origination'], description: 'Trunk type (termination/origination)' },
            max_concurrent_calls: { type: 'number', label: 'Max Concurrent Calls', default: 100, description: 'Maximum concurrent calls allowed on this trunk' },
            calls_per_second: { type: 'number', label: 'Calls Per Second', default: 5, description: 'Maximum calls per second rate limit' },
            strip_digits: { type: 'number', label: 'Strip Digits', default: 0, description: 'Number of digits to strip from dialed number' },
            add_prefix: { type: 'text', label: 'Add Prefix', default: '', description: 'Prefix to add to dialed number' },
            stir_shaken_enabled: { type: 'boolean', label: 'Enable STIR/SHAKEN', default: true, description: 'Enable STIR/SHAKEN for this trunk' }
        }
    }
};

// Initialize the configuration manager
document.addEventListener('DOMContentLoaded', function() {
    console.log('🎊 Configuration Manager loaded');
    initializeConfigManager();
});

async function initializeConfigManager() {
    try {
        console.log('🚀 Initializing configuration manager...');
        
        // Load configuration data
        await loadConfiguration();
        
        // Render the configuration tree
        renderConfigTree();
        
        console.log('✅ Configuration manager initialized successfully');
        
    } catch (error) {
        console.error('❌ Failed to initialize configuration manager:', error);
        showError('Failed to initialize configuration manager: ' + error.message);
    }
}

async function loadConfiguration() {
    try {
        console.log('📡 Loading configuration from backend...');
        
        // Load current configuration
        const configResponse = await fetch('/api/switch/config/current');
        if (configResponse.ok) {
            configData = await configResponse.json();
            console.log('📋 Current configuration loaded:', configData);
        } else {
            console.log('⚠️ No current configuration found, using defaults');
            configData = {};
        }
        
        // Load available templates
        const templatesResponse = await fetch('/api/switch/config/templates');
        if (templatesResponse.ok) {
            const templates = await templatesResponse.json();
            templates.forEach(template => {
                templateData[template.name] = template;
            });
            console.log('📚 Templates loaded:', templateData);
        }
        
    } catch (error) {
        console.error('❌ Failed to load configuration:', error);
        throw error;
    }
}

function renderConfigTree() {
    const container = document.getElementById('config-tree-content');
    container.innerHTML = '';
    
    Object.entries(configTree).forEach(([key, node]) => {
        const element = createTreeNode(key, node, []);
        container.appendChild(element);
    });
}

function createTreeNode(key, node, path) {
    const div = document.createElement('div');
    div.className = 'tree-node';
    
    const item = document.createElement('div');
    item.className = 'tree-item';
    item.dataset.path = [...path, key].join('.');
    
    // Add tooltip if available
    if (node.tooltip) {
        item.title = node.tooltip;
    }
    
    if (node.children) {
        item.classList.add('has-children');
    }
    
    // Create status indicator
    const statusIndicator = document.createElement('span');
    statusIndicator.className = 'status-indicator';
    if (hasConfigForPath([...path, key])) {
        statusIndicator.classList.add('status-saved');
    } else if (node.template) {
        statusIndicator.classList.add('status-modified');
    }
    
    item.innerHTML = `
        ${statusIndicator.outerHTML}
        <span class="tree-icon">${node.icon || '📁'}</span>
        <span class="tree-label">${node.name}</span>
    `;
    
    item.addEventListener('click', (e) => {
        e.stopPropagation();
        selectTreeNode(item, [...path, key], node);
    });
    
    div.appendChild(item);
    
    // Add children if they exist
    if (node.children) {
        const childrenDiv = document.createElement('div');
        childrenDiv.className = 'tree-children';
        
        Object.entries(node.children).forEach(([childKey, childNode]) => {
            const childElement = createTreeNode(childKey, childNode, [...path, key]);
            childrenDiv.appendChild(childElement);
        });
        
        div.appendChild(childrenDiv);
    }
    
    return div;
}

function selectTreeNode(element, path, node) {
    console.log('🎯 Selected tree node:', path, node);
    
    // Update visual selection
    document.querySelectorAll('.tree-item').forEach(item => {
        item.classList.remove('selected');
    });
    element.classList.add('selected');
    
    // Toggle children visibility
    if (node.children) {
        const childrenDiv = element.parentElement.querySelector('.tree-children');
        if (childrenDiv) {
            const isExpanded = childrenDiv.classList.contains('expanded');
            childrenDiv.classList.toggle('expanded');
            element.classList.toggle('expanded');
        }
    }
    
    // Load configuration for this node if it has a config or template
    if (node.config || node.template) {
        loadNodeConfiguration(path, node);
    }
}

function loadNodeConfiguration(path, node) {
    currentPath = path;
    currentConfig = node;
    
    // Update breadcrumb
    updateBreadcrumb(path);
    
    // Get template definition
    const templateName = node.template || (node.config ? extractTemplateFromConfig(node.config) : null);
    const templateDef = templateDefinitions[templateName];
    
    if (!templateDef) {
        showError(`No template definition found for ${templateName}`);
        return;
    }
    
    // Update editor title
    document.getElementById('editor-title').textContent = node.name;
    
    // Load current values for this configuration path
    const configPath = node.config || path.join('.');
    const currentValues = getConfigValues(configPath);
    const templateValues = getTemplateValues(templateName);
    
    // Store original values for reset functionality
    originalValues = { ...currentValues };
    
    // Render the form
    renderConfigForm(templateDef, currentValues, templateValues);
    
    // Update preview
    updateConfigPreview();
    
    // Enable controls
    document.getElementById('save-btn').disabled = false;
    document.getElementById('reset-btn').disabled = false;
    updateSaveStatus(false);
}

function updateBreadcrumb(path) {
    const breadcrumb = document.getElementById('config-breadcrumb');
    const pathParts = path.map(part => part.charAt(0).toUpperCase() + part.slice(1));
    breadcrumb.innerHTML = pathParts.map((part, index) => {
        return index === pathParts.length - 1 
            ? `<span class="current">${part}</span>`
            : `<span>${part}</span>`;
    }).join(' > ');
}

function renderConfigForm(templateDef, currentValues, templateValues) {
    const container = document.getElementById('config-form-container');
    container.innerHTML = '';
    
    const form = document.createElement('div');
    form.className = 'config-form';
    
    Object.entries(templateDef.fields).forEach(([fieldName, fieldDef]) => {
        const fieldGroup = createFormField(fieldName, fieldDef, currentValues, templateValues);
        form.appendChild(fieldGroup);
    });
    
    container.appendChild(form);
}

function createFormField(fieldName, fieldDef, currentValues, templateValues) {
    const group = document.createElement('div');
    group.className = 'field-group';
    
    // Determine field value and source
    const currentValue = currentValues[fieldName];
    const templateValue = templateValues[fieldName] || fieldDef.default;
    const hasOverride = currentValue !== undefined && currentValue !== templateValue;
    
    // Create label with indicators
    const label = document.createElement('label');
    label.className = 'field-label';
    label.setAttribute('for', fieldName);
    
    const labelText = document.createElement('span');
    labelText.textContent = fieldDef.label;
    if (fieldDef.required) {
        labelText.textContent += ' *';
    }
    
    const indicator = document.createElement('span');
    indicator.className = 'template-indicator';
    
    if (hasOverride) {
        indicator.classList.add('override');
        indicator.textContent = 'Override';
    } else if (templateValue !== undefined) {
        indicator.classList.add('template');
        indicator.textContent = 'Template';
    } else {
        indicator.classList.add('custom');
        indicator.textContent = 'Custom';
    }
    
    label.appendChild(labelText);
    label.appendChild(indicator);
    
    // Create input field
    let input;
    const value = currentValue !== undefined ? currentValue : templateValue;
    
    switch (fieldDef.type) {
        case 'boolean':
            input = document.createElement('input');
            input.type = 'checkbox';
            input.checked = value || false;
            break;
            
        case 'number':
            input = document.createElement('input');
            input.type = 'number';
            input.value = value || '';
            break;
            
        case 'select':
            input = document.createElement('select');
            fieldDef.options.forEach(option => {
                const optionElement = document.createElement('option');
                optionElement.value = option;
                optionElement.textContent = option;
                optionElement.selected = option === value;
                input.appendChild(optionElement);
            });
            break;
            
        case 'association':
            input = document.createElement('select');
            // Add empty option
            const emptyOption = document.createElement('option');
            emptyOption.value = '';
            emptyOption.textContent = '-- Select --';
            input.appendChild(emptyOption);
            
            // Determine association type based on field name
            let associationType = '';
            if (fieldName === 'sip_profile_id') {
                associationType = 'sip_profile';
            } else if (fieldName === 'vendor_customer') {
                associationType = 'vendor_customer';
            } else if (fieldName === 'carrier_interconnect') {
                associationType = 'carrier_interconnect';
            }
            
            // Dynamically populate options based on association type
            const options = getAssociationOptions(associationType);
            options.forEach(option => {
                const optionElement = document.createElement('option');
                optionElement.value = option.value;
                optionElement.textContent = option.label;
                optionElement.selected = option.value === value;
                input.appendChild(optionElement);
            });
            break;
            
        default: // text
            input = document.createElement('input');
            input.type = 'text';
            input.value = value || '';
    }
    
    input.id = fieldName;
    input.name = fieldName;
    input.className = 'field-input';
    
    // Apply styling based on value source
    if (hasOverride) {
        input.classList.add('override-value');
    } else if (templateValue !== undefined) {
        input.classList.add('template-value');
    } else {
        input.classList.add('custom-value');
    }
    
    // Add change listener
    input.addEventListener('change', () => {
        onFieldChange(fieldName, input, templateValue);
    });
    
    input.addEventListener('input', () => {
        onFieldChange(fieldName, input, templateValue);
    });
    
    group.appendChild(label);
    group.appendChild(input);
    
    // Add description if available
    if (fieldDef.description) {
        const description = document.createElement('div');
        description.className = 'field-description';
        description.textContent = fieldDef.description;
        group.appendChild(description);
    }
    
    return group;
}

function getAssociationOptions(associationType) {
    if (!configData) return [];
    
    switch (associationType) {
        case 'sip_profile':
            if (configData.sip_profiles) {
                return configData.sip_profiles.map(profile => ({
                    value: profile.id || profile.name,
                    label: `${profile.name} (ID: ${profile.id})`
                }));
            }
            return [];
            
        case 'vendor_customer':
            if (configData.vendors_customers) {
                return Object.entries(configData.vendors_customers).map(([key, entity]) => ({
                    value: key,
                    label: `${entity.name} (${entity.type.toUpperCase()}) - ID: ${entity.id}`
                }));
            }
            return [];
            
        case 'carrier_interconnect':
            const options = [];
            if (configData.carrier_interconnects) {
                if (configData.carrier_interconnects.termination) {
                    configData.carrier_interconnects.termination.forEach(interconnect => {
                        options.push({
                            value: interconnect.name,
                            label: `${interconnect.name} (Termination)`
                        });
                    });
                }
                if (configData.carrier_interconnects.origination) {
                    configData.carrier_interconnects.origination.forEach(interconnect => {
                        options.push({
                            value: interconnect.name,
                            label: `${interconnect.name} (Origination)`
                        });
                    });
                }
            }
            return options;
            
        default:
            return [];
    }
}

function onFieldChange(fieldName, input, templateValue) {
    let newValue;
    
    if (input.type === 'checkbox') {
        newValue = input.checked;
    } else if (input.type === 'number') {
        newValue = input.value ? parseFloat(input.value) : null;
    } else {
        newValue = input.value;
    }
    
    // Update styling based on whether this is an override
    input.classList.remove('template-value', 'override-value', 'custom-value');
    
    if (newValue !== templateValue && templateValue !== undefined) {
        input.classList.add('override-value');
    } else if (templateValue !== undefined) {
        input.classList.add('template-value');
    } else {
        input.classList.add('custom-value');
    }
    
    // Mark as changed
    hasChanges = true;
    updateSaveStatus(true);
    
    // Update preview
    updateConfigPreview();
}

function updateSaveStatus(hasUnsavedChanges) {
    const statusIndicator = document.getElementById('save-status');
    const saveBtn = document.getElementById('save-btn');
    
    if (hasUnsavedChanges) {
        statusIndicator.className = 'status-indicator status-modified';
        saveBtn.textContent = '💾 Save Changes';
        saveBtn.disabled = false;
    } else {
        statusIndicator.className = 'status-indicator status-saved';
        saveBtn.textContent = '💾 Save';
        saveBtn.disabled = true;
    }
    
    hasChanges = hasUnsavedChanges;
}

function updateConfigPreview() {
    if (!currentConfig) return;
    
    const formData = getCurrentFormData();
    const preview = {
        path: currentPath.join('.'),
        template: currentConfig.template,
        configuration: formData
    };
    
    document.getElementById('config-preview-content').textContent = JSON.stringify(preview, null, 2);
    document.getElementById('preview-status').textContent = hasChanges ? 'Modified' : 'Current';
}

function getCurrentFormData() {
    const formData = {};
    const inputs = document.querySelectorAll('#config-form-container input, #config-form-container select');
    
    inputs.forEach(input => {
        const name = input.name;
        if (name) {
            if (input.type === 'checkbox') {
                formData[name] = input.checked;
            } else if (input.type === 'number') {
                formData[name] = input.value ? parseFloat(input.value) : null;
            } else {
                formData[name] = input.value;
            }
        }
    });
    
    return formData;
}

async function saveConfiguration() {
    if (!currentConfig || !hasChanges) return;
    
    try {
        console.log('💾 Saving configuration...');
        
        const formData = getCurrentFormData();
        const configPath = currentConfig.config || currentPath.join('.');
        
        const saveData = {
            path: configPath,
            template: currentConfig.template,
            configuration: formData,
            timestamp: new Date().toISOString()
        };
        
        const response = await fetch('/api/switch/config/save', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify(saveData)
        });
        
        if (response.ok) {
            const result = await response.json();
            console.log('✅ Configuration saved successfully:', result);
            
            // Update local state
            setConfigValues(configPath, formData);
            originalValues = { ...formData };
            
            // Update UI
            updateSaveStatus(false);
            showSuccess('Configuration saved successfully');
            
            // Refresh tree to update status indicators
            renderConfigTree();
            
        } else {
            throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }
        
    } catch (error) {
        console.error('❌ Failed to save configuration:', error);
        showError('Failed to save configuration: ' + error.message);
    }
}

function resetConfiguration() {
    if (!currentConfig) return;
    
    // Reset form values to original values
    Object.entries(originalValues).forEach(([fieldName, value]) => {
        const input = document.getElementById(fieldName);
        if (input) {
            if (input.type === 'checkbox') {
                input.checked = value || false;
            } else {
                input.value = value || '';
            }
        }
    });
    
    updateSaveStatus(false);
    updateConfigPreview();
    showSuccess('Configuration reset to saved values');
}

function copyPreview() {
    const content = document.getElementById('config-preview-content').textContent;
    navigator.clipboard.writeText(content).then(() => {
        showSuccess('Configuration copied to clipboard');
    }).catch(() => {
        showError('Failed to copy to clipboard');
    });
}

function downloadPreview() {
    const content = document.getElementById('config-preview-content').textContent;
    const blob = new Blob([content], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    
    const a = document.createElement('a');
    a.href = url;
    a.download = `redfire-config-${currentPath.join('-')}.json`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
    
    showSuccess('Configuration exported successfully');
}

// Utility functions
function getConfigValues(configPath) {
    const parts = configPath.split('.');
    let current = configData;
    
    for (const part of parts) {
        if (current && typeof current === 'object' && part in current) {
            current = current[part];
        } else {
            return {};
        }
    }
    
    return current || {};
}

function setConfigValues(configPath, values) {
    const parts = configPath.split('.');
    let current = configData;
    
    for (let i = 0; i < parts.length - 1; i++) {
        const part = parts[i];
        if (!current[part] || typeof current[part] !== 'object') {
            current[part] = {};
        }
        current = current[part];
    }
    
    current[parts[parts.length - 1]] = values;
}

function getTemplateValues(templateName) {
    const template = templateDefinitions[templateName];
    if (!template) return {};
    
    const values = {};
    Object.entries(template.fields).forEach(([fieldName, fieldDef]) => {
        if (fieldDef.default !== undefined) {
            values[fieldName] = fieldDef.default;
        }
    });
    
    return values;
}

function hasConfigForPath(path) {
    const configPath = path.join('.');
    const values = getConfigValues(configPath);
    return Object.keys(values).length > 0;
}

function extractTemplateFromConfig(configPath) {
    // Map config paths to templates (this could be more sophisticated)
    if (configPath.includes('sip.profiles')) return 'basic_sip';
    if (configPath.includes('stirshaken')) return 'stir_shaken';
    if (configPath.includes('lcr')) return 'routing_lcr';
    if (configPath.includes('security') || configPath.includes('fraud')) return 'security';
    return null;
}

function showSuccess(message) {
    // You could implement a toast notification system here
    console.log('✅ Success:', message);
}

function showError(message) {
    // You could implement a toast notification system here
    console.error('❌ Error:', message);
    alert('Error: ' + message);
}

// Load configuration data from API
async function loadConfiguration() {
    try {
        const response = await fetch('/api/switch/config/generate', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                config_type: 'full_system',
                parameters: {}
            })
        });

        if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
        }

        const result = await response.json();
        configData = result.config;
        
        console.log('Configuration loaded:', configData);
        showSuccess('Configuration reloaded successfully');
        
        // Refresh current view if we have one loaded
        if (currentConfig) {
            loadTreeItem(currentPath[currentPath.length - 1], currentPath.slice(0, -1));
        }
    } catch (error) {
        console.error('Failed to load configuration:', error);
        showError('Failed to load configuration: ' + error.message);
    }
}

// Initialize on page load
document.addEventListener('DOMContentLoaded', () => {
    console.log('Config Manager initialized');
    buildTree();
    loadConfiguration();
});

// Export functions for use in HTML
window.loadConfiguration = loadConfiguration;
window.saveConfiguration = saveConfiguration;
window.resetConfiguration = resetConfiguration;
window.copyPreview = copyPreview;
window.downloadPreview = downloadPreview;