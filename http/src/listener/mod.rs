use crate::filter::SharedFilterHandlerLayer;
use crate::listener::filter::{ListenerFilterHandlerLayer, ListenerFilterHandlerLayerError};
use crate::listener::policy::{ListenerPolicyHandlers, ListenerPolicyHandlersConversionError};
use crate::route::Route;
use getset::Getters;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::filter::SharedFilterRef;
use vg_config::http::listener::{ListenerProtocol, ListenerRef};

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
    filters: Vec<ListenerFilterHandlerLayer>,

    #[getset(get = "pub")]
    routes: Vec<Arc<Route>>,
}

#[derive(Debug, Error)]
pub enum ListenerConversionError {
    #[error(transparent)]
    Policies(#[from] ListenerPolicyHandlersConversionError),
    #[error("Invalid filter at index {0}: {1}")]
    Filter(usize, ListenerFilterHandlerLayerError),
}

#[derive(TypedBuilder)]
pub struct ListenerConversionContext {
    listener: Arc<vg_config::http::listener::Listener>,
    shared_filters: Arc<HashMap<SharedFilterRef, SharedFilterHandlerLayer>>,
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
                ListenerFilterHandlerLayer::try_from((value.shared_filters.as_ref(), filter))
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
