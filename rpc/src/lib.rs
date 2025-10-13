pub mod propagation;

use derive_more::{Deref, DerefMut, TryFrom};
use getset::{CloneGetters, Getters};
use http::{HeaderMap, StatusCode};
use jsonrpsee::core::SubscriptionResult;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::{ErrorObject, ErrorObjectOwned};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::listener::{Listener, ListenerRef};
use vg_config::http::route::{Route, RouteRef};

#[derive(FromPrimitive, Debug, Serialize, Deserialize, TryFrom, Clone, Copy)]
#[repr(u16)]
pub enum ConfigurationApiError {
    NotFound = StatusCode::NOT_FOUND.as_u16(),
    Unknown = StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
}

impl From<ConfigurationApiError> for ErrorObject<'static> {
    fn from(val: ConfigurationApiError) -> Self {
        let code = val as i32;
        match val {
            ConfigurationApiError::NotFound => ErrorObject::borrowed(code, "Not Found", None::<_>),
            ConfigurationApiError::Unknown => {
                ErrorObject::borrowed(code, "Unknown Error", None::<_>)
            }
        }
    }
}

impl From<ErrorObjectOwned> for ConfigurationApiError {
    fn from(val: ErrorObjectOwned) -> Self {
        ConfigurationApiError::from_i32(val.code()).unwrap_or(ConfigurationApiError::Unknown)
    }
}

#[derive(Default, Debug, Serialize, Deserialize, Clone, Deref, DerefMut)]
#[serde(transparent)]
pub struct PropagationChannel(#[serde(with = "http_serde_ext::header_map")]HeaderMap);

#[derive(Default, Debug, Serialize, Deserialize, Getters, Clone, TypedBuilder)]
pub struct Context {
    #[getset(get = "pub")]
    propagation_channel: PropagationChannel
}

impl AsRef<Context> for  Context {
    fn as_ref(&self) -> &Context {
        &self
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ConfigurationEvent {
    ListenerChanged,
    RouteChanged(RouteRef),
    BackendChanged(BackendRef),
}

#[derive(Debug, Serialize, Deserialize, Getters, CloneGetters, Clone, TypedBuilder)]
pub struct EventMessage {
    #[getset(get = "pub")]
    context: Context,
    #[getset(get_clone = "pub")]
    event: ConfigurationEvent
}

#[derive(Debug, Serialize, Deserialize, Getters, CloneGetters, Clone, TypedBuilder)]
pub struct SubscribeEventsRequest {
    #[getset(get = "pub")]
    context: Context,
    #[getset(get_clone = "pub")]
    listener_ref: ListenerRef
}

#[derive(Debug, Serialize, Deserialize, Getters, CloneGetters, Clone, TypedBuilder)]
pub struct GetListenerRequest {
    #[getset(get = "pub")]
    context: Context,
    #[getset(get_clone = "pub")]
    listener_ref: ListenerRef
}

#[derive(Debug, Serialize, Deserialize, Getters, CloneGetters, Clone, TypedBuilder)]
pub struct GetRouteRequest {
    #[getset(get = "pub")]
    context: Context,
    #[getset(get_clone = "pub")]
    route_ref: RouteRef
}

#[derive(Debug, Serialize, Deserialize, Getters,CloneGetters, Clone, TypedBuilder)]
pub struct GetBackendRequest {
    #[getset(get = "pub")]
    context: Context,
    #[getset(get_clone = "pub")]
    backend_ref: BackendRef
}


#[rpc(client, server)]
pub trait ConfigurationApi {
    #[subscription(name = "subscribeEvents" => "events", item = EventMessage)]
    async fn events(&self, req: SubscribeEventsRequest) -> SubscriptionResult;

    #[method(name = "getListener")]
    async fn listener(&self, req: GetListenerRequest) -> Result<Listener, ConfigurationApiError>;

    #[method(name = "getRoute")]
    async fn route(&self, req: GetRouteRequest) -> Result<Route, ConfigurationApiError>;

    #[method(name = "getBackend")]
    async fn backend(&self, req: GetBackendRequest) -> Result<Backend, ConfigurationApiError>;
}
