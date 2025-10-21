use thiserror::Error;

#[derive(Debug, Error)]
pub enum RecvError {
    #[error("Channel is closed")]
    Closed,
    #[error("Channel has lagged")]
    Lagged,
}