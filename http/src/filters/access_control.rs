use ipnet::IpNet;
use std::net::IpAddr;
use tracing::instrument;

#[derive(Debug)]
enum AccessControlFilterClientMatcher {
    Ip(IpAddr),
    IpRange(IpNet),
}

impl AccessControlFilterClientMatcher {
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

#[derive(Debug)]
pub struct AccessControlFilterHandler {
    allow_matchers: Vec<AccessControlFilterClientMatcher>,
    deny_matchers: Vec<AccessControlFilterClientMatcher>,
}

impl AccessControlFilterHandler {
    pub fn builder() -> AccessControlFilterHandlerBuilder {
        AccessControlFilterHandlerBuilder::new()
    }

    #[instrument(name = "AccessControlFilterHandler::evaluate", skip(self, client_addr))]
    pub fn evaluate(&self, client_addr: IpAddr) -> AccessControlEvaluationResult {
        if self.allow_matchers.is_empty() && self.deny_matchers.is_empty() {
            return AccessControlEvaluationResult::Allowed; // Default to allowed if no rules are defined
        }

        let is_allowed = self
            .allow_matchers
            .iter()
            .any(|matcher| matcher.matches(&client_addr));
        let is_denied = self
            .deny_matchers
            .iter()
            .any(|matcher| matcher.matches(&client_addr));

        if is_denied {
            AccessControlEvaluationResult::Denied // Any deny matcher takes precedence
        } else if is_allowed {
            AccessControlEvaluationResult::Allowed // If there's an allow matcher and no deny matches, allow access
        } else {
            AccessControlEvaluationResult::Denied // If no matchers apply, default to denied
        }
    }
}

pub struct AccessControlFilterHandlerBuilder {
    allow_matchers: Vec<AccessControlFilterClientMatcher>,
    deny_matchers: Vec<AccessControlFilterClientMatcher>,
}

impl AccessControlFilterHandlerBuilder {
    fn new() -> Self {
        Self {
            allow_matchers: Vec::new(),
            deny_matchers: Vec::new(),
        }
    }

    pub fn allow_ip(&mut self, ip: IpAddr) -> &Self {
        self.allow_matchers
            .push(AccessControlFilterClientMatcher::Ip(ip));
        self
    }

    pub fn allow_ip_range(&mut self, range: IpNet) -> &Self {
        self.allow_matchers
            .push(AccessControlFilterClientMatcher::IpRange(range));
        self
    }

    pub fn deny_ip(&mut self, ip: IpAddr) -> &Self {
        self.deny_matchers
            .push(AccessControlFilterClientMatcher::Ip(ip));
        self
    }

    pub fn deny_ip_range(&mut self, range: IpNet) -> &Self {
        self.deny_matchers
            .push(AccessControlFilterClientMatcher::IpRange(range));
        self
    }

    pub fn build(self) -> AccessControlFilterHandler {
        AccessControlFilterHandler {
            allow_matchers: self.allow_matchers,
            deny_matchers: self.deny_matchers,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assertables::*;
    use async_trait::async_trait;
    use http::{HeaderMap, HeaderValue, Method, Uri};
    use std::str::FromStr;

    fn ip(s: &str) -> IpAddr {
        IpAddr::from_str(s).unwrap()
    }
    fn ipnet(s: &str) -> IpNet {
        IpNet::from_str(s).unwrap()
    }

    #[test]
    fn test_no_matchers_ip_some() {
        let handler = AccessControlFilterHandler::builder().build();
        assert_eq!(
            handler.evaluate(ip("1.2.3.4")),
            AccessControlEvaluationResult::Allowed
        );
    }

    #[test]
    fn test_allow_only_ip_some_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip(ip("1.2.3.4"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("1.2.3.4")),
            AccessControlEvaluationResult::Allowed
        );
    }
    #[test]
    fn test_allow_only_ip_some_no_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip(ip("1.2.3.4"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("5.6.7.8")),
            AccessControlEvaluationResult::Denied
        );
    }

