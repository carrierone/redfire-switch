/**
 * RedFire Switch Web UI - Login Page JavaScript
 */

document.addEventListener('DOMContentLoaded', function() {
    // Check if already authenticated
    if (isAuthenticated()) {
        window.location.href = '/';
        return;
    }
    
    // Initialize login form
    initializeLoginForm();
    
    // Test connection to switch
    testSwitchConnection();
});

function initializeLoginForm() {
    const loginForm = document.getElementById('login-form');
    const loginBtn = document.getElementById('login-btn');
    const loginError = document.getElementById('login-error');
    
    loginForm.addEventListener('submit', async function(e) {
        e.preventDefault();
        
        const username = document.getElementById('username').value.trim();
        const password = document.getElementById('password').value;
        
        if (!username || !password) {
            showLoginError('Please enter both username and password');
            return;
        }
        
        // Show loading state
        setLoginLoading(true);
        hideElement(loginError);
        
        try {
            const response = await fetch('/api/login', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({ username, password }),
            });
            
            if (!response.ok) {
                throw new Error(`Login failed: ${response.status}`);
            }
            
            const result = await response.json();
            
            if (result.success && result.data) {
                // Store session data
                localStorage.setItem('redfire_session', JSON.stringify(result.data));
                
                // Show success and redirect
                showNotification('Login successful! Redirecting...', 'success', 2000);
                setTimeout(() => {
                    window.location.href = '/';
                }, 1000);
            } else {
                throw new Error(result.error || 'Login failed');
            }
            
        } catch (error) {
            console.error('Login error:', error);
            showLoginError('Invalid credentials or connection error');
            setLoginLoading(false);
        }
    });
    
    // Auto-fill default credentials for demo
    document.getElementById('username').value = 'admin';
    document.getElementById('password').value = 'admin123';
}

function setLoginLoading(loading) {
    const loginBtn = document.getElementById('login-btn');
    const btnText = loginBtn.querySelector('.btn-text');
    const btnSpinner = loginBtn.querySelector('.btn-spinner');
    
    if (loading) {
        loginBtn.disabled = true;
        hideElement(btnText);
        showElement(btnSpinner);
    } else {
        loginBtn.disabled = false;
        showElement(btnText);
        hideElement(btnSpinner);
    }
}

function showLoginError(message) {
    const loginError = document.getElementById('login-error');
    loginError.textContent = `❌ ${message}`;
    showElement(loginError);
}

async function testSwitchConnection() {
    const statusDot = document.getElementById('status-dot');
    const connectionText = document.getElementById('connection-text');
    
    // Initial connecting state
    updateConnectionStatus('connecting', 'Connecting to switch...');
    
    try {
        // Test connection to the switch API via our proxy
        const response = await fetch('/api/switch/system/stats', {
            method: 'GET',
            headers: {
                'Content-Type': 'application/json',
            },
        });
        
        if (response.ok) {
            updateConnectionStatus('connected', 'Connected to RedFire Switch');
        } else if (response.status === 401) {
            updateConnectionStatus('connected', 'Switch connected (authentication required)');
        } else {
            throw new Error(`HTTP ${response.status}`);
        }
        
    } catch (error) {
        console.error('Connection test failed:', error);
        updateConnectionStatus('error', 'Cannot connect to switch');
        
        // Show additional error info
        setTimeout(() => {
            const errorDetails = document.createElement('div');
            errorDetails.style.cssText = `
                color: white;
                font-size: 0.8rem;
                text-align: center;
                margin-top: 0.5rem;
                opacity: 0.8;
            `;
            errorDetails.textContent = 'Check that RedFire Switch is running';
            
            const connectionStatus = document.getElementById('connection-status');
            if (connectionStatus && !connectionStatus.querySelector('.error-details')) {
                errorDetails.className = 'error-details';
                connectionStatus.appendChild(errorDetails);
            }
        }, 1000);
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
}