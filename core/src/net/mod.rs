use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use std::fmt::Display;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::num::NonZeroU16;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Port(NonZeroU16);

impl Port {
    // Create a new Port from a u16, returning None if the value is zero (invalid NonZeroU16)
    pub fn new(port: u16) -> Option<Self> {
        NonZeroU16::new(port).map(Self)
    }
    // Access underlying numeric value
    pub fn get(self) -> u16 {
        self.0.get()
    }
}

impl Display for Port {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IpRef {
    Addr(IpAddr),
    Net(IpNet),
}

impl From<IpRef> for String {
    fn from(value: IpRef) -> Self {
        match value {
            IpRef::Addr(addr) => addr.to_string(),
            IpRef::Net(net) => net.to_string(),
        }
    }
}

impl From<IpAddr> for IpRef {
    fn from(value: IpAddr) -> Self {
        Self::Addr(value)
    }
}

impl From<Ipv4Addr> for IpRef {
    fn from(value: Ipv4Addr) -> Self {
        Self::Addr(IpAddr::V4(value))
    }
}

impl From<Ipv6Addr> for IpRef {
    fn from(value: Ipv6Addr) -> Self {
        Self::Addr(IpAddr::V6(value))
    }
}

impl From<IpNet> for IpRef {
    fn from(value: IpNet) -> Self {
        Self::Net(value)
    }
}

impl From<Ipv4Net> for IpRef {
    fn from(value: Ipv4Net) -> Self {
        Self::Net(IpNet::V4(value))
    }
}

impl From<Ipv6Net> for IpRef {
    fn from(value: Ipv6Net) -> Self {
        Self::Net(IpNet::V6(value))
    }
}

#[derive(Debug, Error)]
#[error("Failed to parse IP reference")]
pub struct ParseIpRefError;

impl FromStr for IpRef {
    type Err = ParseIpRefError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(addr) = s.parse::<IpAddr>() {
            Ok(Self::Addr(addr))
        } else if let Ok(range) = s.parse::<IpNet>() {
            Ok(Self::Net(range))
        } else {
            Err(ParseIpRefError)
        }
    }
}
