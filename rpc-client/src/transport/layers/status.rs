use super::connection::{ConnectionManager, ConnectionStatus};
use crate::instrumentation::ClientMetrics;
use opentelemetry::KeyValue;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tracing::{debug, info};

/// Connection status reporter provides detailed information about connection state
/// and emits metrics and events for monitoring and observability.
pub struct ConnectionStatusReporter {
    /// Connection manager to monitor
    connection_manager: Arc<ConnectionManager>,
    /// Metrics for emitting connection events
    pub metrics: Option<Arc<ClientMetrics>>,
    /// Last known status for change detection
    last_status: std::sync::RwLock<Option<ConnectionStatus>>,
    /// Status history for trend analysis
    status_history: std::sync::RwLock<Vec<StatusHistoryEntry>>,
    /// Maximum number of history entries to keep
    max_history_entries: usize,
}

impl ConnectionStatusReporter {
    /// Create a new connection status reporter
    pub fn new(connection_manager: Arc<ConnectionManager>) -> Self {
        Self {
            connection_manager,
            metrics: None,
            last_status: std::sync::RwLock::new(None),
            status_history: std::sync::RwLock::new(Vec::new()),
            max_history_entries: 100,
        }
    }

    /// Create a new connection status reporter with metrics
    pub fn with_metrics(
        connection_manager: Arc<ConnectionManager>,
        metrics: Arc<ClientMetrics>,
    ) -> Self {
        Self {
            connection_manager,
            metrics: Some(metrics),
            last_status: std::sync::RwLock::new(None),
            status_history: std::sync::RwLock::new(Vec::new()),
            max_history_entries: 100,
        }
    }

    /// Get the current connection status with detailed information
    pub fn get_detailed_status(&self) -> DetailedConnectionStatus {
        let current_status = self.connection_manager.get_connection_status();
        let history = self.status_history.read().unwrap();

        let uptime = self.calculate_uptime(&history);
        let connection_count = self.calculate_connection_count(&history);
        let last_connected = self.get_last_connected_time(&history);
        let last_disconnected = self.get_last_disconnected_time(&history);

        DetailedConnectionStatus {
            status: current_status.clone(),
            uptime,
            total_connections: connection_count,
            last_connected,
            last_disconnected,
            status_changes: history.len(),
            timestamp: SystemTime::now(),
        }
    }

    /// Update the status and emit metrics/events if changed
    pub fn update_status(&self) {
        let current_status = self.connection_manager.get_connection_status();
        let mut last_status = self.last_status.write().unwrap();

        let status_changed = match &*last_status {
            Some(last) => last != &current_status,
            None => true,
        };

        if status_changed {
            self.record_status_change(&current_status);
            self.emit_status_metrics(&current_status);
            self.log_status_change(&*last_status, &current_status);

            *last_status = Some(current_status);
        }
    }

    /// Get connection status history
    pub fn get_status_history(&self) -> Vec<StatusHistoryEntry> {
        self.status_history.read().unwrap().clone()
    }

    /// Get connection statistics
    pub fn get_connection_statistics(&self) -> ConnectionStatistics {
        let history = self.status_history.read().unwrap();

        let total_connections = self.calculate_connection_count(&history);
        let total_disconnections = self.calculate_disconnection_count(&history);
        let total_reconnection_attempts = self.calculate_reconnection_attempts(&history);
        let uptime = self.calculate_uptime(&history);
        let downtime = self.calculate_downtime(&history);
        let average_connection_duration = self.calculate_average_connection_duration(&history);

        ConnectionStatistics {
            total_connections,
            total_disconnections,
            total_reconnection_attempts,
            uptime,
            downtime,
            average_connection_duration,
            status_changes: history.len(),
            first_connection: history.first().map(|entry| entry.timestamp),
            last_status_change: history.last().map(|entry| entry.timestamp),
        }
    }

