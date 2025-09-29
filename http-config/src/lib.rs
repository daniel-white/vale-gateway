use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use vg_core::net;

pub mod filters;
pub mod request;
pub mod routing;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IpRef {
    Addr(IpAddr),
    Net(IpNet),
}

impl From<&IpRef> for net::IpRef {
    fn from(value: &IpRef) -> Self {
        match value {
            IpRef::Addr(addr) => net::IpRef::Addr(*addr),
            IpRef::Net(net) => net::IpRef::Net(*net),
        }
    }
}

impl From<&net::IpRef> for IpRef {
    fn from(value: &net::IpRef) -> Self {
        match value {
            net::IpRef::Addr(addr) => IpRef::Addr(*addr),
            net::IpRef::Net(net) => IpRef::Net(*net),
        }
    }
}
