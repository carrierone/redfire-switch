/**
 * RedFire Switch Web UI - Calls Page JavaScript
 */

let callsRefreshInterval;
let allCalls = [];
let filteredCalls = [];

document.addEventListener('DOMContentLoaded', function() {
    initializeCallsPage();
});

async function initializeCallsPage() {
    // Load initial data
    await loadActiveCalls();
    
    // Set up auto-refresh every 3 seconds
    callsRefreshInterval = setInterval(loadActiveCalls, 3000);
    
    // Set up modal event handlers
    setupModalHandlers();
}

async function loadActiveCalls() {
    try {
        const calls = await getActiveCalls();
        allCalls = calls || [];
        filteredCalls = [...allCalls];
        
        updateCallsTable();
        updateCallsStats();
        
    } catch (error) {
        console.error('Failed to load active calls:', error);
        showCallsError('Failed to load active calls');
    }
}

function updateCallsTable() {
    const tbody = document.getElementById('calls-tbody');
    if (!tbody) return;
    
    if (filteredCalls.length === 0) {
        tbody.innerHTML = '<tr><td colspan="8" class="loading">No active calls found</td></tr>';
        return;
    }
    
    tbody.innerHTML = filteredCalls.map(call => `
        <tr>
            <td>
                <button class="call-id-btn" onclick="showCallDetails('${call.call_id}')">
                    ${call.call_id}
                </button>
            </td>
            <td>${call.from_number}</td>
            <td>${call.to_number}</td>
            <td>
                <span class="status-badge status-${call.status.toLowerCase()}">
                    ${call.status}
                </span>
            </td>
            <td>${formatDateTime(call.start_time)}</td>
            <td>${call.duration ? formatDuration(call.duration) : formatDuration(getCallDuration(call.start_time))}</td>
            <td>${call.trunk_info ? call.trunk_info.ingress_trunk_id : 'N/A'}</td>
            <td>
                <div class="call-actions">
                    <button class="action-btn-small primary" onclick="showCallDetails('${call.call_id}')" title="View Details">
                        👁️
                    </button>
                    <button class="action-btn-small danger" onclick="initiateHangup('${call.call_id}', '${call.from_number}', '${call.to_number}')" title="Hangup">
                        📞
                    </button>
                </div>
            </td>
        </tr>
    `).join('');
}

function updateCallsStats() {
    // Update stats row
    document.getElementById('total-active-calls').textContent = allCalls.length;
    
    // Calculate calls per minute
    const now = new Date();
    const oneMinuteAgo = new Date(now.getTime() - 60000);
    const recentCalls = allCalls.filter(call => {
        const callTime = new Date(call.start_time);
        return callTime >= oneMinuteAgo;
    });
    document.getElementById('calls-per-minute').textContent = recentCalls.length;
    
    // Calculate average duration
    const completedCalls = allCalls.filter(call => call.duration);
    const avgDuration = completedCalls.length > 0 
        ? completedCalls.reduce((sum, call) => sum + call.duration, 0) / completedCalls.length 
        : 0;
    document.getElementById('avg-duration').textContent = formatDuration(Math.round(avgDuration));
    
    // Calculate success rate
    const successfulCalls = allCalls.filter(call => call.status === 'Answered' || call.status === 'Completed');
    const successRate = allCalls.length > 0 ? (successfulCalls.length / allCalls.length * 100).toFixed(1) : 100;
    document.getElementById('success-rate').textContent = `${successRate}%`;
}

function getCallDuration(startTime) {
    const now = new Date();
    const start = new Date(startTime);
    return Math.floor((now - start) / 1000);
}

function filterCalls() {
    const statusFilter = document.getElementById('status-filter').value.toLowerCase();
    const searchFilter = document.getElementById('search-filter').value.toLowerCase();
    
    filteredCalls = allCalls.filter(call => {
        const matchesStatus = !statusFilter || call.status.toLowerCase() === statusFilter;
        const matchesSearch = !searchFilter || 
            call.from_number.toLowerCase().includes(searchFilter) ||
            call.to_number.toLowerCase().includes(searchFilter) ||
            call.call_id.toLowerCase().includes(searchFilter);
        
        return matchesStatus && matchesSearch;
    });
    
    updateCallsTable();
}

function clearFilters() {
    document.getElementById('status-filter').value = '';
    document.getElementById('search-filter').value = '';
    filteredCalls = [...allCalls];
    updateCallsTable();
}

