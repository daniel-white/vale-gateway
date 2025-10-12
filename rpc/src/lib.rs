use derive_more::TryFrom;
use http::StatusCode;
use jsonrpsee::core::SubscriptionResult;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::{ErrorObject, ErrorObjectOwned};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ConfigurationEvent {
    ListenerChanged,
    RouteChanged(RouteRef),
    BackendChanged(BackendRef),
}

#[rpc(client, server)]
pub trait ConfigurationApi {
    #[subscription(name = "subscribeEvents" => "events", item = ConfigurationEvent)]
    async fn events(&self, listener_ref: ListenerRef) -> SubscriptionResult;

    #[method(name = "getListener")]
    async fn listener(&self, listener_ref: ListenerRef) -> Result<Listener, ConfigurationApiError>;

    #[method(name = "getRoute")]
    async fn route(&self, route_ref: RouteRef) -> Result<Route, ConfigurationApiError>;

    #[method(name = "getBackend")]
    async fn backend(&self, backend_ref: BackendRef) -> Result<Backend, ConfigurationApiError>;
}
