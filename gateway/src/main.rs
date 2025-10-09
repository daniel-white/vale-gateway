use std::time::Duration;

use async_from::AsyncTryInto;
use http::Uri;
use tokio::task::JoinSet;
use tokio::time::sleep;
use vg_rpc_client::{ConfigurationClient, ConfigurationClientOptions, ConfigurationEvent};

#[tokio::main]
async fn main() {
    let c = ConfigurationClientOptions::builder()
        .address(Uri::from_static("ws://localhost:9000"))
        .listener_ref("example_listener".to_string())
        .build();
    let client: ConfigurationClient = c.async_try_into().await.unwrap();

    let e = client.event_receiver();

    let mut js = JoinSet::new();

    let inner_client = client.clone();
    js.spawn(async move {
        let mut e = e;
        loop {
            match e.recv().await {
                Ok(ConfigurationEvent::ListenerChanged) => {
                    let c = inner_client.listener().await.unwrap();
                    println!("Received event: {:?}", c)
                }
                Ok(event) => println!("Received other event: {:?}", event),
                Err(err) => println!("Error receiving event: {:?}", err),
            }
        }
    });

    let _ = client.watch_events().await;

    js.spawn(async move {
        loop {
            let l = client.listener().await;
            //  println!("listener: {:?}", l.unwrap());
            sleep(Duration::from_secs(5)).await;
        }
    });

    js.join_all().await;
}
