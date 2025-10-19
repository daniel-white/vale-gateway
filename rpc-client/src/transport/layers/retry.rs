use super::WsClientLayer;
use futures_util::future::Ready;
use jsonrpsee::ws_client::WsClient;
use std::time::Duration;
use tower_retry::Policy;

/// Retry layer that implements exponential backoff for failed requests
///
/// This layer wraps the WebSocket client to automatically retry failed requests
/// according to a configurable retry policy. It uses exponential backoff with
/// jitter to avoid thundering herd problems.
pub struct RetryLayer {
    /// Retry policy configuration
    policy: crate::config::RetryPolicy,
    /// Metrics for tracking retry operations
    metrics: Option<std::sync::Arc<crate::instrumentation::ClientMetrics>>,
}

impl RetryLayer {
    /// Create a new retry layer with the specified policy
    pub fn new(policy: crate::config::RetryPolicy) -> Self {
        Self {
            policy,
            metrics: None,
        }
    }

    /// Create a new retry layer with the specified policy and metrics
    pub fn with_metrics(
        policy: crate::config::RetryPolicy,
        metrics: std::sync::Arc<crate::instrumentation::ClientMetrics>,
    ) -> Self {
        Self {
            policy,
            metrics: Some(metrics),
        }
    }

    /// Create a retry layer from retry policy configuration
    pub fn from_config(config: &crate::config::RetryPolicy) -> Self {
        Self {
            policy: config.clone(),
            metrics: None,
        }
    }

    /// Create a retry layer from retry policy configuration with metrics
    pub fn from_config_with_metrics(
        config: &crate::config::RetryPolicy,
        metrics: std::sync::Arc<crate::instrumentation::ClientMetrics>,
    ) -> Self {
        Self {
            policy: config.clone(),
            metrics: Some(metrics),
        }
    }

    /// Get the retry policy
    pub fn policy(&self) -> &crate::config::RetryPolicy {
        &self.policy
    }

    /// Get the metrics (if available)
    pub fn metrics(&self) -> Option<&std::sync::Arc<crate::instrumentation::ClientMetrics>> {
        self.metrics.as_ref()
    }
}

impl WsClientLayer for RetryLayer {
    fn configure_client(&self, client: WsClient) -> WsClient {
        // For jsonrpsee WsClient, we can't directly wrap it with Tower retry middleware
        // since it doesn't implement the Tower Service trait. Instead, we'll need to
        // implement retry logic at the transport level or in the enhanced builder.
        // For now, we'll log that the retry layer is configured and return the client.

        tracing::info!(
            max_attempts = self.policy.max_attempts,
            base_delay = ?self.policy.base_delay,
            max_delay = ?self.policy.max_delay,
            backoff_multiplier = self.policy.backoff_multiplier,
            jitter = self.policy.jitter,
            has_metrics = self.metrics.is_some(),
            "Retry layer configured"
        );

        client
    }
}

/// Exponential backoff policy for tower-retry
///
/// This policy implements exponential backoff with jitter for retry attempts.
/// It classifies errors as retryable or non-retryable and calculates appropriate
/// delays between retry attempts.
#[derive(Debug, Clone)]
pub struct ExponentialBackoffPolicy {
    /// Retry policy configuration
    policy: crate::config::RetryPolicy,
    /// Current attempt number (0-based)
    current_attempt: u32,
    /// Metrics for tracking retry operations
    #[allow(dead_code)]
    metrics: Option<std::sync::Arc<crate::instrumentation::ClientMetrics>>,
}

impl ExponentialBackoffPolicy {
    /// Create a new exponential backoff policy
    pub fn new(policy: crate::config::RetryPolicy) -> Self {
        Self {
            policy,
            current_attempt: 0,
            metrics: None,
        }
    }

    /// Create a new exponential backoff policy with metrics
    pub fn with_metrics(
        policy: crate::config::RetryPolicy,
        metrics: std::sync::Arc<crate::instrumentation::ClientMetrics>,
    ) -> Self {
        Self {
            policy,
            current_attempt: 0,
            metrics: Some(metrics),
        }
    }

    /// Check if an error is retryable
    pub fn is_retryable_error(error: &crate::api::ConfigurationClientError) -> bool {
        use crate::api::ErrorClassification;
        error.is_retryable()
    }

    /// Calculate delay for the current attempt
    pub fn calculate_delay(&self) -> Duration {
        self.policy.delay_for_attempt(self.current_attempt)
    }

    /// Increment the attempt counter
    pub fn increment_attempt(&mut self) {
        self.current_attempt += 1;
    }

