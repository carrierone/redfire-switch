/**
 * Anti-Fraud Monitoring Admin Interface
 * ECPA-compliant interface for viewing call recordings and transcriptions
 */

class AntiFraudMonitoringUI {
    constructor() {
        this.apiBaseUrl = '/api';
        this.currentUser = null;
        this.currentPage = 1;
        this.pageSize = 20;
        this.filters = {
            dateFrom: null,
            dateTo: null,
            trunkId: null,
            riskScoreMin: null,
            storageType: null,
            requiresReview: null
        };

        this.init();
    }

    async init() {
        await this.checkAuthentication();
        this.setupEventListeners();
        this.loadDashboard();
    }

    async checkAuthentication() {
        try {
            // For demo purposes, use a mock user since we don't have auth implemented
            // In production, this would check a real authentication endpoint
            this.currentUser = {
                name: 'Demo User',
                role: 'admin',
                permissions: ['view_recordings', 'manage_legal_hold']
            };
            this.updateUserInfo();
        } catch (error) {
            console.error('Authentication check failed:', error);
            // For demo, continue without auth
            this.currentUser = { name: 'Demo User', role: 'admin' };
        }
    }

    updateUserInfo() {
        const userElement = document.getElementById('current-user');
        if (userElement && this.currentUser) {
            userElement.textContent = `${this.currentUser.name} (${this.currentUser.role})`;
        }
    }

    setupEventListeners() {
        // Navigation
        document.getElementById('nav-dashboard')?.addEventListener('click', () => this.showDashboard());
        document.getElementById('nav-recordings')?.addEventListener('click', () => this.showRecordings());
        document.getElementById('nav-transcriptions')?.addEventListener('click', () => this.showTranscriptions());
        document.getElementById('nav-alerts')?.addEventListener('click', () => this.showAlerts());
        document.getElementById('nav-settings')?.addEventListener('click', () => this.showSettings());

        // Filters
        document.getElementById('apply-filters')?.addEventListener('click', () => this.applyFilters());
        document.getElementById('reset-filters')?.addEventListener('click', () => this.resetFilters());

        // Pagination
        document.getElementById('prev-page')?.addEventListener('click', () => this.previousPage());
        document.getElementById('next-page')?.addEventListener('click', () => this.nextPage());

        // Real-time updates
        this.setupWebSocket();
    }

    setupWebSocket() {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${protocol}//${window.location.host}/ws/anti-fraud`;

        this.ws = new WebSocket(wsUrl);

        this.ws.onmessage = (event) => {
            const data = JSON.parse(event.data);
            this.handleRealtimeUpdate(data);
        };

        this.ws.onclose = () => {
            console.log('WebSocket connection closed, attempting to reconnect...');
            setTimeout(() => this.setupWebSocket(), 5000);
        };
    }

    handleRealtimeUpdate(data) {
        switch (data.type) {
            case 'new_alert':
                this.addAlert(data.alert);
                this.updateAlertCount();
                break;
            case 'recording_completed':
                this.updateRecordingStatus(data.recording);
                break;
            case 'transcription_completed':
                this.updateTranscriptionStatus(data.transcription);
                break;
        }
    }

    async loadDashboard() {
        try {
            const [statistics, recentAlerts, systemStatus] = await Promise.all([
                this.fetchStatistics(),
                this.fetchRecentAlerts(),
                this.fetchSystemStatus()
            ]);

            this.renderDashboardSafe(statistics, recentAlerts, systemStatus);
        } catch (error) {
            console.error('Failed to load dashboard:', error);
            this.showError('Failed to load dashboard data');
        }
    }

    async fetchStatistics() {
        const response = await fetch(`${this.apiBaseUrl}/stats?days=7`);
        if (!response.ok) throw new Error('Failed to fetch statistics');
        return await response.json();
    }

    async fetchRecentAlerts() {
        const response = await fetch(`${this.apiBaseUrl}/events/recent?limit=10`);
        if (!response.ok) throw new Error('Failed to fetch alerts');
        return await response.json() || [];
    }

    async fetchSystemStatus() {
        const response = await fetch(`${this.apiBaseUrl}/health`);
        if (!response.ok) throw new Error('Failed to fetch system status');
        return await response.json();
    }

