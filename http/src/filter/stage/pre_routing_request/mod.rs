use crate::filter::stage::inbound_request::InboundRequestFilterHandler;
use derive_more::{Deref, DerefMut, From};

pub mod factory;
mod finalizer;

#[derive(Debug, Deref, DerefMut, From)]
pub struct PreRoutingRequestFilterChain(InboundRequestFilterHandler);
