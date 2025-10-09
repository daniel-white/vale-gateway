use std::net::SocketAddr;
use std::str::FromStr;
use tokio::task::JoinSet;
use vg_rpc_server::{ConfigurationEvent, ConfigurationEventManager, ConfigurationServerOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut join_set = JoinSet::new();

    let event_manager = ConfigurationEventManager::new();
    let options = ConfigurationServerOptions::builder()
        .binding(SocketAddr::from_str("0.0.0.0:9000").unwrap())
        .event_manager(event_manager.clone())
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
