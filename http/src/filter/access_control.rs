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
            AccessControlEvaluationResult::Denied // If no matcher apply, default to denied
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
    use rstest::*;

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

    #[rstest]
    #[case(
        "1.2.3.4",
        "1.2.3.4",
        AccessControlEvaluationResult::Allowed,
        "allow only ip match"
    )]
    #[case(
        "1.2.3.4",
        "5.6.7.8",
        AccessControlEvaluationResult::Denied,
        "allow only ip no match"
    )]
    fn test_allow_only_ip_scenarios(
        #[case] allow_ip: &str,
        #[case] test_ip: &str,
        #[case] expected: AccessControlEvaluationResult,
        #[case] scenario: &str,
    ) {
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip(ip(allow_ip));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip(test_ip)),
            expected,
            "Failed for scenario: {}",
            scenario
        );
    }

    #[rstest]
    #[case(
        "1.2.3.4",
        "1.2.3.4",
        AccessControlEvaluationResult::Denied,
        "deny only ip match"
    )]
    #[case(
        "1.2.3.4",
        "5.6.7.8",
        AccessControlEvaluationResult::Denied,
        "deny only ip no match"
    )]
    fn test_deny_only_ip_scenarios(
        #[case] deny_ip: &str,
        #[case] test_ip: &str,
        #[case] expected: AccessControlEvaluationResult,
        #[case] scenario: &str,
    ) {
        let mut handler = AccessControlFilterHandler::builder();
        handler.deny_ip(ip(deny_ip));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip(test_ip)),
            expected,
            "Failed for scenario: {}",
            scenario
        );
    }

    #[rstest]
    #[case(
        "1.2.3.4",
        "1.2.3.4",
        "1.2.3.4",
        AccessControlEvaluationResult::Denied,
        "allow and deny ip deny match"
    )]
    #[case(
        "1.2.3.4",
        "5.6.7.8",
        "1.2.3.4",
        AccessControlEvaluationResult::Allowed,
        "allow and deny ip allow match"
    )]
    #[case(
        "1.2.3.4",
        "5.6.7.8",
        "9.9.9.9",
        AccessControlEvaluationResult::Denied,
        "allow and deny ip no match"
    )]
    fn test_allow_and_deny_ip_scenarios(
        #[case] allow_ip: &str,
        #[case] deny_ip: &str,
        #[case] test_ip: &str,
        #[case] expected: AccessControlEvaluationResult,
        #[case] scenario: &str,
    ) {
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip(ip(allow_ip));
        handler.deny_ip(ip(deny_ip));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip(test_ip)),
            expected,
            "Failed for scenario: {}",
            scenario
        );
    }

    #[rstest]
    #[case(
        "10.0.0.0/8",
        "10.1.2.3",
        AccessControlEvaluationResult::Allowed,
        "allow ip range match"
    )]
    #[case(
        "10.0.0.0/8",
        "192.168.1.1",
        AccessControlEvaluationResult::Denied,
        "allow ip range no match"
    )]
    fn test_allow_ip_range_scenarios(
        #[case] allow_range: &str,
        #[case] test_ip: &str,
        #[case] expected: AccessControlEvaluationResult,
        #[case] scenario: &str,
    ) {
        let mut handler = AccessControlFilterHandler::builder();
        handler.allow_ip_range(ipnet(allow_range));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip(test_ip)),
            expected,
            "Failed for scenario: {}",
            scenario
        );
    }

    #[rstest]
    #[case(
        "10.0.0.0/8",
        "10.1.2.3",
        AccessControlEvaluationResult::Denied,
        "deny ip range match"
    )]
    #[case(
        "10.0.0.0/8",
        "192.168.1.1",
        AccessControlEvaluationResult::Denied,
        "deny ip range no match"
    )]
    fn test_deny_ip_range_scenarios(
        #[case] deny_range: &str,
        #[case] test_ip: &str,
        #[case] expected: AccessControlEvaluationResult,
        #[case] scenario: &str,
    ) {
        let mut handler = AccessControlFilterHandler::builder();
        handler.deny_ip_range(ipnet(deny_range));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip(test_ip)),
            expected,
            "Failed for scenario: {}",
            scenario
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

    #[rstest]
    #[case("1.2.3.4", AccessControlEvaluationResult::Allowed)]
    #[case("192.168.1.1", AccessControlEvaluationResult::Allowed)]
    #[case("10.0.0.1", AccessControlEvaluationResult::Allowed)]
    #[tokio::test]
    async fn test_access_control_allow_all(
        #[case] test_ip: &str,
        #[case] expected: AccessControlEvaluationResult,
    ) {
        // Test case for allowing all requests - no matcher means allow all
        let handler = AccessControlFilterHandler::builder().build();

        // Should allow any IP when no rules are defined
        assert_eq!(handler.evaluate(ip(test_ip)), expected);
    }

    #[tokio::test]
    async fn test_access_control_deny_all() {
        // Test case for denying all requests - add deny rule for all private and public ranges
        let mut handler = AccessControlFilterHandler::builder();
        handler.deny_ip_range(ipnet("0.0.0.0/0")); // Deny all IPv4
        let handler = handler.build();

        // Should deny any IP when deny all rule is defined
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
}
