use jsonrpsee::core::client::Error as JsonrpseeClientError;
use thiserror::Error;
use vg_rpc::ApiError;

#[derive(Debug, Clone, Error)]
pub enum ApiClientError {
    #[error("Service unavailable")]
    ServiceUnavailable,
    #[error("Listener not found")]
    NotFound,
    #[error("Request timeout")]
    RequestTimeout,
    #[error("Unknown error")]
    Unknown,
}

impl From<JsonrpseeClientError> for ApiClientError {
    fn from(value: JsonrpseeClientError) -> Self {
        match value {
            JsonrpseeClientError::Call(err) => match ApiError::from(err) {
                ApiError::NotFound => ApiClientError::NotFound,
                _ => ApiClientError::Unknown,
            },
            JsonrpseeClientError::RequestTimeout => ApiClientError::RequestTimeout,
            _ => ApiClientError::Unknown,
        }
    }
}