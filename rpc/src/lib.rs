use derive_more::{Deref, DerefMut, TryFrom};
use getset::{CloneGetters, Getters};
use http::{HeaderMap, StatusCode};
use jsonrpsee::core::SubscriptionResult;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::{ErrorObject, ErrorObjectOwned};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use opentelemetry::global::{BoxedSpan, get_text_map_propagator};
use opentelemetry::trace::TraceContextExt;
use opentelemetry_http::{HeaderExtractor, HeaderInjector};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::filter::{SharedFilter, SharedFilterRef};
use vg_config::http::gateway::{Gateway, GatewayRef};
use vg_config::http::route::{Route, RouteRef};

#[derive(FromPrimitive, Debug, Serialize, Deserialize, TryFrom, Clone, Copy)]
#[repr(u16)]
pub enum ApiError {
    NotFound = StatusCode::NOT_FOUND.as_u16(),
    Unknown = StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
}

impl From<ApiError> for ErrorObject<'static> {
    fn from(val: ApiError) -> Self {
        let code = val as i32;
        match val {
            ApiError::NotFound => ErrorObject::borrowed(code, "Not Found", None::<_>),
            ApiError::Unknown => ErrorObject::borrowed(code, "Unknown Error", None::<_>),
        }
    }
}

impl From<ErrorObjectOwned> for ApiError {
    fn from(val: ErrorObjectOwned) -> Self {
        ApiError::from_i32(val.code()).unwrap_or(ApiError::Unknown)
    }
}

#[derive(Default, Debug, Serialize, Deserialize, Clone, Deref, DerefMut)]
#[serde(transparent)]
pub struct PropagationChannel(#[serde(with = "http_serde_ext::header_map")] HeaderMap);

impl PropagationChannel {
    #[must_use] 
    pub fn current() -> Self {
        get_text_map_propagator(|propagator| {
            let mut headers = HeaderMap::default();
            let mut injector = HeaderInjector(&mut headers);
            propagator.inject(&mut injector);
            Self(headers)
        })
    }
}

impl From<&PropagationChannel> for opentelemetry::Context {
    fn from(value: &PropagationChannel) -> Self {
        get_text_map_propagator(|propagator| {
            let extractor = HeaderExtractor(value);
            propagator.extract(&extractor)
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Getters, Clone, TypedBuilder)]
pub struct RequestContext {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "HeaderMap::is_empty")]
    propagation_channel: PropagationChannel,
}

impl RequestContext {
    #[must_use] 
    pub fn new(span: BoxedSpan) -> Self {
        let guard = opentelemetry::Context::new().with_span(span).attach();
        Self::builder()
            .propagation_channel(PropagationChannel::current())
            .build()
    }
}

impl AsRef<RequestContext> for RequestContext {
    fn as_ref(&self) -> &RequestContext {
        self
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Event {
    Initialize,
    GatewayChanged,
    RouteChanged(RouteRef),
    BackendChanged(BackendRef),
    SharedFilterChanged(SharedFilterRef),
}

#[derive(Debug, Serialize, Deserialize, Getters, CloneGetters, Clone, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct EventMessage {
    #[getset(get = "pub")]
    context: RequestContext,
    #[getset(get_clone = "pub")]
    event: Event,
}

#[derive(Debug, Serialize, Deserialize, Getters, CloneGetters, Clone, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeEventsRequest {
    #[getset(get_clone = "pub")]
    gateway_ref: GatewayRef,
}

#[derive(Debug, Serialize, Deserialize, Getters, CloneGetters, Clone, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct GetRouteRequest {
    #[getset(get = "pub")]
    route_ref: RouteRef,
}

#[derive(Debug, Serialize, Deserialize, Getters, CloneGetters, Clone, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct GetBackendRequest {
    #[getset(get = "pub")]
    backend_ref: BackendRef,
}

#[derive(Debug, Serialize, Deserialize, Getters, CloneGetters, Clone, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct GetSharedFilterRequest {
    #[getset(get = "pub")]
    filter_ref: SharedFilterRef,
}

#[derive(Debug, Serialize, Deserialize, Getters, CloneGetters, Clone, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct GetGatewayRequest {
    #[getset(get_clone = "pub")]
    gateway_ref: vg_config::http::gateway::GatewayRef,
}

#[derive(Debug, Serialize, Deserialize, Getters, CloneGetters, Clone, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct GetGatewayResponse {
    #[getset(get_clone = "pub")]
    gateway: Gateway,
}

#[rpc(client, server)]
pub trait Api {
    #[subscription(name = "subscribeEvents" => "events", item = EventMessage)]
    async fn events(&self, req: SubscribeEventsRequest) -> SubscriptionResult;

    #[method(name = "getRoute")]
    async fn route(&self, req: GetRouteRequest) -> Result<Route, ApiError>;

    #[method(name = "getBackend")]
    async fn backend(&self, req: GetBackendRequest) -> Result<Backend, ApiError>;

    #[method(name = "getSharedFilter")]
    async fn shared_filter(&self, req: GetSharedFilterRequest) -> Result<SharedFilter, ApiError>;

    #[method(name = "getGateway")]
    async fn gateway(&self, req: GetGatewayRequest) -> Result<GetGatewayResponse, ApiError>;
}
