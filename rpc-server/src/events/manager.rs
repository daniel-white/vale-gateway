use crate::events::sinks::{
    ConfigurationEventSink, ConfigurationEventSinkId, PendingConfigurationEventSink,
};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::select;
use tokio::sync::broadcast::Sender;
use vg_config::http::listener::ListenerRef;
use vg_rpc::{ConfigurationApiError, ConfigurationEvent};

#[derive(Debug, Clone)]
pub struct ConfigurationEventManager {
    sinks: Arc<DashMap<ConfigurationEventSinkId, ConfigurationEventSink>>,
    tx: Sender<(ListenerRef, ConfigurationEvent)>,
}

impl Default for ConfigurationEventManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigurationEventManager {
    pub fn new() -> Self {
        let (tx, _rx) = tokio::sync::broadcast::channel(100);
        Self {
            sinks: Arc::new(DashMap::new()),
            tx,
        }
    }
}

impl ConfigurationEventManager {
    pub(crate) async fn try_subscribe(
        &self,
        pending_sink: PendingConfigurationEventSink,
    ) -> Result<(), ConfigurationApiError> {
        // TODO validate and accept/reject the pending sink

        let sink = pending_sink.accept().await.unwrap();
        let mut rx = self.tx.subscribe();

        let sinks = self.sinks.clone();
        tokio::spawn(async move {
            sinks.insert(sink.connection_id(), sink.clone());

            loop {
                select! {
                    event = rx.recv() => {
                        if sink.is_closed() {
                            break;
                        }

                        match event {
                            Ok((listener_ref, event)) if &listener_ref == sink.listener_ref() => {
                                let _ = sink.send(event).await;
                                // TODO handle send error
                            }
                            Ok(_) => {
                                continue;
                            }
                            Err(_) => {
                                break;
                            }
                        }
                    }
                    _ = sink.closed() => {
                        break;
                    }
                }
            }

            println!("Removing");
            sinks.remove(&sink.connection_id());
        });

        Ok(())
    }

    pub fn send(&self, listener_ref: ListenerRef, event: ConfigurationEvent) {
        if self.tx.receiver_count() > 0 {
            let _ = self.tx.send((listener_ref, event));
        }
    }
}
