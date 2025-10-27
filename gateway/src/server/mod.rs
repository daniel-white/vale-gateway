use std::cell::RefCell;
use async_from::async_trait;
use pingora::prelude::Opt;
use pingora::server::{RunArgs, ShutdownSignal, ShutdownSignalWatch, UnixShutdownSignalWatch};
use pingora::services::Service;
use thiserror::Error;
use tokio::select;
use tokio::sync::RwLock;
use tokio::task::spawn_blocking;
use typed_builder::TypedBuilder;
use vg_core::sync::handles::{handles, Handle, StopHandle};

#[derive(TypedBuilder)]
pub struct ServerOptions {
    services: Vec<Box<dyn Service>>
}

impl From<ServerOptions> for Server {
    fn from(value: ServerOptions) -> Self {
        Server::builder()
            .services(value.services)
            .build()
    }
}

#[derive(TypedBuilder)]
pub struct Server {
    services: Vec<Box<dyn Service>>
}

impl Server {
    pub fn start(self) -> Result<Handle, ServerError> {
        let mut server = pingora::server::Server::new(None)?;
        
        let (handle, stop_handle) = handles();
        let shutdown_signal = StopHandleShutdownSignalWatch::new(stop_handle);
        let shutdown_signal = Box::new(shutdown_signal);

        server.add_services(self.services);
        
        spawn_blocking(move ||{
            let args = RunArgs {
                shutdown_signal: Box::new(UnixShutdownSignalWatch), // TODO
            };
            server.run(args);
        });
        
        Ok(handle)
    }
    
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("Pingora error: {0}")]
    PingoraError(#[from]#[source]Box<pingora::Error>)
}

struct StopHandleShutdownSignalWatch(RwLock<StopHandle>);

impl StopHandleShutdownSignalWatch {
    fn new(stop_handle: StopHandle) -> Self {
        Self(RwLock::new(stop_handle))
    }
}

#[async_trait]
impl ShutdownSignalWatch for StopHandleShutdownSignalWatch {
    async fn recv(&self) -> ShutdownSignal {
        let mut stop_handle = self.0.write().await;
        let signal_watch = UnixShutdownSignalWatch;
        select! {
            _ = stop_handle.stopped() => ShutdownSignal::GracefulTerminate,
            result = signal_watch.recv() => result
        }
    }
}