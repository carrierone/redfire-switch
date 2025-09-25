import React, { useState, useEffect } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Progress } from '@/components/ui/progress';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import {
  HardDrive,
  AlertTriangle,
  AlertCircle,
  TrendingUp,
  Database,
  FileAudio,
  FileText,
  Trash2
} from 'lucide-react';

interface DiskUsage {
  mount_point: string;
  filesystem: string;
  total_bytes: number;
  used_bytes: number;
  available_bytes: number;
  usage_percentage: number;
  inodes_total: number;
  inodes_used: number;
  inodes_available: number;
  inode_usage_percentage: number;
  last_updated: string;
}

interface StorageCategoryUsage {
  category: string;
  total_bytes: number;
  used_bytes: number;
  file_count: number;
  paths: string[];
  last_updated: string;
}

interface DiskAlert {
  alert_type: 'Warning' | 'Critical' | 'InodesWarning' | 'InodesCritical';
  mount_point: string;
  usage_percentage: number;
  available_bytes: number;
  message: string;
  timestamp: string;
}

interface DiskStatistics {
  timestamp: string;
  mount_points: DiskUsage[];
  recording_storage: StorageCategoryUsage;
  database_storage: StorageCategoryUsage;
  log_storage: StorageCategoryUsage;
  total_system_usage: {
    total_capacity: number;
    total_used: number;
    total_available: number;
    overall_usage_percentage: number;
    critical_mount_points: string[];
    warning_mount_points: string[];
  };
  alerts: DiskAlert[];
}

const formatBytes = (bytes: number): string => {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let size = bytes;
  let unitIndex = 0;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex++;
  }

  return `${size.toFixed(1)} ${units[unitIndex]}`;
};

const getUsageColor = (percentage: number): string => {
  if (percentage >= 90) return 'text-red-600';
  if (percentage >= 80) return 'text-yellow-600';
  return 'text-green-600';
};

const getProgressColor = (percentage: number): string => {
  if (percentage >= 90) return 'bg-red-500';
  if (percentage >= 80) return 'bg-yellow-500';
  return 'bg-green-500';
};

