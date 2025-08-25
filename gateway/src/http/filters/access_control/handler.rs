use ipnet::IpNet;
use std::net::IpAddr;
use tracing::instrument;
use vg_core::http::filters::access_control::{HttpAccessControlEffect, HttpAccessControlFilter};

#[derive(Debug, PartialEq, Eq)]
enum HttpAccessControlFilterClientMatcher {
    Ip(IpAddr),
    IpRange(IpNet),
}

impl HttpAccessControlFilterClientMatcher {
    pub fn matches(&self, client_addr: &IpAddr) -> bool {
        match self {
            Self::Ip(ip) => ip == client_addr,
            Self::IpRange(range) => range.contains(client_addr),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum HttpAccessControlEvaluationResult {
    Allowed,
    Denied,
}

impl HttpAccessControlEvaluationResult {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }

    pub fn is_denied(&self) -> bool {
        matches!(self, Self::Denied)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct HttpAccessControlFilterHandler {
    allow_matchers: Vec<HttpAccessControlFilterClientMatcher>,
    deny_matchers: Vec<HttpAccessControlFilterClientMatcher>,
}

impl HttpAccessControlFilterHandler {
    pub fn builder() -> HttpAccessControlFilterHandlerBuilder {
        HttpAccessControlFilterHandlerBuilder::new()
    }

    pub fn from(filter: &HttpAccessControlFilter) -> Self {
        let mut handler = Self::builder();

        match filter.effect() {
            HttpAccessControlEffect::Allow => {
                for ip in filter.clients().ips() {
                    handler.allow_ip(*ip);
                }
                for range in filter.clients().ip_ranges() {
                    handler.allow_ip_range(*range);
                }
            }
            HttpAccessControlEffect::Deny => {
                for ip in filter.clients().ips() {
                    handler.deny_ip(*ip);
                }
                for range in filter.clients().ip_ranges() {
                    handler.deny_ip_range(*range);
                }
            }
        }

        handler.build()
    }

    #[instrument(name = "AccessControlFilterHandler::evaluate", skip(self, client_addr))]
    pub fn evaluate(&self, client_addr: IpAddr) -> HttpAccessControlEvaluationResult {
        if self.allow_matchers.is_empty() && self.deny_matchers.is_empty() {
            return HttpAccessControlEvaluationResult::Allowed; // Default to allowed if no rules are defined
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
            HttpAccessControlEvaluationResult::Denied // Any deny matcher takes precedence
        } else if is_allowed {
            HttpAccessControlEvaluationResult::Allowed // If there's an allow matcher and no deny matches, allow access
        } else {
            HttpAccessControlEvaluationResult::Denied // If no matchers apply, default to denied
        }
    }
}

pub struct HttpAccessControlFilterHandlerBuilder {
    allow_matchers: Vec<HttpAccessControlFilterClientMatcher>,
    deny_matchers: Vec<HttpAccessControlFilterClientMatcher>,
}

impl HttpAccessControlFilterHandlerBuilder {
    fn new() -> Self {
        Self {
            allow_matchers: Vec::new(),
            deny_matchers: Vec::new(),
        }
    }

    pub fn allow_ip(&mut self, ip: IpAddr) -> &Self {
        self.allow_matchers
            .push(HttpAccessControlFilterClientMatcher::Ip(ip));
        self
    }

    pub fn allow_ip_range(&mut self, range: IpNet) -> &Self {
        self.allow_matchers
            .push(HttpAccessControlFilterClientMatcher::IpRange(range));
        self
    }

    pub fn deny_ip(&mut self, ip: IpAddr) -> &Self {
        self.deny_matchers
            .push(HttpAccessControlFilterClientMatcher::Ip(ip));
        self
    }

    pub fn deny_ip_range(&mut self, range: IpNet) -> &Self {
        self.deny_matchers
            .push(HttpAccessControlFilterClientMatcher::IpRange(range));
        self
    }

    pub fn build(self) -> HttpAccessControlFilterHandler {
        HttpAccessControlFilterHandler {
            allow_matchers: self.allow_matchers,
            deny_matchers: self.deny_matchers,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn ip(s: &str) -> IpAddr {
        IpAddr::from_str(s).unwrap()
    }
    fn ipnet(s: &str) -> IpNet {
        IpNet::from_str(s).unwrap()
    }
    
    #[test]
    fn test_no_matchers_ip_some() {
        let handler = HttpAccessControlFilterHandler::builder().build();
        assert_eq!(
            handler.evaluate(ip("1.2.3.4")),
            HttpAccessControlEvaluationResult::Allowed
        );
    }

    #[test]
    fn test_allow_only_ip_some_match() {
        let mut handler = HttpAccessControlFilterHandler::builder();
        handler.allow_ip(ip("1.2.3.4"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("1.2.3.4")),
            HttpAccessControlEvaluationResult::Allowed
        );
    }
    #[test]
    fn test_allow_only_ip_some_no_match() {
        let mut handler = HttpAccessControlFilterHandler::builder();
        handler.allow_ip(ip("1.2.3.4"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("5.6.7.8")),
            HttpAccessControlEvaluationResult::Denied
        );
    }

    #[test]
    fn test_deny_only_ip_some_match() {
        let mut handler = HttpAccessControlFilterHandler::builder();
        handler.deny_ip(ip("1.2.3.4"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("1.2.3.4")),
            HttpAccessControlEvaluationResult::Denied
        );
    }
    #[test]
    fn test_deny_only_ip_some_no_match() {
        let mut handler = HttpAccessControlFilterHandler::builder();
        handler.deny_ip(ip("1.2.3.4"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("5.6.7.8")),
            HttpAccessControlEvaluationResult::Denied
        );
    }

    #[test]
    fn test_allow_and_deny_ip_some_deny_match() {
        let mut handler = HttpAccessControlFilterHandler::builder();
        handler.allow_ip(ip("1.2.3.4"));
        handler.deny_ip(ip("1.2.3.4"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("1.2.3.4")),
            HttpAccessControlEvaluationResult::Denied
        );
    }
    #[test]
    fn test_allow_and_deny_ip_some_allow_match() {
        let mut handler = HttpAccessControlFilterHandler::builder();
        handler.allow_ip(ip("1.2.3.4"));
        handler.deny_ip(ip("5.6.7.8"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("1.2.3.4")),
            HttpAccessControlEvaluationResult::Allowed
        );
    }
    #[test]
    fn test_allow_and_deny_ip_some_no_match() {
        let mut handler = HttpAccessControlFilterHandler::builder();
        handler.allow_ip(ip("1.2.3.4"));
        handler.deny_ip(ip("5.6.7.8"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("9.9.9.9")),
            HttpAccessControlEvaluationResult::Denied
        );
    }
    #[test]
    fn test_allow_ip_range_match() {
        let mut handler = HttpAccessControlFilterHandler::builder();
        handler.allow_ip_range(ipnet("10.0.0.0/8"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("10.1.2.3")),
            HttpAccessControlEvaluationResult::Allowed
        );
    }

    #[test]
    fn test_allow_ip_range_no_match() {
        let mut handler = HttpAccessControlFilterHandler::builder();
        handler.allow_ip_range(ipnet("10.0.0.0/8"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("192.168.1.1")),
            HttpAccessControlEvaluationResult::Denied
        );
    }

    #[test]
    fn test_deny_ip_range_match() {
        let mut handler = HttpAccessControlFilterHandler::builder();
        handler.deny_ip_range(ipnet("10.0.0.0/8"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("10.1.2.3")),
            HttpAccessControlEvaluationResult::Denied
        );
    }

    #[test]
    fn test_deny_ip_range_no_match() {
        let mut handler = HttpAccessControlFilterHandler::builder();
        handler.deny_ip_range(ipnet("10.0.0.0/8"));
        let handler = handler.build();
        assert_eq!(
            handler.evaluate(ip("192.168.1.1")),
            HttpAccessControlEvaluationResult::Denied
        );
    }

    #[test]
    fn test_allow_and_deny_ip_range_overlap_deny_precedence() {
        let mut handler = HttpAccessControlFilterHandler::builder();
        handler.allow_ip_range(ipnet("10.0.0.0/8"));
        handler.deny_ip_range(ipnet("10.1.0.0/16"));
        let handler = handler.build();
        // 10.1.2.3 is in both ranges, should be Denied
        assert_eq!(
            handler.evaluate(ip("10.1.2.3")),
            HttpAccessControlEvaluationResult::Denied
        );
        // 10.2.2.2 is only in allow range, should be Allowed
        assert_eq!(
            handler.evaluate(ip("10.2.2.2")),
            HttpAccessControlEvaluationResult::Allowed
        );
    }
}
