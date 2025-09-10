// Configuration Generator JavaScript

let templates = [];
let selectedTemplate = null;
let generatedConfig = null;
let configHistory = JSON.parse(localStorage.getItem('configHistory') || '[]');

// Initialize the page
document.addEventListener('DOMContentLoaded', function() {
    console.log('🎊 Config generator page loaded');
    console.log('📍 Current URL:', window.location.href);
    console.log('🚀 About to load templates');
    
    // Load templates and history
    loadTemplates();
    loadConfigHistory();
    
    // Add navigation to existing nav links
    updateNavigation();
});

function updateNavigation() {
    // Add config generator to existing navigation
    const nav = document.querySelector('.nav');
    const configLink = nav.querySelector('a[href="/config"]');
    if (configLink && !nav.querySelector('a[href="/config-generator"]')) {
        const generatorLink = document.createElement('a');
        generatorLink.href = '/config-generator';
        generatorLink.className = 'nav-link';
        generatorLink.textContent = 'Config Generator';
        configLink.parentNode.insertBefore(generatorLink, configLink.nextSibling);
    }
}

async function loadTemplates() {
    try {
        console.log('🚀 loadTemplates called');
        const gridElement = document.getElementById('template-grid');
        console.log('📊 Grid element found:', gridElement);
        
        showLoadingState('template-grid');
        
        // Use direct fetch for better reliability
        console.log('🌐 Making direct fetch to /api/switch/config/templates');
        const response = await fetch('/api/switch/config/templates', {
            method: 'GET',
            headers: {
                'Content-Type': 'application/json',
            }
        });
        
        console.log('📡 Response status:', response.status);
        console.log('✅ Response OK:', response.ok);
        
        if (!response.ok) {
            throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }
        
        const data = await response.json();
        console.log('📦 API response data:', data);
        console.log('🔍 Response type:', typeof data);
        console.log('📋 Is array?', Array.isArray(data));
        
        if (!data || !Array.isArray(data)) {
            console.error('❌ Invalid response - data:', data, 'isArray:', Array.isArray(data));
            throw new Error('Invalid templates response - expected array');
        }
        
        templates = data;
        console.log('✨ Templates loaded successfully:', templates.length, 'templates');
        console.log('🎯 Template names:', templates.map(t => t.name).join(', '));
        console.log('🎨 About to call renderTemplates');
        renderTemplates();
        console.log('🎉 renderTemplates called successfully');
        
    } catch (error) {
        console.error('💥 Failed to load templates:', error);
        
        // Show detailed error information
        const errorMessage = error.message || 'Unknown error';
        const detailedError = `Failed to load templates: ${errorMessage}`;
        
        if (typeof showNotification !== 'undefined') {
            showNotification(detailedError, 'error');
        }
        
        document.getElementById('template-grid').innerHTML = 
            `<div class="error-message">
                <h4>Failed to load templates</h4>
                <p>Error: ${errorMessage}</p>
                <button onclick="loadTemplates()" class="action-btn secondary">🔄 Retry</button>
             </div>`;
    }
}

function renderTemplates() {
    console.log('renderTemplates called');
    const grid = document.getElementById('template-grid');
    console.log('Grid element in render:', grid);
    console.log('Templates to render:', templates);
    
    if (!templates || templates.length === 0) {
        console.log('No templates found, showing error message');
        grid.innerHTML = '<div class="error-message">No templates available</div>';
        return;
    }
    
    console.log('Generating HTML for', templates.length, 'templates');
    const html = templates.map(template => `
        <div class="template-card" onclick="selectTemplate('${template.name}')">
            <div class="template-name">${template.name.replace(/_/g, ' ').toUpperCase()}</div>
            <div class="template-description">${template.description}</div>
            <div class="template-params">
                Required: ${template.required_params.length} | 
                Optional: ${template.optional_params.length}
            </div>
        </div>
    `).join('');
    
    console.log('Generated HTML:', html.substring(0, 200) + '...');
    grid.innerHTML = html;
    console.log('Grid innerHTML updated');
}

function selectTemplate(templateName) {
    // Clear previous selection
    document.querySelectorAll('.template-card').forEach(card => {
        card.classList.remove('selected');
    });
    
    // Select new template
    event.target.closest('.template-card').classList.add('selected');
    
    selectedTemplate = templates.find(t => t.name === templateName);
    if (selectedTemplate) {
        renderParameterForm();
        document.getElementById('parameter-config').style.display = 'block';
        document.getElementById('config-title').textContent = 
            `Configure ${selectedTemplate.name.replace(/_/g, ' ').toUpperCase()}`;
    }
}

