use crate::filter::SharedFilterHandler;
use crate::listener::filter::{ListenerFilterHandler, ListenerFilterHandlerConversionError};
use crate::listener::policy::{ListenerPolicyHandlers, ListenerPolicyHandlersConversionError};
use crate::route::Route;
use getset::{CopyGetters, Getters};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::filter::SharedFilterRef;
use vg_config::http::listener::{ListenerRef, ListenerTransport, ListenerTransportProtocols};
use vg_core::net::Port;

mod filter;
pub mod policy;

#[derive(Debug, TypedBuilder, CopyGetters)]
pub struct Protocols {
    #[getset(get_copy = "pub")]
    http: Option<Port>,
}

impl From<ListenerTransportProtocols> for Protocols {
    fn from(value: ListenerTransportProtocols) -> Self {
        let http_port = if value.http() {
            // Default HTTP port - this should be overridden with actual listener port
            Some(Port::from(std::num::NonZeroU16::new(80).unwrap()))
        } else {
            None
        };
        Self::builder().http(http_port).build()
    }
}

#[derive(Debug, TypedBuilder, Getters)]
pub struct Transport {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    protocols: Protocols,
}

impl From<ListenerTransport> for Transport {
    fn from(value: ListenerTransport) -> Self {
        Self::builder().protocols(value.protocols().clone()).build()
    }
}

#[derive(Debug, TypedBuilder, Getters)]
pub struct Listener {
    #[getset(get_clone = "pub")]
    ref_: ListenerRef,

    #[getset(get_clone = "pub")]
    #[builder(setter(into))]
    transport: Transport,

    #[getset(get = "pub")]
    policies: ListenerPolicyHandlers,

    #[getset(get = "pub")]
    filters: Vec<ListenerFilterHandler>,

    #[getset(get = "pub")]
    routes: Vec<Arc<Route>>,
}

#[derive(Debug, Error)]
pub enum ListenerConversionError {
    #[error(transparent)]
    Policies(#[from] ListenerPolicyHandlersConversionError),
    #[error("Invalid filter at index {0}: {1}")]
    Filter(usize, ListenerFilterHandlerConversionError),
}

#[derive(TypedBuilder)]
pub struct ListenerConversionContext {
    listener: Arc<vg_config::http::listener::Listener>,
    shared_filters: Arc<HashMap<SharedFilterRef, SharedFilterHandler>>,
    routes: Vec<Arc<Route>>,
}

impl TryFrom<ListenerConversionContext> for Listener {
    type Error = ListenerConversionError;

    fn try_from(value: ListenerConversionContext) -> Result<Self, Self::Error> {
        let filters = value
            .listener
            .filters()
            .iter()
            .enumerate()
            .map(|(idx, filter)| {
                ListenerFilterHandler::try_from((value.shared_filters.as_ref(), filter))
                    .map_err(|err| ListenerConversionError::Filter(idx, err))
            })
            .collect::<Result<_, _>>()?;

        let policies: ListenerPolicyHandlers = value.listener.policies().try_into()?;

        // Create transport with the actual listener port
        let transport_protocols = value.listener.transport().protocols().clone();
        let protocols = Protocols::builder()
            .http(if transport_protocols.http() {
                Some(value.listener.port())
            } else {
                None
            })
            .build();
        let transport = Transport::builder().protocols(protocols).build();

        let listener = Listener::builder()
            .ref_(value.listener.ref_())
            .transport(transport)
            .policies(policies)
            .filters(filters)
            .routes(value.routes)
            .build();

        Ok(listener)
    }
}
