/**
 * RedFire Switch Web UI - Dashboard JavaScript
 */

let dashboardRefreshInterval;

document.addEventListener('DOMContentLoaded', function() {
    initializeDashboard();
});

async function initializeDashboard() {
    // Load initial data
    await loadDashboardData();
    
    // Set up auto-refresh every 5 seconds
    dashboardRefreshInterval = setInterval(loadDashboardData, 5000);
}

async function loadDashboardData() {
    try {
        // Load system stats
        const stats = await getSystemStats();
        if (stats) {
            updateSystemStats(stats);
        }
        
        // Load active calls
        const calls = await getActiveCalls();
        if (calls) {
            updateRecentCalls(calls);
            updateCallStats(calls);
        }
        
        // Load trunk status
        await loadTrunkStatus();
        
    } catch (error) {
        console.error('Failed to load dashboard data:', error);
    }
}

function updateSystemStats(stats) {
    // Update status cards
    updateElement('active-calls', stats.active_calls || 0);
    updateElement('calls-per-second', calculateCallsPerSecond(stats));
    updateElement('memory-usage', formatMemoryUsage(stats.memory_usage));
    updateElement('system-status', 'Online');
    updateElement('uptime', `↑ ${formatDuration(stats.uptime_seconds || 0)}`);
    
    // Update system info
    updateElement('system-uptime', formatDuration(stats.uptime_seconds || 0));
    updateElement('total-calls', formatNumber(stats.total_calls || 0));
}

function updateRecentCalls(calls) {
    const tbody = document.querySelector('#recent-calls tbody');
    if (!tbody) return;
    
    if (!calls || calls.length === 0) {
        tbody.innerHTML = '<tr><td colspan="5" class="text-center text-muted">No recent calls</td></tr>';
        return;
    }
    
    // Show only the most recent 10 calls
    const recentCalls = calls.slice(0, 10);
    
    tbody.innerHTML = recentCalls.map(call => `
        <tr>
            <td>${formatDateTime(call.start_time)}</td>
            <td>${call.from_number}</td>
            <td>${call.to_number}</td>
            <td><span class="status-badge status-${call.status.toLowerCase()}">${call.status}</span></td>
            <td>${call.duration ? formatDuration(call.duration) : 'Ongoing'}</td>
        </tr>
    `).join('');
}

function updateCallStats(calls) {
    if (!calls) return;
    
    const activeCalls = calls.filter(call => call.status === 'Answered' || call.status === 'Ringing').length;
    const totalCalls = calls.length;
    
    // Calculate calls per minute (simplified)
    const now = new Date();
    const oneMinuteAgo = new Date(now.getTime() - 60000);
    const recentCalls = calls.filter(call => {
        const callTime = new Date(call.start_time);
        return callTime >= oneMinuteAgo;
    });
    
    updateElement('calls-per-second', Math.round(recentCalls.length / 60 * 10) / 10);
}

async function loadTrunkStatus() {
    try {
        // For now, show sample trunk data
        // In a real implementation, this would come from the API
        const trunkContainer = document.getElementById('trunk-status');
        if (!trunkContainer) return;
        
        const sampleTrunks = [
            { name: 'Trunk-01', status: 'online', calls: 12 },
            { name: 'Trunk-02', status: 'online', calls: 8 },
            { name: 'Trunk-03', status: 'offline', calls: 0 },
        ];
        
        trunkContainer.innerHTML = sampleTrunks.map(trunk => `
            <div class="trunk-item">
                <div>
                    <div class="trunk-name">${trunk.name}</div>
                    <div class="trunk-calls">${trunk.calls} active calls</div>
                </div>
                <span class="trunk-status ${trunk.status}">
                    ${trunk.status.toUpperCase()}
                </span>
            </div>
        `).join('');
        
    } catch (error) {
        console.error('Failed to load trunk status:', error);
        const trunkContainer = document.getElementById('trunk-status');
        if (trunkContainer) {
            trunkContainer.innerHTML = '<div class="text-center text-muted">Failed to load trunk status</div>';
        }
    }
}

function calculateCallsPerSecond(stats) {
    // This is a simplified calculation
    // In a real implementation, you'd track this over time
    if (!stats.active_calls) return 0;
    return (stats.active_calls / 60).toFixed(1);
}

function formatMemoryUsage(memoryUsage) {
    if (!memoryUsage) return 'N/A';
    
    if (typeof memoryUsage === 'object') {
        const used = memoryUsage.used_mb || 0;
        const total = memoryUsage.total_mb || 100;
        const percentage = Math.round((used / total) * 100);
        return `${used} MB (${percentage}%)`;
    }
    
    return String(memoryUsage);
}

function updateElement(id, value) {
    const element = document.getElementById(id);
    if (element) {
        element.textContent = value;
    }
}

// Add styles for status badges
if (!document.querySelector('#dashboard-styles')) {
    const styles = document.createElement('style');
    styles.id = 'dashboard-styles';
    styles.textContent = `
        .status-badge {
            padding: 0.25rem 0.75rem;
            border-radius: 20px;
            font-size: 0.875rem;
            font-weight: 500;
            text-transform: uppercase;
        }
        .status-answered { background: #d4edda; color: #155724; }
        .status-ringing { background: #fff3cd; color: #856404; }
        .status-busy { background: #f8d7da; color: #721c24; }
        .status-failed { background: #f8d7da; color: #721c24; }
        .status-completed { background: #d1ecf1; color: #0c5460; }
        
        .trunk-calls {
            font-size: 0.875rem;
            color: var(--text-muted);
        }
    `;
    document.head.appendChild(styles);
}

// Cleanup when leaving the page
window.addEventListener('beforeunload', function() {
    if (dashboardRefreshInterval) {
        clearInterval(dashboardRefreshInterval);
    }
});