function renderParameterForm() {
    if (!selectedTemplate) return;
    
    const requiredDiv = document.getElementById('required-params');
    const optionalDiv = document.getElementById('optional-params');
    
    // Required parameters
    if (selectedTemplate.required_params.length > 0) {
        requiredDiv.innerHTML = `
            <div class="param-section">
                <h4>Required Parameters</h4>
                ${selectedTemplate.required_params.map(param => `
                    <div class="param-group">
                        <label class="param-label" for="param_${param}">
                            ${param.replace(/_/g, ' ').toUpperCase()} *
                        </label>
                        <input type="text" 
                               id="param_${param}" 
                               name="${param}" 
                               class="param-input required" 
                               required>
                    </div>
                `).join('')}
            </div>
        `;
    } else {
        requiredDiv.innerHTML = '';
    }
    
    // Optional parameters
    if (selectedTemplate.optional_params.length > 0) {
        optionalDiv.innerHTML = `
            <div class="param-section">
                <h4>Optional Parameters</h4>
                ${selectedTemplate.optional_params.map(param => {
                    const inputType = param.param_type === 'boolean' ? 'checkbox' : 
                                    param.param_type === 'number' ? 'number' : 'text';
                    const inputValue = param.default_value !== null ? param.default_value : '';
                    
                    return `
                        <div class="param-group">
                            <label class="param-label" for="param_${param.name}">
                                ${param.name.replace(/_/g, ' ').toUpperCase()}
                            </label>
                            <div class="param-description">${param.description}</div>
                            ${inputType === 'checkbox' ? 
                                `<input type="checkbox" 
                                        id="param_${param.name}" 
                                        name="${param.name}" 
                                        class="param-input"
                                        ${inputValue === true ? 'checked' : ''}>` :
                                `<input type="${inputType}" 
                                        id="param_${param.name}" 
                                        name="${param.name}" 
                                        class="param-input"
                                        value="${inputValue}"
                                        placeholder="Default: ${inputValue || 'none'}">`
                            }
                        </div>
                    `;
                }).join('')}
            </div>
        `;
    } else {
        optionalDiv.innerHTML = '';
    }
}

function resetForm() {
    if (selectedTemplate) {
        renderParameterForm();
        document.getElementById('config-preview').style.display = 'none';
        generatedConfig = null;
    }
}

async function previewConfig() {
    if (!selectedTemplate) return;
    
    try {
        const parameters = getFormParameters();
        
        // Validate required parameters
        const missingRequired = selectedTemplate.required_params.filter(param => 
            !parameters[param] || parameters[param].toString().trim() === ''
        );
        
        if (missingRequired.length > 0) {
            showNotification(`Missing required parameters: ${missingRequired.join(', ')}`, 'error');
            return;
        }
        
        // Generate preview without saving
        const previewData = await generateConfigData(parameters);
        displayConfigPreview(previewData, true);
        
    } catch (error) {
        console.error('Preview failed:', error);
        showNotification('Failed to generate preview', 'error');
    }
}

async function generateConfig() {
    if (!selectedTemplate) return;
    
    try {
        const parameters = getFormParameters();
        
        // Validate required parameters
        const missingRequired = selectedTemplate.required_params.filter(param => 
            !parameters[param] || parameters[param].toString().trim() === ''
        );
        
        if (missingRequired.length > 0) {
            showNotification(`Missing required parameters: ${missingRequired.join(', ')}`, 'error');
            return;
        }
        
        showLoadingState('config-output');
        
        const configData = await generateConfigData(parameters);
        displayConfigPreview(configData);
        
        // Save to history
        saveToHistory(configData, parameters);
        
        showNotification('Configuration generated successfully!', 'success');
        
    } catch (error) {
        console.error('Generation failed:', error);
        showNotification('Failed to generate configuration', 'error');
    }
}

