use std::net::IpAddr;

pub mod client_ip_extractor;

/// Extension that contains the extracted client IP address
/// This can be shared across filters and handlers via request extensions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientIp(IpAddr);

impl ClientIp {
    /// Create a new ClientIp extension
    pub fn new(ip: IpAddr) -> Self {
        Self(ip)
    }

    /// Get the IP address
    pub fn ip(&self) -> IpAddr {
        self.0
    }
}

impl From<IpAddr> for ClientIp {
    fn from(ip: IpAddr) -> Self {
        Self(ip)
    }
}

impl From<ClientIp> for IpAddr {
    fn from(client_ip: ClientIp) -> Self {
        client_ip.0
    }
}
