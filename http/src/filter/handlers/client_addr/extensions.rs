use getset::CopyGetters;
use std::net::{IpAddr, SocketAddr};
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, Copy, TypedBuilder, CopyGetters)]
pub struct RequestSocketAddr {
    #[getset(get_copy = "pub")]
    addr: SocketAddr,
}

#[derive(Debug, Clone, Copy, TypedBuilder, CopyGetters)]
pub struct TrustedClientIpAddr {
    #[getset(get_copy = "pub")]
    ip_addr: IpAddr,
}