    renderDashboard(statistics, recentAlerts, systemStatus) {
        const content = document.getElementById('main-content');
        content.innerHTML = `
            <div class="dashboard">
                <h2>Anti-Fraud Monitoring Dashboard</h2>

                <!-- ECPA Compliance Notice -->
                <div class="compliance-notice">
                    <i class="icon-shield"></i>
                    <strong>ECPA Compliance Active</strong> -
                    Monitoring under 18 U.S.C. § 2511(2)(a)(i) for fraud prevention.
                    Legal Basis: Provider Exception
                </div>

                <!-- Statistics Cards -->
                <div class="stats-grid">
                    <div class="stat-card">
                        <h3>Total Calls Monitored</h3>
                        <div class="stat-value">${statistics.totalCallsMonitored.toLocaleString()}</div>
                        <div class="stat-change ${statistics.callsChangePercent >= 0 ? 'positive' : 'negative'}">
                            ${statistics.callsChangePercent >= 0 ? '+' : ''}${statistics.callsChangePercent}% from last week
                        </div>
                    </div>

                    <div class="stat-card">
                        <h3>Fraud Alerts Generated</h3>
                        <div class="stat-value">${statistics.fraudAlertsGenerated}</div>
                        <div class="stat-change ${statistics.alertsChangePercent >= 0 ? 'positive' : 'negative'}">
                            ${statistics.alertsChangePercent >= 0 ? '+' : ''}${statistics.alertsChangePercent}% from last week
                        </div>
                    </div>

                    <div class="stat-card">
                        <h3>High Risk Calls</h3>
                        <div class="stat-value">${statistics.highRiskCalls}</div>
                        <div class="stat-subtext">Risk Score ≥ 7.0</div>
                    </div>

                    <div class="stat-card">
                        <h3>Storage Usage</h3>
                        <div class="stat-value">
                            ${this.formatBytes(statistics.memoryStorageUsed)} / ${this.formatBytes(statistics.maxMemoryStorage)}
                        </div>
                        <div class="stat-subtext">Memory (${statistics.memoryStoragePercent}% used)</div>
                    </div>
                </div>

                <!-- System Status -->
                <div class="system-status">
                    <h3>System Status</h3>
                    <div class="status-grid">
                        <div class="status-item ${systemStatus.voskServer ? 'healthy' : 'unhealthy'}">
                            <span class="status-indicator"></span>
                            Vosk ASR Server
                        </div>
                        <div class="status-item ${systemStatus.database ? 'healthy' : 'unhealthy'}">
                            <span class="status-indicator"></span>
                            Database Connection
                        </div>
                        <div class="status-item ${systemStatus.memoryStorage ? 'healthy' : 'unhealthy'}">
                            <span class="status-indicator"></span>
                            Memory Storage
                        </div>
                        <div class="status-item ${systemStatus.diskStorage ? 'healthy' : 'unhealthy'}">
                            <span class="status-indicator"></span>
                            Disk Storage
                        </div>
                    </div>
                </div>

                <!-- Recent Alerts -->
                <div class="recent-alerts">
                    <h3>Recent Alerts</h3>
                    <div class="alerts-list">
                        ${recentAlerts.map(alert => this.renderAlertCard(alert)).join('')}
                    </div>
                    ${recentAlerts.length === 0 ? '<p class="no-data">No recent alerts</p>' : ''}
                </div>
            </div>
        `;
    }

    renderAlertCard(alert) {
        const riskLevel = this.getRiskLevel(alert.riskScore);
        return `
            <div class="alert-card ${riskLevel}">
                <div class="alert-header">
                    <span class="alert-type">${alert.eventType}</span>
                    <span class="alert-time">${this.formatTime(alert.createdAt)}</span>
                </div>
                <div class="alert-content">
                    <div class="alert-call-id">Call ID: ${alert.callId}</div>
                    <div class="alert-risk-score">Risk Score: ${alert.riskScore.toFixed(1)}</div>
                    <div class="alert-trunk">Trunk: ${alert.ingressTrunkId}</div>
                </div>
                <div class="alert-actions">
                    <button class="btn btn-sm" onclick="antifraudUI.viewAlert('${alert.id}')">View Details</button>
                    <button class="btn btn-sm btn-primary" onclick="antifraudUI.acknowledgeAlert('${alert.id}')">Acknowledge</button>
                </div>
            </div>
        `;
    }

    getRiskLevel(riskScore) {
        if (riskScore >= 9.0) return 'critical';
        if (riskScore >= 7.0) return 'high';
        if (riskScore >= 5.0) return 'medium';
        return 'low';
    }

    showDashboard() {
        this.setActiveNav('nav-dashboard');
        this.loadDashboard();
    }

    async showRecordings() {
        this.setActiveNav('nav-recordings');
        try {
            const recordings = await this.fetchRecordings();
            this.renderRecordings(recordings);
        } catch (error) {
            console.error('Failed to load recordings:', error);
            this.showError('Failed to load recordings');
        }
    }

