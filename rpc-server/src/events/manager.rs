use crate::ConfigurationEventSinkRegistry;
use crate::events::handles::{ConfigurationEventPollingHandle, polling_handles};
use crate::events::sinks::{ConfigurationEventSink, ConfigurationEventSinkId};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::select;
use tokio::sync::mpsc::{Receiver, Sender, channel};
use typed_builder::TypedBuilder;
use vg_config::http::listener::ListenerRef;
use vg_rpc::ConfigurationEvent;

#[derive(Debug)]
pub struct ConfigurationEventManager {
    sinks: Arc<DashMap<ConfigurationEventSinkId, ConfigurationEventSink>>,
    event_tx: Sender<(ListenerRef, ConfigurationEvent)>,
    event_rx: Receiver<(ListenerRef, ConfigurationEvent)>,
}

impl ConfigurationEventManager {
    pub fn new() -> Self {
        let (tx, rx) = channel(32);
        Self {
            sinks: Arc::new(DashMap::new()),
            event_tx: tx,
            event_rx: rx,
        }
    }
}

impl ConfigurationEventManager {
    pub async fn start_polling(self) -> ConfigurationEventPollingHandle {
        let (polling_handle, stop_handle) = polling_handles();
        let event_rx = self.event_rx;
        let sinks = self.sinks;
        tokio::spawn(async move {
            let mut event_rx = event_rx;
            let mut stop_handle = stop_handle;
            loop {
                select! {
                    event = event_rx.recv() => {
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

        polling_handle
    }

    pub fn sink_registry(&self) -> ConfigurationEventSinkRegistry {
        ConfigurationEventSinkRegistry::builder()
            .sinks(self.sinks.clone())
            .build()
    }

    pub fn sender(&self) -> ConfigurationEventSender {
        ConfigurationEventSender::builder()
            .tx(self.event_tx.clone())
            .build()
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
