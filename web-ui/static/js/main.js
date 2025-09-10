/**
 * RedFire Switch Web UI - Main JavaScript
 */

// API base URL for switch communication
const API_BASE = '/api/switch';

// Global state
let currentUser = null;
let refreshIntervals = [];

// Utility functions
function showElement(element) {
    if (element) element.classList.remove('hidden');
}

function hideElement(element) {
    if (element) element.classList.add('hidden');
}

function formatDuration(seconds) {
    if (!seconds || seconds === 0) return '0s';
    
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    
    if (hours > 0) {
        return `${hours}h ${minutes}m ${secs}s`;
    } else if (minutes > 0) {
        return `${minutes}m ${secs}s`;
    } else {
        return `${secs}s`;
    }
}

function formatDateTime(dateString) {
    if (!dateString) return 'N/A';
    const date = new Date(dateString);
    return date.toLocaleString();
}

function formatNumber(num) {
    if (typeof num !== 'number') return '0';
    return new Intl.NumberFormat().format(num);
}

// API communication functions
async function apiCall(endpoint, options = {}) {
    const url = `${API_BASE}${endpoint}`;
    const defaultOptions = {
        headers: {
            'Content-Type': 'application/json',
        },
    };
    
    // Add authentication header if user is logged in
    if (currentUser && currentUser.token) {
        defaultOptions.headers['Authorization'] = `Bearer ${currentUser.token}`;
    }
    
    const finalOptions = { ...defaultOptions, ...options };
    
    try {
        const response = await fetch(url, finalOptions);
        
        if (!response.ok) {
            if (response.status === 401) {
                // Unauthorized - redirect to login
                window.location.href = '/login';
                return null;
            }
            throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }
        
        return await response.json();
    } catch (error) {
        console.error('API call failed:', error);
        showNotification(`API Error: ${error.message}`, 'error');
        throw error;
    }
}

// Authentication functions
function isAuthenticated() {
    const session = localStorage.getItem('redfire_session');
    if (!session) return false;
    
    try {
        const sessionData = JSON.parse(session);
        const now = new Date();
        const expiresAt = new Date(sessionData.expires_at);
        
        if (now >= expiresAt) {
            localStorage.removeItem('redfire_session');
            return false;
        }
        
        currentUser = sessionData;
        return true;
    } catch (error) {
        localStorage.removeItem('redfire_session');
        return false;
    }
}

function checkAuthentication() {
    // For demo purposes, we'll just check if there's a session in localStorage
    // In a real implementation, this would validate the token with the server
    const session = localStorage.getItem('redfire_session');
    if (session) {
        try {
            currentUser = JSON.parse(session);
            return true;
        } catch (error) {
            console.error('Invalid session data:', error);
            localStorage.removeItem('redfire_session');
            return false;
        }
    }
    return false;
}

function logout() {
    // Clear local session
    localStorage.removeItem('redfire_session');
    currentUser = null;
    
    // Clear any intervals
    refreshIntervals.forEach(clearInterval);
    refreshIntervals = [];
    
    // Redirect to login
    window.location.href = '/login';
}

// Notification system
function showNotification(message, type = 'info', duration = 5000) {
    // Remove existing notifications
    const existing = document.querySelector('.notification');
    if (existing) {
        existing.remove();
    }
    
    const notification = document.createElement('div');
    notification.className = `notification notification-${type}`;
    notification.innerHTML = `
        <span>${message}</span>
        <button onclick="this.parentElement.remove()">&times;</button>
    `;
    
    // Add styles if not already added
    if (!document.querySelector('#notification-styles')) {
        const styles = document.createElement('style');
        styles.id = 'notification-styles';
        styles.textContent = `
            .notification {
                position: fixed;
                top: 20px;
                right: 20px;
                padding: 1rem 1.5rem;
                border-radius: 8px;
                color: white;
                font-weight: 500;
                z-index: 1001;
                display: flex;
                align-items: center;
                gap: 1rem;
                max-width: 400px;
                box-shadow: 0 4px 15px rgba(0, 0, 0, 0.2);
                animation: slideIn 0.3s ease;
            }
            .notification-info { background: #17a2b8; }
            .notification-success { background: #28a745; }
            .notification-warning { background: #ffc107; color: #212529; }
            .notification-error { background: #dc3545; }
            .notification button {
                background: none;
                border: none;
                color: inherit;
                font-size: 1.2rem;
                cursor: pointer;
                padding: 0;
                line-height: 1;
            }
            @keyframes slideIn {
                from { transform: translateX(100%); opacity: 0; }
                to { transform: translateX(0); opacity: 1; }
            }
        `;
        document.head.appendChild(styles);
    }
    
    document.body.appendChild(notification);
    
    if (duration > 0) {
        setTimeout(() => {
            if (notification.parentElement) {
                notification.remove();
            }
        }, duration);
    }
}

// System stats functions
async function getSystemStats() {
    try {
        return await apiCall('/system/stats');
    } catch (error) {
        console.error('Failed to get system stats:', error);
        return null;
    }
}

async function getActiveCalls() {
    try {
        return await apiCall('/calls');
    } catch (error) {
        console.error('Failed to get active calls:', error);
        return [];
    }
}

// Initialize page functionality
function initializePage() {
    // Check authentication for protected pages
    if (!window.location.pathname.includes('/login')) {
        if (!isAuthenticated()) {
            window.location.href = '/login';
            return;
        }
    }
    
    // Set up logout button
    const logoutBtn = document.getElementById('logout-btn');
    if (logoutBtn) {
        logoutBtn.addEventListener('click', logout);
    }
    
    // Test connection on page load
    testConnection();
}

async function testConnection() {
    try {
        await apiCall('/system/stats');
        updateConnectionStatus('connected', 'Connected to RedFire Switch');
    } catch (error) {
        updateConnectionStatus('error', 'Connection failed');
    }
}

function updateConnectionStatus(status, message) {
    const statusDot = document.getElementById('status-dot');
    const connectionText = document.getElementById('connection-text');
    
    if (statusDot) {
        statusDot.className = `status-dot ${status}`;
    }
    
    if (connectionText) {
        connectionText.textContent = message;
    }
    
    // Also update any connection status elements
    const connectionStatus = document.getElementById('connection-status');
    if (connectionStatus && status === 'connected') {
        connectionStatus.textContent = 'Unix Socket';
        connectionStatus.className = 'info-value status-connected';
    }
}

// Initialize when DOM is loaded
document.addEventListener('DOMContentLoaded', initializePage);

// Global functions for buttons
window.refreshCalls = async function() {
    showNotification('Refreshing calls...', 'info', 2000);
    if (window.loadActiveCalls) {
        await window.loadActiveCalls();
    }
    showNotification('Calls refreshed', 'success', 2000);
};

window.viewActiveCalls = function() {
    window.location.href = '/calls';
};

window.reloadConfig = async function() {
    try {
        await apiCall('/system/config/reload', { method: 'POST' });
        showNotification('Configuration reloaded successfully', 'success');
    } catch (error) {
        showNotification('Failed to reload configuration', 'error');
    }
};

window.viewLogs = function() {
    showNotification('Log viewer not yet implemented', 'info');
};

window.exportStats = function() {
    showNotification('Export functionality not yet implemented', 'info');
};

window.exportCalls = function() {
    showNotification('Export functionality not yet implemented', 'info');
};