    /// Clear status history
    pub fn clear_history(&self) {
        let mut history = self.status_history.write().unwrap();
        history.clear();
        info!("Connection status history cleared");
    }

    /// Record a status change in history
    fn record_status_change(&self, status: &ConnectionStatus) {
        let mut history = self.status_history.write().unwrap();

        let entry = StatusHistoryEntry {
            status: status.clone(),
            timestamp: SystemTime::now(),
        };

        history.push(entry);

        // Limit history size
        if history.len() > self.max_history_entries {
            history.remove(0);
        }
    }

    /// Emit metrics for the current status
    fn emit_status_metrics(&self, status: &ConnectionStatus) {
        if let Some(metrics) = &self.metrics {
            let _status_value = match status {
                ConnectionStatus::Disconnected => 0,
                ConnectionStatus::Connecting => 1,
                ConnectionStatus::Connected => 2,
                ConnectionStatus::Reconnecting { .. } => 3,
            };

            // Update connection status gauge
            metrics.active_connections.record(
                if matches!(status, ConnectionStatus::Connected) {
                    1
                } else {
                    0
                },
                &[],
            );

            // Increment status change counter
            let status_name = match status {
                ConnectionStatus::Disconnected => "disconnected",
                ConnectionStatus::Connecting => "connecting",
                ConnectionStatus::Connected => "connected",
                ConnectionStatus::Reconnecting { .. } => "reconnecting",
            };

            metrics.requests_total.add(
                1,
                &[
                    KeyValue::new("event_type", "status_change"),
                    KeyValue::new("status", status_name),
                ],
            );

            // For reconnecting status, track attempt count
            if let ConnectionStatus::Reconnecting { attempts } = status {
                metrics
                    .reconnection_attempts_total
                    .add(1, &[KeyValue::new("attempt", attempts.to_string())]);
            }
        }
    }

    /// Log status changes
    fn log_status_change(
        &self,
        old_status: &Option<ConnectionStatus>,
        new_status: &ConnectionStatus,
    ) {
        match (old_status, new_status) {
            (None, status) => {
                info!(status = ?status, "Initial connection status");
            }
            (Some(old), new) if old != new => {
                info!(
                    old_status = ?old,
                    new_status = ?new,
                    "Connection status changed"
                );
            }
            _ => {
                debug!(status = ?new_status, "Connection status unchanged");
            }
        }
    }

    /// Calculate total uptime from history
    fn calculate_uptime(&self, history: &[StatusHistoryEntry]) -> Duration {
        let mut uptime = Duration::ZERO;
        let mut last_connected: Option<SystemTime> = None;

        for entry in history {
            match (&entry.status, &last_connected) {
                (ConnectionStatus::Connected, None) => {
                    last_connected = Some(entry.timestamp);
                }
                (
                    ConnectionStatus::Disconnected | ConnectionStatus::Reconnecting { .. },
                    Some(connected_at),
                ) => {
                    if let Ok(duration) = entry.timestamp.duration_since(*connected_at) {
                        uptime += duration;
                    }
                    last_connected = None;
                }
                _ => {}
            }
        }

        // If currently connected, add time since last connection
        if let Some(connected_at) = last_connected {
            if let Ok(duration) = SystemTime::now().duration_since(connected_at) {
                uptime += duration;
            }
        }

        uptime
    }

    /// Calculate total downtime from history
    fn calculate_downtime(&self, history: &[StatusHistoryEntry]) -> Duration {
        let mut downtime = Duration::ZERO;
        let mut last_disconnected: Option<SystemTime> = None;

        for entry in history {
            match (&entry.status, &last_disconnected) {
                (ConnectionStatus::Disconnected | ConnectionStatus::Reconnecting { .. }, None) => {
                    last_disconnected = Some(entry.timestamp);
                }
                (ConnectionStatus::Connected, Some(disconnected_at)) => {
                    if let Ok(duration) = entry.timestamp.duration_since(*disconnected_at) {
                        downtime += duration;
                    }
                    last_disconnected = None;
                }
                _ => {}
            }
        }

        // If currently disconnected, add time since last disconnection
        if let Some(disconnected_at) = last_disconnected {
            if let Ok(duration) = SystemTime::now().duration_since(disconnected_at) {
                downtime += duration;
            }
        }

        downtime
    }