    async fetchRecordings() {
        const params = new URLSearchParams({
            page: this.currentPage,
            limit: this.pageSize,
            ...this.filters
        });

        const response = await fetch(`${this.apiBaseUrl}/recordings?${params}`);
        if (!response.ok) throw new Error('Failed to fetch recordings');
        return await response.json();
    }

    renderRecordings(recordings) {
        const content = document.getElementById('main-content');
        content.innerHTML = `
            <div class="recordings-page">
                <h2>Call Recordings</h2>

                <!-- Filters -->
                <div class="filters-panel">
                    <h3>Filters</h3>
                    <div class="filter-row">
                        <div class="filter-group">
                            <label>Date Range:</label>
                            <input type="date" id="filter-date-from" placeholder="From">
                            <input type="date" id="filter-date-to" placeholder="To">
                        </div>
                        <div class="filter-group">
                            <label>Trunk ID:</label>
                            <input type="number" id="filter-trunk-id" placeholder="Trunk ID">
                        </div>
                        <div class="filter-group">
                            <label>Storage Type:</label>
                            <select id="filter-storage-type">
                                <option value="">All</option>
                                <option value="memory">Memory</option>
                                <option value="disk">Disk</option>
                            </select>
                        </div>
                        <div class="filter-group">
                            <label>Legal Hold:</label>
                            <select id="filter-legal-hold">
                                <option value="">All</option>
                                <option value="true">Yes</option>
                                <option value="false">No</option>
                            </select>
                        </div>
                        <div class="filter-actions">
                            <button id="apply-filters" class="btn btn-primary">Apply Filters</button>
                            <button id="reset-filters" class="btn">Reset</button>
                        </div>
                    </div>
                </div>

                <!-- Recordings Table -->
                <div class="recordings-table">
                    <table>
                        <thead>
                            <tr>
                                <th>Call ID</th>
                                <th>Trunk</th>
                                <th>Duration</th>
                                <th>Storage</th>
                                <th>Recorded At</th>
                                <th>Risk Score</th>
                                <th>Legal Hold</th>
                                <th>Actions</th>
                            </tr>
                        </thead>
                        <tbody>
                            ${recordings.data.map(recording => this.renderRecordingRow(recording)).join('')}
                        </tbody>
                    </table>
                </div>

                <!-- Pagination -->
                <div class="pagination">
                    <button id="prev-page" class="btn" ${recordings.currentPage <= 1 ? 'disabled' : ''}>Previous</button>
                    <span class="page-info">Page ${recordings.currentPage} of ${recordings.totalPages}</span>
                    <button id="next-page" class="btn" ${recordings.currentPage >= recordings.totalPages ? 'disabled' : ''}>Next</button>
                </div>
            </div>
        `;
    }

    renderRecordingRow(recording) {
        const riskScore = recording.transcription ? recording.transcription.riskScore : 'N/A';
        const riskClass = recording.transcription ? this.getRiskLevel(recording.transcription.riskScore) : '';

        return `
            <tr>
                <td>${recording.callId}</td>
                <td>${recording.ingressTrunkId}</td>
                <td>${this.formatDuration(recording.durationSeconds)}</td>
                <td>
                    <span class="storage-type ${recording.storageType}">
                        ${recording.storageType === 'memory' ? 'Memory' : 'Disk'}
                    </span>
                </td>
                <td>${this.formatDateTime(recording.recordedAt)}</td>
                <td>
                    ${recording.transcription ?
                        `<span class="risk-score ${riskClass}">${recording.transcription.riskScore.toFixed(1)}</span>` :
                        'Pending'
                    }
                </td>
                <td>
                    <span class="legal-hold ${recording.legalHold ? 'active' : 'inactive'}">
                        ${recording.legalHold ? 'Yes' : 'No'}
                    </span>
                </td>
                <td>
                    <div class="action-buttons">
                        <button class="btn btn-sm" onclick="antifraudUI.viewRecording('${recording.id}')">View</button>
                        ${recording.transcription ?
                            `<button class="btn btn-sm" onclick="antifraudUI.viewTranscription('${recording.transcription.id}')">Transcript</button>` :
                            ''
                        }
                        ${this.currentUser.role === 'admin' ?
                            `<button class="btn btn-sm btn-warning" onclick="antifraudUI.toggleLegalHold('${recording.id}')">
                                ${recording.legalHold ? 'Release' : 'Hold'}
                            </button>` :
                            ''
                        }
                    </div>
                </td>
            </tr>
        `;
    }

    async viewRecording(recordingId) {
        try {
            const recording = await this.fetchRecordingDetails(recordingId);
            this.showRecordingModal(recording);
        } catch (error) {
            console.error('Failed to load recording details:', error);
            this.showError('Failed to load recording details');
        }
    }

