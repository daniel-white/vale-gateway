use crate::ConfigurationEventSinkRegistry;
use crate::events::handles::{ConfigurationEventServerHandle, handles};
use crate::events::sinks::{ConfigurationEventSink, ConfigurationEventSinkId};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tokio::{select, spawn};
use typed_builder::TypedBuilder;
use vg_config::http::listener::ListenerRef;
use vg_rpc::ConfigurationEvent;

#[derive(Debug)]
pub struct ConfigurationEventServer {
    sinks: Arc<DashMap<ConfigurationEventSinkId, ConfigurationEventSink>>,
    tx: Sender<(ListenerRef, ConfigurationEvent)>,
    rx: Receiver<(ListenerRef, ConfigurationEvent)>,
}

impl Default for ConfigurationEventServer {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigurationEventServer {
    pub fn new() -> Self {
        let (tx, rx) = channel(32);
        Self {
            sinks: Arc::new(DashMap::new()),
            tx,
            rx,
        }
    }

    pub fn sinks(&self) -> ConfigurationEventSinkRegistry {
        ConfigurationEventSinkRegistry::builder()
            .sinks(self.sinks.clone())
            .build()
    }

    pub fn sender(&self) -> ConfigurationEventSender {
        ConfigurationEventSender::builder()
            .tx(self.tx.clone())
            .build()
    }

    pub fn start(self) -> ConfigurationEventServerHandle {
        let (server_handle, stop_handle) = handles();
        let rx = self.rx;
        let sinks = self.sinks;

        spawn(async move {
            let mut rx = rx;
            let mut stop_handle = stop_handle;
            loop {
                select! {
                    event = rx.recv() => {
                        match event {
                            Some((listener_ref, event)) => {
                                let current_sinks: Vec<_> = sinks.iter()
                                    .filter(|entry| &listener_ref == entry.listener_ref() && !entry.is_closed())
                                    .collect();

                                for sink in current_sinks {
                                    let _ = sink.send(event.clone()).await;
                                    // TODO handle error
                                }

                                sinks.retain(|_, sink| !sink.is_closed());
                            },
                            None => break
                        }
                    }
                    _ = stop_handle.stopped() => {
                        break;
                    }
                }
            }
        });

        server_handle
    }
}

#[derive(Debug, Clone, TypedBuilder)]
pub struct ConfigurationEventSender {
    tx: Sender<(ListenerRef, ConfigurationEvent)>,
}

impl ConfigurationEventSender {
    pub async fn send(&self, listener_ref: ListenerRef, event: ConfigurationEvent) {
        if let Err(err) = self.tx.send((listener_ref, event)).await {
            println!("Error sending: {:?}", err)
        }
    }
}
