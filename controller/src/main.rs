use async_trait::async_trait;
use std::net::SocketAddr;
use std::str::FromStr;
use tokio::task::JoinSet;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::listener::{Listener, ListenerRef};
use vg_config::http::listener::policy::ListenerPolicies;
use vg_config::http::provider::HttpConfigurationProvider;
use vg_config::http::route::{Route, RouteRef};
use vg_rpc_server::{ConfigurationEvent, ConfigurationEventManager, ConfigurationServerOptions};

pub struct HttpConfigProvider;

#[async_trait]
impl HttpConfigurationProvider for HttpConfigProvider {
    async fn listener(&self, listener_ref: ListenerRef) -> Option<Listener> {
        let l = Listener::builder()
            .ref_(listener_ref)
            .policies(ListenerPolicies::default())
            .build();
        
        Some(l)
    }

    async fn route(&self, route_ref: RouteRef) -> Option<Route> {
        None
    }

    async fn backend(&self, backend_ref: BackendRef) -> Option<Backend> {
        None
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut join_set = JoinSet::new();

    let http_config = Box::from(HttpConfigProvider);
    let event_manager = ConfigurationEventManager::new();
    let options = ConfigurationServerOptions::builder()
        .binding(SocketAddr::from_str("0.0.0.0:9000").unwrap())
        .event_manager(event_manager.clone())
        .http_configuration(http_config)
        .build();

    let server_handle = options.start_server().await?;

    join_set.spawn(server_handle.stopped());

    join_set.spawn(async move {
        loop {
            event_manager.send(
                "example_listener".to_string().into(),
                ConfigurationEvent::ListenerChanged,
            );
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });

    join_set.join_all().await;

    Ok(())
}
