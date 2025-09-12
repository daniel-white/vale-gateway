#![warn(
    clippy::pedantic,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unwrap_used,
    clippy::expect_used
)]
#![allow(
    clippy::needless_pass_by_value,
    clippy::needless_continue,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::struct_field_names
)]

mod cli;
//mod controllers;

//mod backends;
pub mod gateways;
mod health;
pub mod http;
mod instrumentation;
pub mod ipc;
pub mod kubernetes;
mod options;
mod services;

//use crate::controllers::{spawn_controllers, SpawnControllersParams, StaticResponsesCache};
use crate::ipc::SpawnIpcError;
use crate::kubernetes::start_kubernetes_client;
use crate::options::Options;
use clap::Parser;
use cli::Cli;
use std::sync::Arc;
use thiserror::Error;
use tracing::error;
use vg_core::crypto::init_crypto;
use vg_core::instrumentation::init_instrumentation;
use vg_core::task::Builder as TaskBuilder;

#[derive(Debug, Error)]
pub enum MainError {
    #[error("Failed to build IPC parameters")]
    SpawnIpcParameters,
    #[error("Failed to spawn IPC services: {0}")]
    SpawnIpc(#[from] SpawnIpcError),
    #[error("Failed to build controllers parameters")]
    SpawnControllersParams,
}

#[tokio::main(flavor = "multi_thread")]
#[allow(clippy::expect_used)] // Expect is used here to ensure that the application fails fast if the parameters are invalid
async fn main() -> Result<(), MainError> {
    let task_builder = TaskBuilder::default();
    let args = Cli::parse();
    let options = Arc::new(Options::default());

    init_crypto();
    init_instrumentation(&task_builder, "vg-control-plane");

    let kube_client_rx = start_kubernetes_client(&task_builder);

    // Start IPC services for gateway communication
    let ipc_services = {
        use crate::http::filters::static_response::cache::StaticResponsesCache;
        use crate::ipc::{SpawnIpcParameters, spawn_ipc};

        let static_responses_cache = StaticResponsesCache::default();
        let params = SpawnIpcParameters::builder()
            .options(options.clone())
            .port(args.port())
            .kube_client_rx(kube_client_rx.clone())
            .static_responses_cache(static_responses_cache)
            .build();

        spawn_ipc(&task_builder, params)
            .await
            .inspect_err(|err| error!("Failed to spawn IPC services: {}", err))?
    };

    // Create IPC configuration for gateway instances
    let ipc_config = Arc::new(
        vg_core::ipc::IpcConfiguration::builder()
            .addr(std::net::SocketAddr::from((
                [127, 0, 0, 1],
                u16::from(ipc_services.port()),
            )))
            .build(),
    );

    // Start the gateway orchestrator - handles configuration merging and resource management
    crate::gateways::GatewayOrchestrator::spawn_all(
        &task_builder,
        options,
        &kube_client_rx,
        ipc_config,
        Arc::new(ipc_services),
    );

    task_builder.join_all().await;

    Ok(())
}
