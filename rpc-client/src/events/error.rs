use thiserror::Error;
use jsonrpsee::core::client::Error as ClientError;
use vg_rpc::ApiError;
#[derive(Debug, Error)]
pub enum RecvError {
    #[error("Channel is closed")]
    Closed,
    #[error("Channel has lagged")]
    Lagged,
}

#[derive(Debug, Error)]
pub enum EventClientError {
    #[error("Listener not found")]
    NotFound,
    #[error("Request timeout")]
    RequestTimeout,
    #[error("Unknown event")]
    Unknown,
}

impl From<ClientError> for EventClientError {
    fn from(value: ClientError) -> Self {
        match value {
            ClientError::Call(err) => match ApiError::from(err) {
                ApiError::NotFound => EventClientError::NotFound,
                _ => EventClientError::Unknown,
            },
            ClientError::RequestTimeout => EventClientError::RequestTimeout,
            _ => EventClientError::Unknown,
        }
    }
}