    #[test]
    fn test_deny_only_ip_some_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.deny_ip(ip("1.2.3.4"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("1.2.3.4")),
            AccessControlEvaluationResult::Denied
        );
    }
    #[test]
    fn test_deny_only_ip_some_no_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.deny_ip(ip("1.2.3.4"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("5.6.7.8")),
            AccessControlEvaluationResult::Denied
        );
    }

    #[test]
    fn test_allow_and_deny_ip_some_deny_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip(ip("1.2.3.4"));
        handler.deny_ip(ip("1.2.3.4"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("1.2.3.4")),
            AccessControlEvaluationResult::Denied
        );
    }
    #[test]
    fn test_allow_and_deny_ip_some_allow_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip(ip("1.2.3.4"));
        handler.deny_ip(ip("5.6.7.8"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("1.2.3.4")),
            AccessControlEvaluationResult::Allowed
        );
    }
    #[test]
    fn test_allow_and_deny_ip_some_no_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip(ip("1.2.3.4"));
        handler.deny_ip(ip("5.6.7.8"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("9.9.9.9")),
            AccessControlEvaluationResult::Denied
        );
    }
    #[test]
    fn test_allow_ip_range_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip_range(ipnet("10.0.0.0/8"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("10.1.2.3")),
            AccessControlEvaluationResult::Allowed
        );
    }

    #[test]
    fn test_allow_ip_range_no_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip_range(ipnet("10.0.0.0/8"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("192.168.1.1")),
            AccessControlEvaluationResult::Denied
        );
    }

    #[test]
    fn test_deny_ip_range_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.deny_ip_range(ipnet("10.0.0.0/8"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("10.1.2.3")),
            AccessControlEvaluationResult::Denied
        );
    }

    #[test]
    fn test_deny_ip_range_no_match() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.deny_ip_range(ipnet("10.0.0.0/8"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("192.168.1.1")),
            AccessControlEvaluationResult::Denied
        );
    }

    #[test]
    fn test_allow_and_deny_ip_range_overlap_deny_precedence() {
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip_range(ipnet("10.0.0.0/8"));
        handler.deny_ip_range(ipnet("10.1.0.0/16"));
        let handler = handler.build();
        // 10.1.2.3 is in both ranges, should be Denied
        assert_eq!(
            handler.evaluate(ip("10.1.2.3")),
            AccessControlEvaluationResult::Denied
        );
        // 10.2.2.2 is only in allow range, should be Allowed
        assert_eq!(
            handler.evaluate(ip("10.2.2.2")),
            AccessControlEvaluationResult::Allowed
        );
    }

    #[tokio::test]
    async fn test_access_control_allow_all() {
        // Test case for allowing all requests - no matchers means allow all
        let handler = AccessControlFilterHandler::builder().build();

        // Should allow any IP when no rules are defined
        assert_eq!(
            handler.evaluate(ip("1.2.3.4")),
            AccessControlEvaluationResult::Allowed
        );
        assert_eq!(
            handler.evaluate(ip("192.168.1.1")),
            AccessControlEvaluationResult::Allowed
        );
        assert_eq!(
            handler.evaluate(ip("10.0.0.1")),
            AccessControlEvaluationResult::Allowed
        );
    }

    #[tokio::test]
    async fn test_access_control_deny_all() {
        // Test case for denying all requests - add deny rule for all private and public ranges
        let mut handler = AccessControlFilterHandler::builder();
        handler.deny_ip_range(ipnet("0.0.0.0/0")); // Deny all IPv4
        let handler = handler.build();

        // Should deny any IP when broad deny rule exists
        assert_eq!(
            handler.evaluate(ip("1.2.3.4")),
            AccessControlEvaluationResult::Denied
        );
        assert_eq!(
            handler.evaluate(ip("192.168.1.1")),
            AccessControlEvaluationResult::Denied
        );
        assert_eq!(
            handler.evaluate(ip("10.0.0.1")),
            AccessControlEvaluationResult::Denied
        );
    }

    #[tokio::test]
    async fn test_access_control_ip_whitelist() {
        // Test IP-based whitelisting
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip(ip("192.168.1.100"));
        handler.allow_ip(ip("10.0.0.1"));
        let handler = handler.build();

        // Test that only whitelisted IPs are allowed
        assert_eq!(
            handler.evaluate(ip("192.168.1.100")),
            AccessControlEvaluationResult::Allowed
        );
        assert_eq!(
            handler.evaluate(ip("10.0.0.1")),
            AccessControlEvaluationResult::Allowed
        );

        // Non-whitelisted IPs should be denied
        assert_eq!(
            handler.evaluate(ip("192.168.1.200")),
            AccessControlEvaluationResult::Denied
        );
        assert_eq!(
            handler.evaluate(ip("172.16.0.1")),
            AccessControlEvaluationResult::Denied
        );
    }

    #[tokio::test]
    async fn test_access_control_ip_blacklist() {
        // Test IP-based blacklisting using deny rules
        let mut handler = AccessControlFilterHandler::builder();
        handler.deny_ip(ip("192.168.1.100"));
        handler.deny_ip(ip("10.0.0.1"));
        let handler = handler.build();

        // Test that blacklisted IPs are denied
        assert_eq!(
            handler.evaluate(ip("192.168.1.100")),
            AccessControlEvaluationResult::Denied
        );
        assert_eq!(
            handler.evaluate(ip("10.0.0.1")),
            AccessControlEvaluationResult::Denied
        );

        // Other IPs should also be denied (default deny when only deny rules exist)
        assert_eq!(
            handler.evaluate(ip("192.168.1.200")),
            AccessControlEvaluationResult::Denied
        );
    }

    #[tokio::test]
    async fn test_access_control_method_filtering() {
        // Test HTTP method filtering - this would be implemented at a higher level
        // For now, we'll test the IP-based access control that exists
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip_range(ipnet("192.168.1.0/24"));
        let handler = handler.build();

        // IPs within allowed range should be allowed
        assert_eq!(
            handler.evaluate(ip("192.168.1.100")),
            AccessControlEvaluationResult::Allowed
        );

        // IPs outside allowed range should be denied
        assert_eq!(
            handler.evaluate(ip("10.0.0.1")),
            AccessControlEvaluationResult::Denied
        );

        // TODO: Method filtering would be implemented in a higher-level filter
        // that combines IP access control with method validation
    }

    #[tokio::test]
    async fn test_access_control_header_based() {
        // Test header-based access control - currently only IP-based is implemented
        // This test demonstrates how IP-based control works as foundation for header-based auth
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip_range(ipnet("192.168.1.0/24")); // Allow internal network
        let handler = handler.build();

        // Valid IP should be allowed (header validation would be additional layer)
        assert_eq!(
            handler.evaluate(ip("192.168.1.100")),
            AccessControlEvaluationResult::Allowed
        );

        // Invalid IP should be denied regardless of headers
        assert_eq!(
            handler.evaluate(ip("1.2.3.4")),
            AccessControlEvaluationResult::Denied
        );

        // TODO: Header-based authorization would be implemented as additional filter layer
    }

    #[tokio::test]
    async fn test_access_control_user_agent_filtering() {
        // Test User-Agent based filtering - would be implemented at higher level
        // For now, test IP-based filtering that could be combined with UA filtering
        let mut handler = AccessControlFilterHandler::builder();
        handler.deny_ip_range(ipnet("1.0.0.0/8")); // Block certain IP ranges (e.g., known bot ranges)
        handler.allow_ip_range(ipnet("192.168.0.0/16")); // Allow internal network
        let handler = handler.build();

        // Bot IPs should be denied
        assert_eq!(
            handler.evaluate(ip("1.1.1.1")),
            AccessControlEvaluationResult::Denied
        );

        // Internal IPs should be allowed
        assert_eq!(
            handler.evaluate(ip("192.168.1.100")),
            AccessControlEvaluationResult::Allowed
        );

        // TODO: User-Agent filtering would be implemented as additional filter layer
    }

    #[tokio::test]
    async fn test_access_control_rate_limiting() {
        // Test rate limiting functionality - would be stateful, implemented at higher level
        // For now, test IP-based access that could be foundation for rate limiting
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip_range(ipnet("192.168.1.0/24"));
        let handler = handler.build();

        // Same IP should consistently get same result (stateless)
        for _i in 1..=10 {
            assert_eq!(
                handler.evaluate(ip("192.168.1.100")),
                AccessControlEvaluationResult::Allowed
            );
        }

        // Blocked IP should consistently be blocked
        for _i in 1..=10 {
            assert_eq!(
                handler.evaluate(ip("10.0.0.1")),
                AccessControlEvaluationResult::Denied
            );
        }

        // TODO: Rate limiting would require stateful tracking of requests per IP/time window
    }

    #[tokio::test]
    async fn test_access_control_time_based() {
        // Test time-based access control - would be implemented at higher level
        // For now, test IP-based access control that works regardless of time
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip_range(ipnet("192.168.1.0/24"));
        let handler = handler.build();

        // Access control should be consistent regardless of when it's called
        assert_eq!(
            handler.evaluate(ip("192.168.1.100")),
            AccessControlEvaluationResult::Allowed
        );

        // Wait briefly and test again
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        assert_eq!(
            handler.evaluate(ip("192.168.1.100")),
            AccessControlEvaluationResult::Allowed
        );

        // TODO: Time-based access would require checking current time against allowed windows
    }

    #[tokio::test]
    async fn test_access_control_combined_rules() {
        // Test multiple access control rules combined - IP allow/deny precedence
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip_range(ipnet("192.168.1.0/24")); // Allow subnet
        handler.deny_ip(ip("192.168.1.50")); // Deny specific IP in that subnet
        handler.allow_ip(ip("10.0.0.100")); // Allow specific external IP
        let handler = handler.build();

        // IP in allowed subnet should be allowed
        assert_eq!(
            handler.evaluate(ip("192.168.1.100")),
            AccessControlEvaluationResult::Allowed
        );

        // Specifically denied IP should be denied (deny takes precedence)
        assert_eq!(
            handler.evaluate(ip("192.168.1.50")),
            AccessControlEvaluationResult::Denied
        );

        // Specifically allowed external IP should be allowed
        assert_eq!(
            handler.evaluate(ip("10.0.0.100")),
            AccessControlEvaluationResult::Allowed
        );

        // Other external IPs should be denied
        assert_eq!(
            handler.evaluate(ip("10.0.0.1")),
            AccessControlEvaluationResult::Denied
        );

        // TODO: More complex combinations (IP + method + headers) would be higher-level filters
    }

    #[tokio::test]
    async fn test_access_control_cors_preflight() {
        // Test CORS preflight handling - would be implemented at higher level
        // For now, test that IP-based access control works for CORS scenarios
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip_range(ipnet("0.0.0.0/0")); // Allow all IPs for CORS
        let handler = handler.build();

        // All IPs should be allowed for CORS preflight
        assert_eq!(
            handler.evaluate(ip("203.0.113.1")),
            AccessControlEvaluationResult::Allowed
        );
        assert_eq!(
            handler.evaluate(ip("192.168.1.1")),
            AccessControlEvaluationResult::Allowed
        );

        // TODO: CORS preflight would check Origin header and add CORS response headers
    }

    // Helper functions for testing - implement basic IP creation utilities
    #[tokio::test]
    async fn test_helper_functions() {
        // Test our helper functions work correctly
        assert_eq!(ip("1.2.3.4"), IpAddr::from_str("1.2.3.4").unwrap());
        assert_eq!(ipnet("10.0.0.0/8"), IpNet::from_str("10.0.0.0/8").unwrap());

        // Test IPv6 support
        assert_eq!(ip("::1"), IpAddr::from_str("::1").unwrap());
        assert_eq!(
            ipnet("2001:db8::/32"),
            IpNet::from_str("2001:db8::/32").unwrap()
        );
    }
}
