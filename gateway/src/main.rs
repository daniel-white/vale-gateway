mod cli;
mod http;
mod infra;
mod instrumentation;

use crate::cli::Cli;
use crate::http::filters::{HttpFilterHandlers, HttpFilterHandlersDependencies};
use crate::http::listener::controllers::{
    http_listener, http_listener_filter_handlers, http_listener_routes,
};
use crate::http::proxy::HttpProxy;
use crate::http::router::http_router;
use crate::infra::configuration::{
    gateway_configuration, GatewayConfigurationParams, IpcSourceParams,
};
use crate::infra::ipc::{ipc_addr, poll_gateway_events, PollGatewayEventsParams};
use crate::infra::{InstanceContext, TopologyLocation};
use clap::Parser;
use pingora::prelude::http_proxy_service;
use pingora::server::Server;
use reqwest_middleware::ClientBuilder;
use reqwest_tracing::TracingMiddleware;
use std::sync::Arc;
use vg_core::crypto::init_crypto;
use vg_core::instrumentation::init_instrumentation;
use vg_core::sync::signal::signal;
use vg_core::task::Builder as TaskBuilder;

#[tokio::main]
async fn main() {
    let task_builder = TaskBuilder::default();

    init_crypto();
    init_instrumentation(&task_builder, "vg-gateway");

    let client = reqwest::ClientBuilder::new()
        .build()
        .expect("Failed to create HTTP client");

    let client = Arc::new(
        ClientBuilder::new(client)
            .with(TracingMiddleware::default())
            .build(),
    );

    let args = Cli::parse();

    let location = {
        let zone = args.zone_name().filter(|z| !z.is_empty());
        let node = args.node_name().filter(|n| !n.is_empty());

        let location = TopologyLocation::builder().zone(zone).node(node).build();
        Arc::new(location)
    };

    let instance_context = {
        let instance_context = InstanceContext::builder()
            .pod_name(args.pod_name())
            .gateway_namespace(args.pod_namespace())
            .gateway_name(args.gateway_name())
            .location(location.clone())
            .build();

        Arc::new(instance_context)
    };

    let (ipc_endpoint_tx, ipc_endpoint_rx) = signal("ipc_endpoint");

    let gateway_events_tx = {
        let params = PollGatewayEventsParams::builder()
            .client(client.clone())
            .ipc_endpoint_rx(ipc_endpoint_rx.clone())
            .instance_context(instance_context.clone())
            .build();

        poll_gateway_events(&task_builder, params)
    };

    let gateway_rx = {
        let ipc_source_params = IpcSourceParams::builder()
            .client(client.clone())
            .ipc_endpoint_rx(ipc_endpoint_rx.clone())
            .gateway_events_rx(gateway_events_tx.subscribe())
            .instance_context(instance_context.clone())
            .build();

        let fs_source_params = crate::infra::configuration::FsSourceParams::builder()
            .file_path(args.config_file_path())
            .build();

        let gateway_configuration_params = GatewayConfigurationParams::builder()
            .ipc_source_params(ipc_source_params)
            .fs_source_params(fs_source_params)
            .build();

        gateway_configuration(&task_builder, gateway_configuration_params)
    };

    ipc_addr(&task_builder, &gateway_rx, ipc_endpoint_tx);

    let http_listener_rx = http_listener(&task_builder, &gateway_rx);

    let http_listener_routes_rx = http_listener_routes(&task_builder, &http_listener_rx);

    let http_filter_handlers = {
        let dependencies = HttpFilterHandlersDependencies::builder()
            .client(client)
            .http_listener_rx(http_listener_rx.clone())
            .ipc_endpoint(ipc_endpoint_rx)
            .instance_context(instance_context)
            .build();
        HttpFilterHandlers::new(&task_builder, dependencies)
    };

    let http_listener_filter_handlers_rx =
        http_listener_filter_handlers(&task_builder, &http_listener_rx, &http_filter_handlers);

    let http_router_rx = http_router(
        &task_builder,
        &http_listener_routes_rx,
        &http_listener_filter_handlers_rx,
        &http_filter_handlers,
        location.clone(),
    );

    task_builder.new_task("server").spawn_blocking(move || {
        let mut server = Server::new(None).unwrap();
        server.bootstrap();
        let proxy = HttpProxy::builder().http_router_rx(http_router_rx).build();
        let mut service = http_proxy_service(&server.configuration, proxy);

        service.add_tcp("0.0.0.0:8080");

        server.add_service(service);

        server.run_forever();
    });

    task_builder.join_all().await;
}
