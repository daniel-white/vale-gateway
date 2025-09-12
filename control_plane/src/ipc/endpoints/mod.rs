mod get_gateway_configuration;
mod get_gateway_events;
mod get_static_response;
mod liveness_check;

use self::get_gateway_configuration::get_gateway_configuration;
use self::get_gateway_events::get_gateway_events;
use crate::health::KubernetesApiHealthIndicator;
use crate::http::filters::static_response::cache::StaticResponsesCache;
use crate::ipc::endpoints::get_static_response::get_static_response;
use crate::ipc::endpoints::liveness_check::liveness_check;
use crate::ipc::events::EventStreamFactory;
use crate::ipc::gateways::GatewayConfigurationReader;
use crate::kubernetes::KubeClientCell;
use crate::options::Options;
use axum::Router;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum_health::Health;
use axum_otel_metrics::HttpMetricsLayerBuilder;
use axum_tracing_opentelemetry::middleware::{OtelAxumLayer, OtelInResponseLayer};
use getset::{CloneGetters, CopyGetters, Getters};
use problemdetails::Problem;
use std::net::SocketAddr;
use std::sync::Arc;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::select;
use tracing::{error, info};
use typed_builder::TypedBuilder;
use vg_core::instrumentation::trace_id;
use vg_core::net::Port;
use vg_core::sync::signal::Receiver;
use vg_core::task::Builder as TaskBuilder;

#[derive(TypedBuilder, Getters, CloneGetters, Clone)]
pub struct IpcEndpointState {
    #[getset(get_clone = "pub")]
    options: Arc<Options>,

    #[getset(get = "pub")]
    events: EventStreamFactory,

    #[getset(get = "pub")]
    gateways: GatewayConfigurationReader,

    #[getset(get_clone = "pub")]
    static_responses_cache: StaticResponsesCache,
}

#[derive(TypedBuilder, CloneGetters, CopyGetters)]
pub struct SpawnIpcEndpointParameters {
    #[getset(get_clone = "")]
    options: Arc<Options>,

    #[getset(get_copy = "")]
    port: Port,

    #[getset(get_clone = "")]
    events: EventStreamFactory,

    #[getset(get_clone = "")]
    gateways: GatewayConfigurationReader,

    #[getset(get_clone = "")]
    kube_client_rx: Receiver<KubeClientCell>,

    #[getset(get_clone = "")]
    static_responses_cache: StaticResponsesCache,
}

impl SpawnIpcEndpointParameters {
    fn endpoint(&self) -> SocketAddr {
        SocketAddr::from(([0, 0, 0, 0], self.port().into()))
    }
}

#[derive(Debug, Error)]
pub enum SpawnIpcEndpointError {
    #[error("Failed to create initial IPC endpoint state")]
    InitialState,

    #[error("Failed to bind IPC endpoint: {0}")]
    NetworkBind(#[from] std::io::Error),
}

pub async fn spawn_ipc_endpoint(
    task_builder: &TaskBuilder,
    params: SpawnIpcEndpointParameters,
) -> Result<(), SpawnIpcEndpointError> {
    let initial_state = IpcEndpointState::builder()
        .options(params.options())
        .gateways(params.gateways())
        .events(params.events())
        .static_responses_cache(params.static_responses_cache())
        .build();

    let kube_health = KubernetesApiHealthIndicator::new(&params.kube_client_rx);
    let health = Health::builder().with_indicator(kube_health).build();

    let endpoint = params.endpoint();
    let tcp_listener = TcpListener::bind(endpoint).await?;

    info!("IPC HTTP service starting on {}", endpoint);

    task_builder.new_task("ipc_endpoint").spawn(async move {
        let app = router(initial_state, health);

        select! {
            result = axum::serve(tcp_listener, app) => {
                match result {
                    Ok(_) => info!("IPC HTTP service stopped gracefully"),
                    Err(e) => error!("IPC HTTP service error: {}", e),
                }
            },
            _ = tokio::signal::ctrl_c() => {
                info!("Received shutdown signal, stopping IPC HTTP service");
            }
        }
    });

    info!("IPC HTTP service spawned successfully on {}", endpoint);
    Ok(())
}

fn router(state: IpcEndpointState, health: Health) -> Router {
    Router::new()
        // Health check endpoints
        .route("/healthz/liveness", get(liveness_check))
        .route("/healthz/readiness", get(axum_health::health))
        
        // IPC endpoints for gateway communication
        .route(
            "/ipc/namespaces/{gateway_namespace}/gateways/{gateway_name}/configuration",
            get(get_gateway_configuration),
        )
        .route(
            "/ipc/namespaces/{gateway_namespace}/gateways/{gateway_name}/events",
            get(get_gateway_events),
        )
        .route(
            "/ipc/namespaces/{gateway_namespace}/gateways/{gateway_name}/static_responses/{static_response_filter_id}",
            get(get_static_response),
        )
        
        // API info endpoint
        .route("/api/info", get(api_info))
        
        // Fallback handlers
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        
        // State and middleware
        .with_state(state)
        .layer(HttpMetricsLayerBuilder::default().build())
        .layer(OtelAxumLayer::default())
        .layer(OtelInResponseLayer)
        .layer(health)
}

async fn not_found() -> impl IntoResponse {
    let mut problem = Problem::from(StatusCode::NOT_FOUND)
        .with_value("status", StatusCode::NOT_FOUND.as_u16())
        .with_title("Not Found")
        .with_detail("The requested resource could not be found");

    if let Some(trace_id) = trace_id() {
        problem = problem.with_instance(trace_id);
    }

    problem
}

async fn method_not_allowed() -> impl IntoResponse {
    let mut problem = Problem::from(StatusCode::METHOD_NOT_ALLOWED)
        .with_value("status", StatusCode::METHOD_NOT_ALLOWED.as_u16())
        .with_title("Method Not Allowed")
        .with_detail("The requested method is not allowed for this resource");

    if let Some(trace_id) = trace_id() {
        problem = problem.with_instance(trace_id);
    }

    problem
}

async fn api_info() -> impl IntoResponse {
    use axum::Json;
    use serde_json::json;
    
    Json(json!({
        "service": "vale-gateway-control-plane",
        "version": env!("CARGO_PKG_VERSION"),
        "endpoints": {
            "health": {
                "liveness": "/healthz/liveness",
                "readiness": "/healthz/readiness"
            },
            "ipc": {
                "configuration": "/ipc/namespaces/{namespace}/gateways/{name}/configuration",
                "events": "/ipc/namespaces/{namespace}/gateways/{name}/events",
                "static_responses": "/ipc/namespaces/{namespace}/gateways/{name}/static_responses/{filter_id}"
            }
        }
    }))
}