async function generateConfigData(parameters) {
    console.log('🔧 generateConfigData called with parameters:', parameters);
    console.log('📋 Selected template:', selectedTemplate.name);
    
    const requestData = {
        config_type: selectedTemplate.name,
        parameters: parameters
    };
    
    console.log('📤 Sending request:', requestData);
    
    const response = await fetch('/api/switch/config/generate', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
        },
        body: JSON.stringify(requestData)
    });
    
    console.log('📡 Generation response status:', response.status);
    console.log('✅ Generation response OK:', response.ok);
    
    if (!response.ok) {
        const errorText = await response.text();
        console.error('❌ Generation failed:', errorText);
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }
    
    const data = await response.json();
    console.log('🎉 Configuration generated successfully:', data);
    
    if (!data) {
        throw new Error('No response from server');
    }
    
    return data;
}

function displayConfigPreview(configData, isPreview = false) {
    generatedConfig = configData;
    
    const configOutput = document.getElementById('config-output');
    const configFilename = document.getElementById('config-filename');
    const configSize = document.getElementById('config-size');
    
    const configJson = JSON.stringify(configData.config, null, 2);
    
    configOutput.textContent = configJson;
    configFilename.textContent = isPreview ? 
        `Preview: ${configData.filename}` : configData.filename;
    configSize.textContent = `${formatBytes(new Blob([configJson]).size)}`;
    
    document.getElementById('config-preview').style.display = 'block';
    
    // Smooth scroll to preview
    document.getElementById('config-preview').scrollIntoView({ 
        behavior: 'smooth', 
        block: 'start' 
    });
}

function getFormParameters() {
    const form = document.getElementById('config-form');
    const formData = new FormData(form);
    const parameters = {};
    
    for (let [key, value] of formData.entries()) {
        const input = form.querySelector(`[name="${key}"]`);
        
        if (input.type === 'checkbox') {
            parameters[key] = input.checked;
        } else if (input.type === 'number') {
            parameters[key] = value ? parseFloat(value) : null;
        } else {
            parameters[key] = value;
        }
    }
    
    return parameters;
}

function copyConfig() {
    if (!generatedConfig) return;
    
    const configJson = JSON.stringify(generatedConfig.config, null, 2);
    
    navigator.clipboard.writeText(configJson).then(() => {
        showNotification('Configuration copied to clipboard!', 'success');
    }).catch(() => {
        // Fallback for older browsers
        const textarea = document.createElement('textarea');
        textarea.value = configJson;
        document.body.appendChild(textarea);
        textarea.select();
        document.execCommand('copy');
        document.body.removeChild(textarea);
        showNotification('Configuration copied to clipboard!', 'success');
    });
}