function showCallDetails(callId) {
    const call = allCalls.find(c => c.call_id === callId);
    if (!call) {
        showNotification('Call not found', 'error');
        return;
    }
    
    const modal = document.getElementById('call-details-modal');
    const content = document.getElementById('call-details-content');
    
    content.innerHTML = `
        <div class="detail-item">
            <strong>Call ID:</strong>
            <span>${call.call_id}</span>
        </div>
        <div class="detail-item">
            <strong>From:</strong>
            <span>${call.from_number}</span>
        </div>
        <div class="detail-item">
            <strong>To:</strong>
            <span>${call.to_number}</span>
        </div>
        <div class="detail-item">
            <strong>Status:</strong>
            <span class="status-badge status-${call.status.toLowerCase()}">${call.status}</span>
        </div>
        <div class="detail-item">
            <strong>Direction:</strong>
            <span>${call.direction || 'Inbound'}</span>
        </div>
        <div class="detail-item">
            <strong>Start Time:</strong>
            <span>${formatDateTime(call.start_time)}</span>
        </div>
        <div class="detail-item">
            <strong>Duration:</strong>
            <span>${call.duration ? formatDuration(call.duration) : formatDuration(getCallDuration(call.start_time))}</span>
        </div>
        <div class="detail-item">
            <strong>Ingress Trunk:</strong>
            <span>${call.trunk_info ? call.trunk_info.ingress_trunk_id : 'N/A'}</span>
        </div>
        <div class="detail-item">
            <strong>Egress Trunk:</strong>
            <span>${call.trunk_info && call.trunk_info.egress_trunk_id ? call.trunk_info.egress_trunk_id : 'N/A'}</span>
        </div>
        <div class="detail-item">
            <strong>Trunk Type:</strong>
            <span>${call.trunk_info ? call.trunk_info.trunk_type : 'N/A'}</span>
        </div>
    `;
    
    // Store current call for hangup action
    modal.dataset.callId = callId;
    
    showElement(modal);
}

function initiateHangup(callId, fromNumber, toNumber) {
    const modal = document.getElementById('hangup-modal');
    
    document.getElementById('hangup-call-id').textContent = callId;
    document.getElementById('hangup-from').textContent = fromNumber;
    document.getElementById('hangup-to').textContent = toNumber;
    
    modal.dataset.callId = callId;
    
    showElement(modal);
}

async function confirmHangup() {
    const modal = document.getElementById('hangup-modal');
    const callId = modal.dataset.callId;
    
    if (!callId) return;
    
    try {
        await apiCall(`/calls/${callId}/hangup`, { method: 'POST' });
        showNotification('Call hung up successfully', 'success');
        closeHangupModal();
        
        // Refresh calls immediately
        await loadActiveCalls();
        
    } catch (error) {
        console.error('Failed to hangup call:', error);
        showNotification('Failed to hangup call', 'error');
    }
}

function closeModal() {
    hideElement(document.getElementById('call-details-modal'));
}

function closeHangupModal() {
    hideElement(document.getElementById('hangup-modal'));
}

function setupModalHandlers() {
    // Close modals when clicking outside
    document.addEventListener('click', function(e) {
        if (e.target.classList.contains('modal')) {
            hideElement(e.target);
        }
    });
    
    // Close modals with Escape key
    document.addEventListener('keydown', function(e) {
        if (e.key === 'Escape') {
            hideElement(document.getElementById('call-details-modal'));
            hideElement(document.getElementById('hangup-modal'));
        }
    });
}

function showCallsError(message) {
    const tbody = document.getElementById('calls-tbody');
    if (tbody) {
        tbody.innerHTML = `<tr><td colspan="8" class="text-center text-muted">❌ ${message}</td></tr>`;
    }
}

// Global functions
window.filterCalls = filterCalls;
window.clearFilters = clearFilters;
window.showCallDetails = showCallDetails;
window.initiateHangup = initiateHangup;
window.confirmHangup = confirmHangup;
window.closeModal = closeModal;
window.closeHangupModal = closeHangupModal;
window.loadActiveCalls = loadActiveCalls;

// Add additional styles
if (!document.querySelector('#calls-styles')) {
    const styles = document.createElement('style');
    styles.id = 'calls-styles';
    styles.textContent = `
        .call-id-btn {
            background: none;
            border: none;
            color: var(--primary-color);
            cursor: pointer;
            text-decoration: underline;
            font-family: monospace;
        }
        
        .call-id-btn:hover {
            color: var(--primary-dark);
        }
        
        .call-actions {
            display: flex;
            gap: 0.5rem;
        }
        
        .action-btn-small {
            padding: 0.25rem 0.5rem;
            font-size: 0.875rem;
            border: none;
            border-radius: 4px;
            cursor: pointer;
            background: var(--primary-color);
            color: white;
        }
        
        .action-btn-small.danger {
            background: var(--danger-color);
        }
        
        .action-btn-small:hover {
            opacity: 0.8;
        }
        
        .detail-item {
            display: flex;
            justify-content: space-between;
            padding: 0.75rem 0;
            border-bottom: 1px solid var(--border-color);
        }
        
        .detail-item:last-child {
            border-bottom: none;
        }
    `;
    document.head.appendChild(styles);
}

// Cleanup when leaving the page
window.addEventListener('beforeunload', function() {
    if (callsRefreshInterval) {
        clearInterval(callsRefreshInterval);
    }
});