const DiskSpaceMonitor: React.FC = () => {
  const [diskStats, setDiskStats] = useState<DiskStatistics | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [cleanupRecommendations, setCleanupRecommendations] = useState<string[]>([]);

  useEffect(() => {
    fetchDiskStatistics();
    const interval = setInterval(fetchDiskStatistics, 60000); // Update every minute
    return () => clearInterval(interval);
  }, []);

  const fetchDiskStatistics = async () => {
    try {
      const response = await fetch('/api/voice-integrity/disk-statistics');
      if (!response.ok) {
        throw new Error('Failed to fetch disk statistics');
      }
      const data = await response.json();
      setDiskStats(data);

      // Fetch cleanup recommendations
      const recommendationsResponse = await fetch('/api/voice-integrity/cleanup-recommendations');
      if (recommendationsResponse.ok) {
        const recommendations = await recommendationsResponse.json();
        setCleanupRecommendations(recommendations);
      }

      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setLoading(false);
    }
  };

  const handleForceCheck = async () => {
    setLoading(true);
    try {
      const response = await fetch('/api/voice-integrity/disk-check', {
        method: 'POST'
      });
      if (!response.ok) {
        throw new Error('Failed to trigger disk check');
      }
      await fetchDiskStatistics();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to trigger disk check');
    } finally {
      setLoading(false);
    }
  };

  if (loading && !diskStats) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <HardDrive className="h-5 w-5" />
            Disk Space Monitoring
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-center justify-center p-4">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500" />
          </div>
        </CardContent>
      </Card>
    );
  }

  if (error) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-red-600">
            <AlertCircle className="h-5 w-5" />
            Disk Space Monitoring Error
          </CardTitle>
        </CardHeader>
        <CardContent>
          <Alert variant="destructive">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        </CardContent>
      </Card>
    );
  }

  if (!diskStats) return null;

  return (
    <div className="space-y-6">
      {/* System Overview */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <HardDrive className="h-5 w-5" />
            System Storage Overview
            <button
              onClick={handleForceCheck}
              disabled={loading}
              className="ml-auto px-3 py-1 text-sm bg-blue-500 text-white rounded hover:bg-blue-600 disabled:opacity-50"
            >
              {loading ? 'Checking...' : 'Force Check'}
            </button>
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-4">
            <div className="text-center">
              <div className="text-2xl font-bold text-blue-600">
                {formatBytes(diskStats.total_system_usage.total_capacity)}
              </div>
              <div className="text-sm text-gray-500">Total Capacity</div>
            </div>
            <div className="text-center">
              <div className="text-2xl font-bold text-orange-600">
                {formatBytes(diskStats.total_system_usage.total_used)}
              </div>
              <div className="text-sm text-gray-500">Used Space</div>
            </div>
            <div className="text-center">
              <div className="text-2xl font-bold text-green-600">
                {formatBytes(diskStats.total_system_usage.total_available)}
              </div>
              <div className="text-sm text-gray-500">Available Space</div>
            </div>
          </div>

          <div className="mb-4">
            <div className="flex justify-between items-center mb-2">
              <span className="text-sm font-medium">Overall Usage</span>
              <span className={`text-sm font-bold ${getUsageColor(diskStats.total_system_usage.overall_usage_percentage)}`}>
                {diskStats.total_system_usage.overall_usage_percentage.toFixed(1)}%
              </span>
            </div>
            <Progress
              value={diskStats.total_system_usage.overall_usage_percentage}
              className="h-3"
            />
          </div>

          {diskStats.alerts.length > 0 && (
            <div className="space-y-2">
              <h4 className="font-medium text-red-600 flex items-center gap-2">
                <AlertTriangle className="h-4 w-4" />
                Active Alerts
              </h4>
              {diskStats.alerts.map((alert, index) => (
                <Alert key={index} variant={alert.alert_type.includes('Critical') ? 'destructive' : 'default'}>
                  <AlertDescription>
                    <Badge variant={alert.alert_type.includes('Critical') ? 'destructive' : 'secondary'} className="mr-2">
                      {alert.alert_type}
                    </Badge>
                    {alert.message}
                  </AlertDescription>
                </Alert>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      {/* Mount Points */}
      <Card>
        <CardHeader>
          <CardTitle>Mount Points</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            {diskStats.mount_points.map((mount, index) => (
              <div key={index} className="border rounded-lg p-4">
                <div className="flex justify-between items-center mb-2">
                  <div>
                    <span className="font-medium">{mount.mount_point}</span>
                    <span className="text-sm text-gray-500 ml-2">({mount.filesystem})</span>
                  </div>
                  <div className="text-right">
                    <div className={`font-bold ${getUsageColor(mount.usage_percentage)}`}>
                      {mount.usage_percentage.toFixed(1)}%
                    </div>
                    <div className="text-sm text-gray-500">
                      {formatBytes(mount.available_bytes)} free
                    </div>
                  </div>
                </div>

                <Progress
                  value={mount.usage_percentage}
                  className="h-2 mb-2"
                />

                <div className="grid grid-cols-2 gap-4 text-sm">
                  <div>
                    <div className="text-gray-500">Disk Usage:</div>
                    <div>{formatBytes(mount.used_bytes)} / {formatBytes(mount.total_bytes)}</div>
                  </div>
                  <div>
                    <div className="text-gray-500">Inode Usage:</div>
                    <div className={mount.inode_usage_percentage > 80 ? 'text-yellow-600' : ''}>
                      {mount.inode_usage_percentage.toFixed(1)}% ({mount.inodes_used.toLocaleString()} / {mount.inodes_total.toLocaleString()})
                    </div>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Storage Categories */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        {/* Voice Recordings */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <FileAudio className="h-5 w-5 text-blue-500" />
              Voice Recordings
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              <div className="flex justify-between">
                <span className="text-sm text-gray-500">Used:</span>
                <span className="font-medium">{formatBytes(diskStats.recording_storage.used_bytes)}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-sm text-gray-500">Files:</span>
                <span className="font-medium">{diskStats.recording_storage.file_count.toLocaleString()}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-sm text-gray-500">Usage:</span>
                <span className={`font-medium ${getUsageColor(
                  (diskStats.recording_storage.used_bytes / diskStats.recording_storage.total_bytes) * 100
                )}`}>
                  {((diskStats.recording_storage.used_bytes / diskStats.recording_storage.total_bytes) * 100).toFixed(1)}%
                </span>
              </div>
              <Progress
                value={(diskStats.recording_storage.used_bytes / diskStats.recording_storage.total_bytes) * 100}
                className="h-2"
              />
              <div className="text-xs text-gray-400">
                Paths: {diskStats.recording_storage.paths.join(', ')}
              </div>
            </div>
          </CardContent>
        </Card>

        {/* Database Storage */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Database className="h-5 w-5 text-green-500" />
              Database
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              <div className="flex justify-between">
                <span className="text-sm text-gray-500">Used:</span>
                <span className="font-medium">{formatBytes(diskStats.database_storage.used_bytes)}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-sm text-gray-500">Files:</span>
                <span className="font-medium">{diskStats.database_storage.file_count.toLocaleString()}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-sm text-gray-500">Usage:</span>
                <span className={`font-medium ${getUsageColor(
                  (diskStats.database_storage.used_bytes / diskStats.database_storage.total_bytes) * 100
                )}`}>
                  {((diskStats.database_storage.used_bytes / diskStats.database_storage.total_bytes) * 100).toFixed(1)}%
                </span>
              </div>
              <Progress
                value={(diskStats.database_storage.used_bytes / diskStats.database_storage.total_bytes) * 100}
                className="h-2"
              />
              <div className="text-xs text-gray-400">
                Paths: {diskStats.database_storage.paths.join(', ')}
              </div>
            </div>
          </CardContent>
        </Card>

        {/* Log Storage */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <FileText className="h-5 w-5 text-purple-500" />
              Logs
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              <div className="flex justify-between">
                <span className="text-sm text-gray-500">Used:</span>
                <span className="font-medium">{formatBytes(diskStats.log_storage.used_bytes)}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-sm text-gray-500">Files:</span>
                <span className="font-medium">{diskStats.log_storage.file_count.toLocaleString()}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-sm text-gray-500">Usage:</span>
                <span className={`font-medium ${getUsageColor(
                  (diskStats.log_storage.used_bytes / diskStats.log_storage.total_bytes) * 100
                )}`}>
                  {((diskStats.log_storage.used_bytes / diskStats.log_storage.total_bytes) * 100).toFixed(1)}%
                </span>
              </div>
              <Progress
                value={(diskStats.log_storage.used_bytes / diskStats.log_storage.total_bytes) * 100}
                className="h-2"
              />
              <div className="text-xs text-gray-400">
                Paths: {diskStats.log_storage.paths.join(', ')}
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Cleanup Recommendations */}
      {cleanupRecommendations.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Trash2 className="h-5 w-5 text-orange-500" />
              Cleanup Recommendations
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-2">
              {cleanupRecommendations.map((recommendation, index) => (
                <Alert key={index}>
                  <TrendingUp className="h-4 w-4" />
                  <AlertDescription>{recommendation}</AlertDescription>
                </Alert>
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Last Updated */}
      <div className="text-center text-sm text-gray-500">
        Last updated: {new Date(diskStats.timestamp).toLocaleString()}
      </div>
    </div>
  );
};

export default DiskSpaceMonitor;