function downloadConfig() {
    if (!generatedConfig) return;
    
    const configJson = JSON.stringify(generatedConfig.config, null, 2);
    const blob = new Blob([configJson], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    
    const a = document.createElement('a');
    a.href = url;
    a.download = generatedConfig.filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
    
    showNotification(`Downloaded ${generatedConfig.filename}`, 'success');
}

function downloadAllConfigs() {
    if (configHistory.length === 0) {
        showNotification('No configurations to download', 'info');
        return;
    }
    
    // Create a zip-like structure (multiple files)
    configHistory.forEach((item, index) => {
        setTimeout(() => {
            const configJson = JSON.stringify(item.config, null, 2);
            const blob = new Blob([configJson], { type: 'application/json' });
            const url = URL.createObjectURL(blob);
            
            const a = document.createElement('a');
            a.href = url;
            a.download = `${item.filename}_${index + 1}.json`;
            document.body.appendChild(a);
            a.click();
            document.body.removeChild(a);
            URL.revokeObjectURL(url);
        }, index * 500); // Stagger downloads
    });
    
    showNotification(`Downloading ${configHistory.length} configuration files`, 'success');
}

function saveToHistory(configData, parameters) {
    const historyItem = {
        id: Date.now(),
        timestamp: new Date().toISOString(),
        config_type: selectedTemplate.name,
        filename: configData.filename,
        config: configData.config,
        parameters: parameters,
        description: selectedTemplate.description
    };
    
    // Add to beginning of history
    configHistory.unshift(historyItem);
    
    // Limit history to 20 items
    if (configHistory.length > 20) {
        configHistory = configHistory.slice(0, 20);
    }
    
    // Save to localStorage
    localStorage.setItem('configHistory', JSON.stringify(configHistory));
    
    // Update UI
    loadConfigHistory();
}

function loadConfigHistory() {
    const historyList = document.getElementById('config-history-list');
    
    if (configHistory.length === 0) {
        historyList.innerHTML = '<div class="history-placeholder">No configurations generated yet</div>';
        return;
    }
    
    historyList.innerHTML = configHistory.map(item => `
        <div class="history-item" onclick="loadFromHistory(${item.id})">
            <div class="history-info">
                <div class="history-title">${item.config_type.replace(/_/g, ' ').toUpperCase()}</div>
                <div class="history-meta">
                    ${new Date(item.timestamp).toLocaleDateString()} at ${new Date(item.timestamp).toLocaleTimeString()}
                    | ${formatBytes(new Blob([JSON.stringify(item.config)]).size)}
                </div>
            </div>
            <div class="history-actions">
                <button class="action-btn-small secondary" onclick="event.stopPropagation(); downloadHistoryItem(${item.id})">
                    💾
                </button>
                <button class="action-btn-small danger" onclick="event.stopPropagation(); removeFromHistory(${item.id})">
                    🗑️
                </button>
            </div>
        </div>
    `).join('');
}

function loadFromHistory(id) {
    const item = configHistory.find(h => h.id === id);
    if (!item) return;
    
    // Find and select the template
    const template = templates.find(t => t.name === item.config_type);
    if (!template) {
        showNotification('Template not found for this configuration', 'error');
        return;
    }
    
    selectedTemplate = template;
    
    // Update UI
    document.querySelectorAll('.template-card').forEach(card => card.classList.remove('selected'));
    const templateCard = Array.from(document.querySelectorAll('.template-card'))
        .find(card => card.textContent.includes(template.name.replace(/_/g, ' ').toUpperCase()));
    if (templateCard) templateCard.classList.add('selected');
    
    // Render form and populate with saved parameters
    renderParameterForm();
    
    // Populate form fields
    setTimeout(() => {
        Object.entries(item.parameters).forEach(([key, value]) => {
            const input = document.getElementById(`param_${key}`);
            if (input) {
                if (input.type === 'checkbox') {
                    input.checked = value;
                } else {
                    input.value = value;
                }
            }
        });
        
        // Display the configuration
        displayConfigPreview({
            config: item.config,
            filename: item.filename,
            config_type: item.config_type
        });
        
        document.getElementById('parameter-config').style.display = 'block';
        document.getElementById('config-title').textContent = 
            `Configure ${template.name.replace(/_/g, ' ').toUpperCase()} (From History)`;
            
    }, 100);
    
    showNotification('Configuration loaded from history', 'success');
}

function downloadHistoryItem(id) {
    const item = configHistory.find(h => h.id === id);
    if (!item) return;
    
    const configJson = JSON.stringify(item.config, null, 2);
    const blob = new Blob([configJson], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    
    const a = document.createElement('a');
    a.href = url;
    a.download = item.filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
    
    showNotification(`Downloaded ${item.filename}`, 'success');
}

function removeFromHistory(id) {
    if (confirm('Remove this configuration from history?')) {
        configHistory = configHistory.filter(h => h.id !== id);
        localStorage.setItem('configHistory', JSON.stringify(configHistory));
        loadConfigHistory();
        showNotification('Configuration removed from history', 'success');
    }
}

function clearHistory() {
    if (confirm('Clear all configuration history? This cannot be undone.')) {
        configHistory = [];
        localStorage.removeItem('configHistory');
        loadConfigHistory();
        showNotification('Configuration history cleared', 'success');
    }
}

// Utility functions
function showLoadingState(elementId) {
    const element = document.getElementById(elementId);
    if (element) {
        element.innerHTML = '<div class="loading-spinner">Loading...</div>';
    }
}

function formatBytes(bytes) {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

// Add some CSS for loading and error states
const style = document.createElement('style');
style.textContent = `
    .loading-spinner {
        text-align: center;
        padding: 2rem;
        color: var(--text-muted);
    }
    
    .error-message {
        text-align: center;
        padding: 2rem;
        color: var(--error-color);
        background: rgba(220, 53, 69, 0.1);
        border: 1px solid rgba(220, 53, 69, 0.2);
        border-radius: var(--border-radius);
    }
    
    .action-btn-small {
        padding: 0.5rem;
        font-size: 0.85rem;
        min-width: auto;
        width: 2.5rem;
        height: 2.5rem;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        border-radius: 50%;
    }
    
    .param-input:invalid {
        border-color: var(--error-color);
    }
    
    .param-input:valid.required {
        border-left-color: var(--success-color);
    }
`;
document.head.appendChild(style);