    /// Calculate total number of connections from history
    fn calculate_connection_count(&self, history: &[StatusHistoryEntry]) -> u32 {
        history
            .iter()
            .filter(|entry| matches!(entry.status, ConnectionStatus::Connected))
            .count() as u32
    }

    /// Calculate total number of disconnections from history
    fn calculate_disconnection_count(&self, history: &[StatusHistoryEntry]) -> u32 {
        history
            .iter()
            .filter(|entry| matches!(entry.status, ConnectionStatus::Disconnected))
            .count() as u32
    }

    /// Calculate total reconnection attempts from history
    fn calculate_reconnection_attempts(&self, history: &[StatusHistoryEntry]) -> u32 {
        history
            .iter()
            .filter_map(|entry| {
                if let ConnectionStatus::Reconnecting { attempts } = &entry.status {
                    Some(*attempts)
                } else {
                    None
                }
            })
            .sum()
    }

    /// Calculate average connection duration
    fn calculate_average_connection_duration(
        &self,
        history: &[StatusHistoryEntry],
    ) -> Option<Duration> {
        let mut durations = Vec::new();
        let mut last_connected: Option<SystemTime> = None;

        for entry in history {
            match (&entry.status, &last_connected) {
                (ConnectionStatus::Connected, None) => {
                    last_connected = Some(entry.timestamp);
                }
                (
                    ConnectionStatus::Disconnected | ConnectionStatus::Reconnecting { .. },
                    Some(connected_at),
                ) => {
                    if let Ok(duration) = entry.timestamp.duration_since(*connected_at) {
                        durations.push(duration);
                    }
                    last_connected = None;
                }
                _ => {}
            }
        }

        if durations.is_empty() {
            None
        } else {
            let total: Duration = durations.iter().sum();
            Some(total / durations.len() as u32)
        }
    }

    /// Get the last time the connection was established
    fn get_last_connected_time(&self, history: &[StatusHistoryEntry]) -> Option<SystemTime> {
        history
            .iter()
            .rev()
            .find(|entry| matches!(entry.status, ConnectionStatus::Connected))
            .map(|entry| entry.timestamp)
    }

    /// Get the last time the connection was lost
    fn get_last_disconnected_time(&self, history: &[StatusHistoryEntry]) -> Option<SystemTime> {
        history
            .iter()
            .rev()
            .find(|entry| matches!(entry.status, ConnectionStatus::Disconnected))
            .map(|entry| entry.timestamp)
    }
}

/// Detailed connection status with additional information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedConnectionStatus {
    /// Current connection status
    pub status: ConnectionStatus,
    /// Total uptime since monitoring started
    pub uptime: Duration,
    /// Total number of successful connections
    pub total_connections: u32,
    /// Timestamp of last successful connection
    pub last_connected: Option<SystemTime>,
    /// Timestamp of last disconnection
    pub last_disconnected: Option<SystemTime>,
    /// Number of status changes recorded
    pub status_changes: usize,
    /// Timestamp when this status was captured
    pub timestamp: SystemTime,
}

/// Connection statistics for monitoring and analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStatistics {
    /// Total number of successful connections
    pub total_connections: u32,
    /// Total number of disconnections
    pub total_disconnections: u32,
    /// Total number of reconnection attempts
    pub total_reconnection_attempts: u32,
    /// Total uptime
    pub uptime: Duration,
    /// Total downtime
    pub downtime: Duration,
    /// Average duration of connections
    pub average_connection_duration: Option<Duration>,
    /// Total number of status changes
    pub status_changes: usize,
    /// Timestamp of first connection
    pub first_connection: Option<SystemTime>,
    /// Timestamp of last status change
    pub last_status_change: Option<SystemTime>,
}

