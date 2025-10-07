use jsonrpsee::server::Server;
use vg_rpc::api::RpcApiServer;
use crate::rpc::RpcApiServerImpl;

mod rpc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = Server::builder().build("127.0.0.1:9000").await?;

    let addr = server.local_addr()?;
    let handle = server.start(RpcApiServerImpl::default().into_rpc());

    tokio::spawn(handle.stopped()).await?;
    
    Ok(())
}
