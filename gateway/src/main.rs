mod configuration;

use std::time::Duration;

use async_from::AsyncTryInto;
use http::Uri;
use tokio::task::JoinSet;
use tokio::time::sleep;
use vg_rpc_client::{ConfigurationClient, ConfigurationClientOptions, ConfigurationEvent};
use crate::configuration::{ConfigurationRegistry, ConfigurationRegistrySynchronizer};

#[tokio::main]
async fn main() {
    let c = ConfigurationClientOptions::builder()
        .address(Uri::from_static("ws://localhost:9000"))
        .listener_ref("example_listener".to_string())
        .build();
    let client: ConfigurationClient = c.async_try_into().await.unwrap();

    
    let reg = ConfigurationRegistry::new();
    
    let e = client.event_receiver();
    
    let sync = ConfigurationRegistrySynchronizer::builder().registry(reg.clone()).client(client.clone()).build();

    let mut js = JoinSet::new();
    
    js.spawn(sync.start());
    
    let _ = client.watch_events().await;

    let mut reg_e = reg.subscribe();
    js.spawn(async move {
        loop {
            let l = reg_e.recv().await;
            println!("reg event: {:?}", l);
        }
    });

    js.join_all().await;
}