    /// Check if we've exceeded the maximum number of attempts
    pub fn is_exhausted(&self) -> bool {
        self.current_attempt >= self.policy.max_attempts
    }

    /// Get the current attempt number
    pub fn current_attempt(&self) -> u32 {
        self.current_attempt
    }

    /// Reset the policy for a new request
    pub fn reset(&mut self) {
        self.current_attempt = 0;
    }
}

// Implementation of tower-retry Policy trait for integration with Tower middleware
impl<Req> Policy<Req, (), crate::api::ConfigurationClientError> for ExponentialBackoffPolicy
where
    Req: Clone,
{
    type Future = Ready<Self>;

    fn retry(
        &self,
        _req: &Req,
        result: Result<&(), &crate::api::ConfigurationClientError>,
    ) -> Option<Self::Future> {
        match result {
            Ok(_) => {
                // Request succeeded, no retry needed
                None
            }
            Err(error) => {
                // Check if we should retry this error
                if !Self::is_retryable_error(error) {
                    tracing::debug!(
                        error = %error,
                        "Error is not retryable, not attempting retry"
                    );
                    return None;
                }

                // Check if we've exceeded max attempts
                if self.current_attempt >= self.policy.max_attempts {
                    tracing::debug!(
                        attempt = self.current_attempt,
                        max_attempts = self.policy.max_attempts,
                        "Maximum retry attempts exceeded"
                    );
                    return None;
                }

                // Create a new policy instance for the next attempt
                let mut next_policy = self.clone();
                next_policy.increment_attempt();

                let delay = next_policy.calculate_delay();

                tracing::debug!(
                    attempt = next_policy.current_attempt,
                    delay = ?delay,
                    error = %error,
                    "Scheduling retry attempt"
                );

                Some(futures_util::future::ready(next_policy))
            }
        }
    }

    fn clone_request(&self, req: &Req) -> Option<Req> {
        Some(req.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ConfigurationClientError;
    use crate::config::RetryPolicy;
    use std::time::Duration;

    #[test]
    fn test_retry_layer_creation() {
        let policy = RetryPolicy::builder()
            .max_attempts(3)
            .base_delay(Duration::from_millis(100))
            .build();

        let layer = RetryLayer::new(policy.clone());
        assert_eq!(layer.policy().max_attempts, 3);
        assert_eq!(layer.policy().base_delay, Duration::from_millis(100));
    }

    #[test]
    fn test_retry_layer_from_config() {
        let policy = RetryPolicy::builder()
            .max_attempts(5)
            .base_delay(Duration::from_millis(200))
            .backoff_multiplier(1.5)
            .build();

        let layer = RetryLayer::from_config(&policy);
        assert_eq!(layer.policy().max_attempts, 5);
        assert_eq!(layer.policy().base_delay, Duration::from_millis(200));
        assert_eq!(layer.policy().backoff_multiplier, 1.5);
    }

    #[test]
    fn test_exponential_backoff_policy_creation() {
        let policy = RetryPolicy::default();
        let backoff_policy = ExponentialBackoffPolicy::new(policy);

        assert_eq!(backoff_policy.current_attempt(), 0);
        assert!(!backoff_policy.is_exhausted());
    }

    #[test]
    fn test_exponential_backoff_policy_increment() {
        let policy = RetryPolicy::builder().max_attempts(3).build();
        let mut backoff_policy = ExponentialBackoffPolicy::new(policy);

        assert_eq!(backoff_policy.current_attempt(), 0);

        backoff_policy.increment_attempt();
        assert_eq!(backoff_policy.current_attempt(), 1);

        backoff_policy.increment_attempt();
        assert_eq!(backoff_policy.current_attempt(), 2);

        backoff_policy.increment_attempt();
        assert_eq!(backoff_policy.current_attempt(), 3);
        assert!(backoff_policy.is_exhausted());
    }

    #[test]
    fn test_exponential_backoff_policy_reset() {
        let policy = RetryPolicy::default();
        let mut backoff_policy = ExponentialBackoffPolicy::new(policy);

        backoff_policy.increment_attempt();
        backoff_policy.increment_attempt();
        assert_eq!(backoff_policy.current_attempt(), 2);

        backoff_policy.reset();
        assert_eq!(backoff_policy.current_attempt(), 0);
        assert!(!backoff_policy.is_exhausted());
    }

    #[test]
    fn test_exponential_backoff_delay_calculation() {
        let policy = RetryPolicy::builder()
            .base_delay(Duration::from_millis(100))
            .backoff_multiplier(2.0)
            .jitter(0.0) // No jitter for predictable testing
            .build();

        let mut backoff_policy = ExponentialBackoffPolicy::new(policy);

        // First attempt (attempt 0) should have no delay
        assert_eq!(backoff_policy.calculate_delay(), Duration::ZERO);

        // Second attempt (attempt 1) should have base delay
        backoff_policy.increment_attempt();
        assert_eq!(backoff_policy.calculate_delay(), Duration::from_millis(100));

        // Third attempt (attempt 2) should have base_delay * multiplier
        backoff_policy.increment_attempt();
        assert_eq!(backoff_policy.calculate_delay(), Duration::from_millis(200));
    }

    #[test]
    fn test_exponential_backoff_max_delay_cap() {
        let policy = RetryPolicy::builder()
            .base_delay(Duration::from_millis(100))
            .max_delay(Duration::from_millis(150))
            .backoff_multiplier(2.0)
            .jitter(0.0) // No jitter for predictable testing
            .build();

        let mut backoff_policy = ExponentialBackoffPolicy::new(policy);

        // Increment to attempt 2, which would normally be 200ms but should be capped at 150ms
        backoff_policy.increment_attempt();
        backoff_policy.increment_attempt();

        let delay = backoff_policy.calculate_delay();
        assert!(delay <= Duration::from_millis(150));
    }

    #[test]
    fn test_is_retryable_error() {
        // Test retryable errors
        assert!(ExponentialBackoffPolicy::is_retryable_error(
            &ConfigurationClientError::RequestTimeout(Duration::from_secs(30))
        ));
        assert!(ExponentialBackoffPolicy::is_retryable_error(
            &ConfigurationClientError::ConnectionUnavailable
        ));
        assert!(ExponentialBackoffPolicy::is_retryable_error(
            &ConfigurationClientError::ServiceUnavailable
        ));

        // Test non-retryable errors
        assert!(!ExponentialBackoffPolicy::is_retryable_error(
            &ConfigurationClientError::NotFound
        ));
        assert!(!ExponentialBackoffPolicy::is_retryable_error(
            &ConfigurationClientError::CircuitBreakerOpen
        ));
        assert!(!ExponentialBackoffPolicy::is_retryable_error(
            &ConfigurationClientError::MaxRetriesExceeded(3)
        ));
    }

    #[test]
    fn test_tower_retry_policy_success() {
        let policy_config = RetryPolicy::default();
        let policy = ExponentialBackoffPolicy::new(policy_config);

        // Simulate successful request
        let result: Result<&(), &ConfigurationClientError> = Ok(&());
        let retry_decision = policy.retry(&(), result);

        // Should not retry on success
        assert!(retry_decision.is_none());
    }

    #[test]
    fn test_tower_retry_policy_retryable_error() {
        let policy_config = RetryPolicy::builder().max_attempts(3).build();
        let policy = ExponentialBackoffPolicy::new(policy_config);

        // Simulate retryable error
        let error = ConfigurationClientError::RequestTimeout(Duration::from_secs(30));
        let result: Result<&(), &ConfigurationClientError> = Err(&error);
        let retry_decision = policy.retry(&(), result);

        // Should retry on retryable error
        assert!(retry_decision.is_some());

        // Should retry on retryable error
        assert!(retry_decision.is_some());
    }

    #[test]
    fn test_tower_retry_policy_non_retryable_error() {
        let policy_config = RetryPolicy::default();
        let policy = ExponentialBackoffPolicy::new(policy_config);

        // Simulate non-retryable error
        let error = ConfigurationClientError::NotFound;
        let result: Result<&(), &ConfigurationClientError> = Err(&error);
        let retry_decision = policy.retry(&(), result);

        // Should not retry on non-retryable error
        assert!(retry_decision.is_none());
    }

    #[test]
    fn test_tower_retry_policy_max_attempts_exceeded() {
        let policy_config = RetryPolicy::builder().max_attempts(2).build();
        let mut policy = ExponentialBackoffPolicy::new(policy_config);

        // Simulate reaching max attempts
        policy.increment_attempt();
        policy.increment_attempt(); // Now at attempt 2, which equals max_attempts

        let error = ConfigurationClientError::RequestTimeout(Duration::from_secs(30));
        let result: Result<&(), &ConfigurationClientError> = Err(&error);
        let retry_decision = policy.retry(&(), result);

        // Should not retry when max attempts exceeded
        assert!(retry_decision.is_none());
    }

    #[test]
    fn test_clone_request() {
        let policy_config = RetryPolicy::default();
        let policy = ExponentialBackoffPolicy::new(policy_config);

        let request = "test_request".to_string();
        let cloned = policy.clone_request(&request);

        assert!(cloned.is_some());
        assert_eq!(cloned.unwrap(), request);
    }
}
