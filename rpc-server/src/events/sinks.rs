use getset::Getters;
use jsonrpsee_core::server::{
    ConnectionId, PendingSubscriptionSink, SubscriptionMessage, SubscriptionSink,
};
use typed_builder::TypedBuilder;
use vg_config::http::listener::ListenerRef;
use vg_rpc::{ConfigurationApiError, ConfigurationEvent};

#[derive(Debug, TypedBuilder)]
pub struct PendingConfigurationEventSink {
    listener_ref: ListenerRef,
    sink: PendingSubscriptionSink,
}

impl PendingConfigurationEventSink {
    pub fn connection_id(&self) -> ConnectionId {
        self.sink.connection_id()
    }

    pub async fn accept(self) -> Result<ConfigurationEventSink, ()> {
        match self.sink.accept().await {
            Ok(sink) => Ok(ConfigurationEventSink::builder()
                .listener_ref(self.listener_ref)
                .sink(sink)
                .build()),
            Err(_) => Err(()), // TODO: handle error
        }
    }

    pub async fn reject(self, error: ConfigurationApiError) -> Result<(), ConfigurationApiError> {
        self.sink.reject(error).await;
        Ok(())
    }
}

#[derive(Debug, Clone, TypedBuilder, Getters)]
pub struct ConfigurationEventSink {
    #[getset(get = "pub")]
    listener_ref: ListenerRef,
    sink: SubscriptionSink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConfigurationEventSinkId(ConnectionId);

impl ConfigurationEventSink {
    pub fn connection_id(&self) -> ConfigurationEventSinkId {
        ConfigurationEventSinkId(self.sink.connection_id())
    }

    pub fn is_closed(&self) -> bool {
        self.sink.is_closed()
    }

    pub async fn send(&self, event: ConfigurationEvent) -> Result<(), ()> {
        // TODO handle serialization error
        let message =
            SubscriptionMessage::new(self.sink.method_name(), self.sink.subscription_id(), &event)
                .unwrap();
        self.sink.send(message).await.map_err(|_| ())
    }

    pub async fn closed(&self) {
        self.sink.closed().await
    }
}
