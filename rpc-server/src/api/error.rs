use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiServerStartError{
    #[error("TODO")]
    Unknown
}