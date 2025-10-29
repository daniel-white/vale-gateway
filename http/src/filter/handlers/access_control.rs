use ipnet::IpNet;
use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use thiserror::Error;
use tower::Service;
use tracing::instrument;
use vg_config::http::filter::access_control::{AccessControlEffect, AccessControlFilter};
use vg_core::net::IpRef;

use crate::filter::error::FilterError;
use crate::filter::traits::{FilterHandler, InboundRequestFilter};
use crate::filter::types::{FilterRequest, FilterResponse};

/// Errors specific to AccessControl filter configuration
#[derive(Debug, Error)]
pub enum AccessControlError {
    #[error("Invalid IP configuration: {message}")]
    InvalidIpConfig { message: String },

    #[error("Invalid access control effect")]
    InvalidEffect,

    #[error("No client addresses specified")]
    NoClientAddresses,
}

// Automatic conversion from AccessControlError to unified FilterError
impl From<AccessControlError> for FilterError {
    fn from(err: AccessControlError) -> Self {
        FilterError::Configuration {
            message: err.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
enum IpMatcher {
    Ip(IpAddr),
    IpRange(IpNet),
}

impl IpMatcher {
    #[inline(always)]
    pub fn matches(&self, client_addr: &IpAddr) -> bool {
        match self {
            Self::Ip(ip) => ip == client_addr,
            Self::IpRange(range) => range.contains(client_addr),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessControlEvaluationResult {
    Allowed,
    Denied,
}

impl AccessControlEvaluationResult {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }

    pub fn is_denied(&self) -> bool {
        matches!(self, Self::Denied)
    }
}

#[derive(Debug, Clone)]
pub struct AccessControlFilterHandler {
    allow_matchers: Arc<[IpMatcher]>,
    deny_matchers: Arc<[IpMatcher]>,
}

impl AccessControlFilterHandler {
    #[instrument(name = "AccessControlFilterHandler::evaluate", skip(self, client_addr))]
    #[inline]
    pub fn evaluate(&self, client_addr: IpAddr) -> AccessControlEvaluationResult {
        // Fast path: if no rules are defined, default to allowed
        if self.allow_matchers.is_empty() && self.deny_matchers.is_empty() {
            return AccessControlEvaluationResult::Allowed;
        }

        // Check deny matchers first (they take precedence) - early return on match
        for matcher in self.deny_matchers.iter() {
            if matcher.matches(&client_addr) {
                return AccessControlEvaluationResult::Denied;
            }
        }

        // Check allow matchers - early return on match
        for matcher in self.allow_matchers.iter() {
            if matcher.matches(&client_addr) {
                return AccessControlEvaluationResult::Allowed;
            }
        }

        // Default to denied if no matchers apply
        AccessControlEvaluationResult::Denied
    }
}

impl Service<FilterRequest> for AccessControlFilterHandler {
    type Response = FilterResponse;
    type Error = FilterError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: FilterRequest) -> Self::Future {
        let (parts, body) = req.into_parts();

        // Clone Arc references (cheap operation)
        let allow_matchers = Arc::clone(&self.allow_matchers);
        let deny_matchers = Arc::clone(&self.deny_matchers);

        Box::pin(async move {
            // Extract client IP from extensions
            let client_ip = parts
                .extensions
                .get::<crate::extensions::ClientIp>()
                .ok_or(FilterError::NoClientIp)?
                .ip();

            // Evaluate access control rules directly without creating temporary handler
            let result = {
                // Fast path: if no rules are defined, default to allowed
                if allow_matchers.is_empty() && deny_matchers.is_empty() {
                    AccessControlEvaluationResult::Allowed
                } else {
                    // Check deny matchers first (they take precedence) - early return on match
                    let is_denied = deny_matchers
                        .iter()
                        .any(|matcher| matcher.matches(&client_ip));
                    if is_denied {
                        AccessControlEvaluationResult::Denied
                    } else {
                        // Check allow matchers - early return on match
                        let is_allowed = allow_matchers
                            .iter()
                            .any(|matcher| matcher.matches(&client_ip));
                        if is_allowed {
                            AccessControlEvaluationResult::Allowed
                        } else {
                            AccessControlEvaluationResult::Denied
                        }
                    }
                }
            };

            if result.is_denied() {
                // Return a 403 Forbidden response instead of an error
                let response = http::Response::builder()
                    .status(http::StatusCode::FORBIDDEN)
                    .body(())
                    .map_err(|e| FilterError::HttpProtocol {
                        message: e.to_string(),
                    })?;
                return Ok(response);
            }

            // Reconstruct request and continue (pass through)
            let _request = http::Request::from_parts(parts, body);
            Ok(http::Response::builder()
                .status(http::StatusCode::OK)
                .body(())
                .map_err(|e| FilterError::HttpProtocol {
                    message: e.to_string(),
                })?)
        })
    }
}

impl FilterHandler for AccessControlFilterHandler {
    type Config = AccessControlFilter;
    type ConfigError = AccessControlError;

    fn try_from_config(config: Self::Config) -> Result<Self, Self::ConfigError> {
        if config.clients().is_empty() {
            return Err(AccessControlError::NoClientAddresses);
        }

        let mut allow_matchers = Vec::new();
        let mut deny_matchers = Vec::new();

        for ip_ref in config.clients() {
            let matcher = match ip_ref {
                IpRef::Addr(addr) => IpMatcher::Ip(*addr),
                IpRef::Net(net) => IpMatcher::IpRange(*net),
            };

            match config.effect() {
                AccessControlEffect::Allow => allow_matchers.push(matcher),
                AccessControlEffect::Deny => deny_matchers.push(matcher),
            }
        }

        Ok(Self {
            allow_matchers: allow_matchers.into(),
            deny_matchers: deny_matchers.into(),
        })
    }
}

impl TryFrom<AccessControlFilter> for AccessControlFilterHandler {
    type Error = AccessControlError;

    fn try_from(config: AccessControlFilter) -> Result<Self, Self::Error> {
        Self::try_from_config(config)
    }
}

impl InboundRequestFilter for AccessControlFilterHandler {}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;
    use std::str::FromStr;
    use tower::ServiceExt;

    fn ip(s: &str) -> IpAddr {
        IpAddr::from_str(s).unwrap()
    }
    fn ipnet(s: &str) -> IpNet {
        IpNet::from_str(s).unwrap()
    }

    fn create_test_request(client_ip: IpAddr) -> FilterRequest {
        let mut request = http::Request::builder().uri("/test").body(()).unwrap();
        request
            .extensions_mut()
            .insert(crate::extensions::ClientIp::new(client_ip));
        request
    }

    #[test]
    fn test_no_matchers_evaluation() {
        let config = AccessControlFilter::builder()
            .effect(AccessControlEffect::Allow)
            .clients(vec![IpRef::Addr(ip("192.168.1.1"))])
            .build();

        let handler = AccessControlFilterHandler::try_from(config).unwrap();
        assert_eq!(
            handler.evaluate(ip("1.2.3.4")),
            AccessControlEvaluationResult::Denied
        );
        assert_eq!(
            handler.evaluate(ip("192.168.1.1")),
            AccessControlEvaluationResult::Allowed
        );
    }

    #[rstest]
    #[case(
        AccessControlEffect::Allow,
        "1.2.3.4",
        "1.2.3.4",
        AccessControlEvaluationResult::Allowed,
        "allow only ip match"
    )]
    #[case(
        AccessControlEffect::Allow,
        "1.2.3.4",
        "5.6.7.8",
        AccessControlEvaluationResult::Denied,
        "allow only ip no match"
    )]
    #[case(
        AccessControlEffect::Deny,
        "1.2.3.4",
        "1.2.3.4",
        AccessControlEvaluationResult::Denied,
        "deny ip match"
    )]
    #[case(
        AccessControlEffect::Deny,
        "1.2.3.4",
        "5.6.7.8",
        AccessControlEvaluationResult::Denied,
        "deny ip no match"
    )]
    fn test_ip_scenarios(
        #[case] effect: AccessControlEffect,
        #[case] config_ip: &str,
        #[case] test_ip: &str,
        #[case] expected: AccessControlEvaluationResult,
        #[case] scenario: &str,
    ) {
        let config = AccessControlFilter::builder()
            .effect(effect)
            .clients(vec![IpRef::Addr(ip(config_ip))])
            .build();

        let handler = AccessControlFilterHandler::try_from(config).unwrap();
        assert_eq!(
            handler.evaluate(ip(test_ip)),
            expected,
            "Failed for scenario: {}",
            scenario
        );
    }

    #[rstest]
    #[case(
        AccessControlEffect::Allow,
        "10.0.0.0/8",
        "10.1.2.3",
        AccessControlEvaluationResult::Allowed,
        "allow ip range match"
    )]
    #[case(
        AccessControlEffect::Allow,
        "10.0.0.0/8",
        "192.168.1.1",
        AccessControlEvaluationResult::Denied,
        "allow ip range no match"
    )]
    #[case(
        AccessControlEffect::Deny,
        "10.0.0.0/8",
        "10.1.2.3",
        AccessControlEvaluationResult::Denied,
        "deny ip range match"
    )]
    #[case(
        AccessControlEffect::Deny,
        "10.0.0.0/8",
        "192.168.1.1",
        AccessControlEvaluationResult::Denied,
        "deny ip range no match"
    )]
    fn test_ip_range_scenarios(
        #[case] effect: AccessControlEffect,
        #[case] range: &str,
        #[case] test_ip: &str,
        #[case] expected: AccessControlEvaluationResult,
        #[case] scenario: &str,
    ) {
        let config = AccessControlFilter::builder()
            .effect(effect)
            .clients(vec![IpRef::Net(ipnet(range))])
            .build();

        let handler = AccessControlFilterHandler::try_from(config).unwrap();
        assert_eq!(
            handler.evaluate(ip(test_ip)),
            expected,
            "Failed for scenario: {}",
            scenario
        );
    }

    #[tokio::test]
    async fn test_service_allowed_request() {
        let config = AccessControlFilter::builder()
            .effect(AccessControlEffect::Allow)
            .clients(vec![IpRef::Addr(ip("192.168.1.100"))])
            .build();

        let mut handler = AccessControlFilterHandler::try_from(config).unwrap();
        let request = create_test_request(ip("192.168.1.100"));

        let response = handler.call(request).await.unwrap();
        assert_eq!(response.status(), http::StatusCode::OK);
    }

    #[tokio::test]
    async fn test_service_denied_request_returns_403() {
        let config = AccessControlFilter::builder()
            .effect(AccessControlEffect::Allow)
            .clients(vec![IpRef::Addr(ip("192.168.1.100"))])
            .build();

        let mut handler = AccessControlFilterHandler::try_from(config).unwrap();
        let request = create_test_request(ip("203.0.113.1")); // Different IP, should be denied

        let response = handler.call(request).await.unwrap();
        assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_service_no_client_ip_extension() {
        let config = AccessControlFilter::builder()
            .effect(AccessControlEffect::Allow)
            .clients(vec![IpRef::Addr(ip("192.168.1.100"))])
            .build();

        let mut handler = AccessControlFilterHandler::try_from(config).unwrap();
        let request = http::Request::builder().uri("/test").body(()).unwrap(); // No ClientIp extension

        let result = handler.call(request).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No client IP found")
        );
    }

    #[test]
    fn test_try_from_config_allow_effect() {
        let config = AccessControlFilter::builder()
            .effect(AccessControlEffect::Allow)
            .clients(vec![
                IpRef::Addr(ip("192.168.1.1")),
                IpRef::Net(ipnet("10.0.0.0/8")),
            ])
            .build();

        let handler = AccessControlFilterHandler::try_from(config).unwrap();

        // Test that the handler works as expected
        assert_eq!(
            handler.evaluate(ip("192.168.1.1")),
            AccessControlEvaluationResult::Allowed
        );
        assert_eq!(
            handler.evaluate(ip("10.1.2.3")),
            AccessControlEvaluationResult::Allowed
        );
        assert_eq!(
            handler.evaluate(ip("8.8.8.8")),
            AccessControlEvaluationResult::Denied
        );
    }

    #[test]
    fn test_try_from_config_deny_effect() {
        let config = AccessControlFilter::builder()
            .effect(AccessControlEffect::Deny)
            .clients(vec![
                IpRef::Addr(ip("192.168.1.1")),
                IpRef::Net(ipnet("10.0.0.0/8")),
            ])
            .build();

        let handler = AccessControlFilterHandler::try_from(config).unwrap();

        // Test that the handler works as expected
        assert_eq!(
            handler.evaluate(ip("192.168.1.1")),
            AccessControlEvaluationResult::Denied
        );
        assert_eq!(
            handler.evaluate(ip("10.1.2.3")),
            AccessControlEvaluationResult::Denied
        );
        assert_eq!(
            handler.evaluate(ip("8.8.8.8")),
            AccessControlEvaluationResult::Denied // Default deny when no allow rules
        );
    }

    #[test]
    fn test_try_from_config_empty_clients() {
        let config = AccessControlFilter::builder()
            .effect(AccessControlEffect::Allow)
            .clients(vec![])
            .build();

        let result = AccessControlFilterHandler::try_from(config);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No client addresses specified")
        );
    }

    #[tokio::test]
    async fn test_service_with_oneshot() {
        let config = AccessControlFilter::builder()
            .effect(AccessControlEffect::Allow)
            .clients(vec![IpRef::Addr(ip("192.168.1.100"))])
            .build();

        let handler = AccessControlFilterHandler::try_from(config).unwrap();
        let request = create_test_request(ip("192.168.1.100"));

        let response = handler.oneshot(request).await.unwrap();
        assert_eq!(response.status(), http::StatusCode::OK);
    }
}