    async viewTranscription(transcriptionId) {
        try {
            const transcription = await this.fetchTranscriptionDetails(transcriptionId);
            this.showTranscriptionModal(transcription);
        } catch (error) {
            console.error('Failed to load transcription details:', error);
            this.showError('Failed to load transcription details');
        }
    }

    showRecordingModal(recording) {
        const modal = document.createElement('div');
        modal.className = 'modal';
        modal.innerHTML = `
            <div class="modal-content">
                <div class="modal-header">
                    <h3>Recording Details</h3>
                    <button class="modal-close">&times;</button>
                </div>
                <div class="modal-body">
                    <div class="recording-details">
                        <div class="detail-group">
                            <h4>Call Information</h4>
                            <p><strong>Call ID:</strong> ${recording.callId}</p>
                            <p><strong>Session ID:</strong> ${recording.sessionId}</p>
                            <p><strong>Trunk ID:</strong> ${recording.ingressTrunkId}</p>
                            <p><strong>Duration:</strong> ${this.formatDuration(recording.durationSeconds)}</p>
                        </div>

                        <div class="detail-group">
                            <h4>Technical Details</h4>
                            <p><strong>Codec:</strong> ${recording.codec}</p>
                            <p><strong>Sample Rate:</strong> ${recording.sampleRate} Hz</p>
                            <p><strong>Channels:</strong> ${recording.channels}</p>
                            <p><strong>File Size:</strong> ${this.formatBytes(recording.fileSizeBytes)}</p>
                        </div>

                        <div class="detail-group">
                            <h4>Storage Information</h4>
                            <p><strong>Storage Type:</strong> ${recording.storageType}</p>
                            <p><strong>Recorded At:</strong> ${this.formatDateTime(recording.recordedAt)}</p>
                            <p><strong>Legal Hold:</strong> ${recording.legalHold ? 'Yes' : 'No'}</p>
                            ${recording.legalAuthorizationRef ?
                                `<p><strong>Legal Authorization:</strong> ${recording.legalAuthorizationRef}</p>` :
                                ''
                            }
                        </div>

                        ${recording.transcription ? `
                        <div class="detail-group">
                            <h4>Analysis Results</h4>
                            <p><strong>Risk Score:</strong>
                                <span class="risk-score ${this.getRiskLevel(recording.transcription.riskScore)}">
                                    ${recording.transcription.riskScore.toFixed(1)}
                                </span>
                            </p>
                            <p><strong>Banned Words Detected:</strong> ${recording.transcription.bannedWordsDetected}</p>
                            <p><strong>Requires Review:</strong> ${recording.transcription.requiresReview ? 'Yes' : 'No'}</p>
                        </div>
                        ` : ''}

                        <!-- ECPA Compliance Notice -->
                        <div class="compliance-warning">
                            <i class="icon-warning"></i>
                            <strong>ECPA Compliance:</strong> This recording is subject to ECPA regulations.
                            Access and use must comply with 18 U.S.C. § 2511 and applicable privacy laws.
                        </div>
                    </div>
                </div>
                <div class="modal-footer">
                    <button class="btn" onclick="this.closest('.modal').remove()">Close</button>
                    ${this.currentUser.role === 'admin' && recording.storageType === 'memory' ?
                        `<button class="btn btn-warning" onclick="antifraudUI.escalateToLegalHold('${recording.id}')">Escalate to Legal Hold</button>` :
                        ''
                    }
                </div>
            </div>
        `;

        document.body.appendChild(modal);
        modal.style.display = 'block';

        // Close modal handlers
        modal.querySelector('.modal-close').onclick = () => modal.remove();
        modal.onclick = (e) => {
            if (e.target === modal) modal.remove();
        };
    }

    // Utility functions
    formatBytes(bytes) {
        if (bytes === 0) return '0 B';
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
    }

