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
use vg_config::http::listener::{ListenerRef, ListenerProtocol};
use vg_core::net::Port;

mod filter;
pub mod policy;


#[derive(Debug, TypedBuilder, Getters)]
pub struct Listener {
    #[getset(get_clone = "pub")]
    ref_: ListenerRef,

    #[getset(get_clone = "pub")]
    protocol: ListenerProtocol,

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
        
        let listener = Listener::builder()
            .ref_(value.listener.ref_())
            .protocol(value.listener.protocol().clone())
            .policies(policies)
            .filters(filters)
            .routes(value.routes)
            .build();

        Ok(listener)
    }
}