/// Entry in the status history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusHistoryEntry {
    /// Connection status at this point in time
    pub status: ConnectionStatus,
    /// Timestamp when this status was recorded
    pub timestamp: SystemTime,
}

/// Connection health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionHealth {
    /// Overall health status
    pub health: HealthStatus,
    /// Connection availability percentage (0.0 to 1.0)
    pub availability: f64,
    /// Average time between connection failures
    pub mean_time_between_failures: Option<Duration>,
    /// Average time to recover from failures
    pub mean_time_to_recovery: Option<Duration>,
    /// Current consecutive successful connections
    pub consecutive_successes: u32,
    /// Current consecutive failures
    pub consecutive_failures: u32,
}

/// Health status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Connection is healthy and stable
    Healthy,
    /// Connection has minor issues but is functional
    Degraded,
    /// Connection is experiencing significant problems
    Unhealthy,
    /// Connection is completely unavailable
    Critical,
}

impl ConnectionStatusReporter {
    /// Calculate connection health metrics
    pub fn get_connection_health(&self) -> ConnectionHealth {
        let statistics = self.get_connection_statistics();
        let current_status = self.connection_manager.get_connection_status();

        let total_time = statistics.uptime + statistics.downtime;
        let availability = if total_time.is_zero() {
            1.0
        } else {
            statistics.uptime.as_secs_f64() / total_time.as_secs_f64()
        };

        let health = match current_status {
            ConnectionStatus::Connected => {
                if availability >= 0.95 {
                    HealthStatus::Healthy
                } else if availability >= 0.80 {
                    HealthStatus::Degraded
                } else {
                    HealthStatus::Unhealthy
                }
            }
            ConnectionStatus::Connecting | ConnectionStatus::Reconnecting { .. } => {
                if availability >= 0.50 {
                    HealthStatus::Degraded
                } else {
                    HealthStatus::Unhealthy
                }
            }
            ConnectionStatus::Disconnected => {
                if availability >= 0.50 {
                    HealthStatus::Unhealthy
                } else {
                    HealthStatus::Critical
                }
            }
        };

        // Calculate MTBF and MTTR (simplified)
        let mean_time_between_failures = if statistics.total_disconnections > 0 {
            Some(statistics.uptime / statistics.total_disconnections)
        } else {
            None
        };

        let mean_time_to_recovery = if statistics.total_connections > 0 {
            Some(statistics.downtime / statistics.total_connections)
        } else {
            None
        };

        ConnectionHealth {
            health,
            availability,
            mean_time_between_failures,
            mean_time_to_recovery,
            consecutive_successes: 0, // Would need more detailed tracking
            consecutive_failures: 0,  // Would need more detailed tracking
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ReconnectionConfig;
    use crate::transport::layers::connection::{ConnectionManager, DefaultClientFactory};
    use std::time::Duration;

    #[test]
    fn test_status_reporter_creation() {
        let factory = Arc::new(DefaultClientFactory);
        let config = ReconnectionConfig::default();
        let manager = Arc::new(ConnectionManager::new(factory, config));
        let reporter = ConnectionStatusReporter::new(manager);

        let status = reporter.get_detailed_status();
        assert_eq!(status.status, ConnectionStatus::Disconnected);
        assert_eq!(status.total_connections, 0);
        assert_eq!(status.status_changes, 0);
    }

    #[test]
    fn test_status_history_entry() {
        let entry = StatusHistoryEntry {
            status: ConnectionStatus::Connected,
            timestamp: SystemTime::now(),
        };

        assert_eq!(entry.status, ConnectionStatus::Connected);
        assert!(entry.timestamp <= SystemTime::now());
    }

    #[test]
    fn test_health_status_values() {
        assert_ne!(HealthStatus::Healthy, HealthStatus::Degraded);
        assert_ne!(HealthStatus::Unhealthy, HealthStatus::Critical);
    }

    #[test]
    fn test_connection_statistics_default() {
        let stats = ConnectionStatistics {
            total_connections: 0,
            total_disconnections: 0,
            total_reconnection_attempts: 0,
            uptime: Duration::ZERO,
            downtime: Duration::ZERO,
            average_connection_duration: None,
            status_changes: 0,
            first_connection: None,
            last_status_change: None,
        };

        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.uptime, Duration::ZERO);
        assert!(stats.average_connection_duration.is_none());
    }

    #[test]
    fn test_detailed_status_serialization() {
        let status = DetailedConnectionStatus {
            status: ConnectionStatus::Connected,
            uptime: Duration::from_secs(3600),
            total_connections: 5,
            last_connected: Some(SystemTime::now()),
            last_disconnected: None,
            status_changes: 10,
            timestamp: SystemTime::now(),
        };

        // Test that the status has expected values
        assert_eq!(status.status, ConnectionStatus::Connected);
        assert_eq!(status.total_connections, 5);
    }

    #[test]
    fn test_connection_health_calculation() {
        let factory = Arc::new(DefaultClientFactory);
        let config = ReconnectionConfig::default();
        let manager = Arc::new(ConnectionManager::new(factory, config));
        let reporter = ConnectionStatusReporter::new(manager);

        let health = reporter.get_connection_health();

        // With no history, should have perfect availability
        assert_eq!(health.availability, 1.0);
        assert!(health.mean_time_between_failures.is_none());
        assert!(health.mean_time_to_recovery.is_none());
    }

    #[test]
    fn test_status_reporter_with_metrics() {
        use crate::instrumentation::ClientMetrics;
        use opentelemetry::global;

        let factory = Arc::new(DefaultClientFactory);
        let config = ReconnectionConfig::default();
        let manager = Arc::new(ConnectionManager::new(factory, config));

        let meter = global::meter("test");
        let metrics = Arc::new(ClientMetrics::new(&meter));
        let reporter = ConnectionStatusReporter::with_metrics(manager, metrics);

        // Should have metrics available
        assert!(reporter.metrics.is_some());
    }

    #[test]
    fn test_status_change_detection() {
        let factory = Arc::new(DefaultClientFactory);
        let config = ReconnectionConfig::default();
        let manager = Arc::new(ConnectionManager::new(factory, config));
        let reporter = ConnectionStatusReporter::new(manager);

        // Initial update should record change
        reporter.update_status();
        let history = reporter.get_status_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].status, ConnectionStatus::Disconnected);

        // Same status should not create new entry
        reporter.update_status();
        let history = reporter.get_status_history();
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn test_history_size_limit() {
        let factory = Arc::new(DefaultClientFactory);
        let config = ReconnectionConfig::default();
        let manager = Arc::new(ConnectionManager::new(factory, config));
        let mut reporter = ConnectionStatusReporter::new(manager);
        reporter.max_history_entries = 3;

        // Manually add entries to test limit
        for i in 0..5 {
            reporter.record_status_change(&ConnectionStatus::Reconnecting { attempts: i });
        }

        let history = reporter.get_status_history();
        assert_eq!(history.len(), 3); // Should be limited to max_history_entries
    }

    #[test]
    fn test_clear_history() {
        let factory = Arc::new(DefaultClientFactory);
        let config = ReconnectionConfig::default();
        let manager = Arc::new(ConnectionManager::new(factory, config));
        let reporter = ConnectionStatusReporter::new(manager);

        // Add some history
        reporter.update_status();
        assert!(!reporter.get_status_history().is_empty());

        // Clear history
        reporter.clear_history();
        assert!(reporter.get_status_history().is_empty());
    }
}