    formatDuration(seconds) {
        const hours = Math.floor(seconds / 3600);
        const minutes = Math.floor((seconds % 3600) / 60);
        const secs = seconds % 60;

        if (hours > 0) {
            return `${hours}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
        }
        return `${minutes}:${secs.toString().padStart(2, '0')}`;
    }

    formatDateTime(dateString) {
        return new Date(dateString).toLocaleString();
    }

    formatTime(dateString) {
        return new Date(dateString).toLocaleTimeString();
    }

    setActiveNav(navId) {
        document.querySelectorAll('.nav-item').forEach(item => item.classList.remove('active'));
        document.getElementById(navId)?.classList.add('active');
    }

    showError(message) {
        const errorDiv = document.createElement('div');
        errorDiv.className = 'error-message';
        errorDiv.textContent = message;
        document.body.appendChild(errorDiv);

        setTimeout(() => errorDiv.remove(), 5000);
    }

    // Additional methods for other functionality
    async acknowledgeAlert(alertId) {
        try {
            const response = await fetch(`${this.apiBaseUrl}/events/${alertId}/acknowledge`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({
                    acknowledged_by: this.currentUser?.name || 'Unknown',
                    acknowledged_at: new Date().toISOString()
                })
            });

            if (response.ok) {
                this.showSuccess('Alert acknowledged successfully');
                this.loadDashboard(); // Refresh data
            } else {
                throw new Error('Failed to acknowledge alert');
            }
        } catch (error) {
            console.error('Error acknowledging alert:', error);
            this.showError('Failed to acknowledge alert');
        }
    }

    async toggleLegalHold(recordingId) {
        try {
            const response = await fetch(`${this.apiBaseUrl}/recordings/${recordingId}/legal-hold`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                }
            });

            if (response.ok) {
                this.showSuccess('Legal hold status updated');
                this.showRecordings(); // Refresh recordings view
            } else {
                throw new Error('Failed to toggle legal hold');
            }
        } catch (error) {
            console.error('Error toggling legal hold:', error);
            this.showError('Failed to update legal hold status');
        }
    }

    async escalateToLegalHold(recordingId) {
        if (!confirm('Are you sure you want to escalate this recording to legal hold? This will move it to permanent disk storage.')) {
            return;
        }

        try {
            const response = await fetch(`${this.apiBaseUrl}/recordings/${recordingId}/escalate`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({
                    reason: 'Manual escalation by ' + (this.currentUser?.name || 'Unknown'),
                    escalated_by: this.currentUser?.name || 'Unknown'
                })
            });

            if (response.ok) {
                this.showSuccess('Recording escalated to legal hold');
                // Close modal if open
                document.querySelector('.modal')?.remove();
                this.showRecordings(); // Refresh recordings view
            } else {
                throw new Error('Failed to escalate recording');
            }
        } catch (error) {
            console.error('Error escalating recording:', error);
            this.showError('Failed to escalate recording to legal hold');
        }
    }

    async fetchRecordingDetails(recordingId) {
        const response = await fetch(`${this.apiBaseUrl}/recordings/${recordingId}`);
        if (!response.ok) throw new Error('Failed to fetch recording details');
        return await response.json();
    }

    async fetchTranscriptionDetails(transcriptionId) {
        const response = await fetch(`${this.apiBaseUrl}/transcriptions/${transcriptionId}`);
        if (!response.ok) throw new Error('Failed to fetch transcription details');
        return await response.json();
    }

    showSuccess(message) {
        const successDiv = document.createElement('div');
        successDiv.className = 'success-message';
        successDiv.textContent = message;
        successDiv.style.cssText = `
            position: fixed;
            top: 20px;
            right: 20px;
            background: #4CAF50;
            color: white;
            padding: 12px 24px;
            border-radius: 4px;
            z-index: 10000;
            box-shadow: 0 2px 10px rgba(0,0,0,0.2);
        `;
        document.body.appendChild(successDiv);

        setTimeout(() => successDiv.remove(), 3000);
    }

    // Handle cases where data might be missing or malformed
    renderDashboardSafe(statistics, recentAlerts, systemStatus) {
        // Provide safe defaults for all data
        const safeStats = {
            totalCallsMonitored: statistics?.total_calls_monitored || 0,
            fraudAlertsGenerated: statistics?.fraud_alerts || 0,
            highRiskCalls: statistics?.high_risk_calls || 0,
            memoryStorageUsed: statistics?.memory_storage_used || 0,
            maxMemoryStorage: statistics?.max_memory_storage || 1,
            memoryStoragePercent: Math.round(((statistics?.memory_storage_used || 0) / (statistics?.max_memory_storage || 1)) * 100),
            callsChangePercent: statistics?.week_over_week_change || 0,
            alertsChangePercent: statistics?.week_over_week_change || 0
        };

        const safeStatus = {
            voskServer: systemStatus?.vosk_server !== false,
            database: systemStatus?.database !== false,
            memoryStorage: systemStatus?.storage !== false,
            diskStorage: systemStatus?.storage !== false
        };

        const safeAlerts = Array.isArray(recentAlerts) ? recentAlerts : [];

        this.renderDashboard(safeStats, safeAlerts, safeStatus);
    }
}

// Initialize the UI when the page loads
let antifraudUI;
document.addEventListener('DOMContentLoaded', () => {
    antifraudUI = new AntiFraudMonitoringUI();
});