use crate::extensions::TrustedClientIpAddr;
use getset::CopyGetters;
use ipnet::IpNet;
use std::net::IpAddr;
use typed_builder::TypedBuilder;
use vg_config::http::filter::access_control::{AccessControlEffect, AccessControlFilter};
use vg_core::net::IpRef;

#[derive(Debug)]
enum Matcher {
    Ip(IpAddr),
    IpRange(IpNet),
}

impl Matcher {
    #[inline(always)]
    pub fn matches(&self, client_addr: IpAddr) -> bool {
        match self {
            Self::Ip(ip) => *ip == client_addr,
            Self::IpRange(range) => range.contains(&client_addr),
        }
    }
}

impl From<&IpRef> for Matcher {
    fn from(value: &IpRef) -> Self {
        match value {
            IpRef::Addr(ip_addr) => Self::Ip(*ip_addr),
            IpRef::Net(ip_net) => Self::IpRange(*ip_net),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationResult {
    Allowed,
    Denied,
}

impl EvaluationResult {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }

    pub fn is_denied(&self) -> bool {
        matches!(self, Self::Denied)
    }

    pub fn negate(self) -> Self {
        match self {
            Self::Allowed => Self::Denied,
            Self::Denied => Self::Allowed,
        }
    }
}

impl From<AccessControlEffect> for EvaluationResult {
    fn from(value: AccessControlEffect) -> Self {
        match value {
            AccessControlEffect::Allow => Self::Allowed,
            AccessControlEffect::Deny => Self::Denied,
        }
    }
}

#[derive(Debug, TypedBuilder, CopyGetters)]
pub struct AccessControlPolicyHandler {
    #[getset(get_copy = "pub")]
    effect: AccessControlEffect,
    matchers: Vec<Matcher>,
}

impl AccessControlPolicyHandler {
    pub fn evaluate(&self, client_addr: &TrustedClientIpAddr) -> EvaluationResult {
        if self.matchers.is_empty() {
            let result: EvaluationResult = self.effect.into();
            result.negate()
        } else {
            let matches = self
                .matchers
                .iter()
                .any(|m| m.matches(client_addr.ip_addr()));

            if matches {
                self.effect.into()
            } else {
                let result: EvaluationResult = self.effect.into();
                result.negate()
            }
        }
    }
}

impl From<&AccessControlFilter> for AccessControlPolicyHandler {
    fn from(value: &AccessControlFilter) -> Self {
        let matcher: Vec<_> = value.clients().iter().map(Matcher::from).collect();
        Self::builder()
            .effect(value.effect())
            .matchers(matcher)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use crate::extensions::TrustedClientIpAddr;
    use crate::filter::handler::access_control::policy::{
        AccessControlPolicyHandler, EvaluationResult, Matcher,
    };
    use ipnet::IpNet;
    use std::net::IpAddr;
    use std::str::FromStr;
    use vg_config::http::filter::access_control::AccessControlEffect;

    fn ip(s: &str) -> IpAddr {
        IpAddr::from_str(s).unwrap()
    }

    fn ipnet(s: &str) -> IpNet {
        IpNet::from_str(s).unwrap()
    }

    fn trusted_ip(s: &str) -> TrustedClientIpAddr {
        let ip = ip(s);
        TrustedClientIpAddr::builder().ip_addr(ip).build()
    }

    #[test]
    fn no_matchers_allow() {
        let config = AccessControlPolicyHandler::builder()
            .effect(AccessControlEffect::Allow)
            .matchers(Vec::new())
            .build();

        let handler = AccessControlPolicyHandler::try_from(config).unwrap();
        assert_eq!(
            handler.evaluate(&trusted_ip("1.2.3.4")),
            EvaluationResult::Allowed
        );
        assert_eq!(
            handler.evaluate(&trusted_ip("192.168.1.1")),
            EvaluationResult::Allowed
        );
    }

    #[test]
    fn no_matchers_deny() {
        let config = AccessControlPolicyHandler::builder()
            .effect(AccessControlEffect::Deny)
            .matchers(Vec::new())
            .build();

        let handler = AccessControlPolicyHandler::try_from(config).unwrap();
        assert_eq!(
            handler.evaluate(&trusted_ip("1.2.3.4")),
            EvaluationResult::Denied
        );
        assert_eq!(
            handler.evaluate(&trusted_ip("192.168.1.1")),
            EvaluationResult::Denied
        );
    }

    #[test]
    fn matchers_allow() {
        let config = AccessControlPolicyHandler::builder()
            .effect(AccessControlEffect::Allow)
            .matchers(vec![
                Matcher::Ip(ip("127.0.0.1")),
                Matcher::IpRange(ipnet("192.168.1.1/24")),
            ])
            .build();

        let handler = AccessControlPolicyHandler::try_from(config).unwrap();
        assert_eq!(
            handler.evaluate(&trusted_ip("1.2.3.4")),
            EvaluationResult::Denied
        );
        assert_eq!(
            handler.evaluate(&trusted_ip("192.168.1.127")),
            EvaluationResult::Allowed
        );
        assert_eq!(
            handler.evaluate(&trusted_ip("127.0.0.1")),
            EvaluationResult::Allowed
        );
    }

    #[test]
    fn matchers_deny() {
        let config = AccessControlPolicyHandler::builder()
            .effect(AccessControlEffect::Deny)
            .matchers(vec![
                Matcher::Ip(ip("127.0.0.1")),
                Matcher::IpRange(ipnet("192.168.1.1/24")),
            ])
            .build();

        let handler = AccessControlPolicyHandler::try_from(config).unwrap();
        assert_eq!(
            handler.evaluate(&trusted_ip("1.2.3.4")),
            EvaluationResult::Allowed
        );
        assert_eq!(
            handler.evaluate(&trusted_ip("192.168.1.127")),
            EvaluationResult::Denied
        );
        assert_eq!(
            handler.evaluate(&trusted_ip("127.0.0.1")),
            EvaluationResult::Denied
        );
    }
}
