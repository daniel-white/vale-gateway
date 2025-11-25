use async_from::async_trait;
use pingora::prelude::{HttpPeer, Session};
use pingora::proxy::{ProxyHttp, http_proxy_service_with_name};
use pingora::server::configuration::ServerConf;
use pingora::services::Service;
use std::sync::Arc;
use typed_builder::TypedBuilder;
use vg_config::http::listener::ListenerRef;
use vg_core::net::Port;
use vg_rpc_client::api::error::ApiClientError::ServiceUnavailable;

#[derive(TypedBuilder)]
pub struct ProxyServiceOptions {
    port: Port,
    listener_ref: ListenerRef,
}

impl From<ProxyServiceOptions> for ProxyService {
    fn from(value: ProxyServiceOptions) -> Self {
        Self::builder()
            .port(value.port)
            .listener_ref(value.listener_ref)
            .build()
    }
}

#[derive(TypedBuilder)]
pub struct ProxyService {
    port: Port,
    listener_ref: ListenerRef,
}

impl From<ProxyService> for Box<dyn Service> {
    fn from(value: ProxyService) -> Self {
        let conf = Arc::new(ServerConf::default());
        let proxy = HttpProxy;
        let mut service = http_proxy_service_with_name(&conf, proxy, value.listener_ref.to_string().as_str());
        service.add_tcp(format!("0.0.0.0:{}", value.port).as_str());
        service.add_tcp(format!("[::]:{}", value.port).as_str());

        Box::from(service)
    }
}

pub struct HttpProxy;
#[async_trait]
impl ProxyHttp for HttpProxy {
    type CTX = ();

    fn new_ctx(&self) -> Self::CTX {
        todo!()
    }

    async fn upstream_peer(&self, session: &mut Session, ctx: &mut Self::CTX) -> pingora::Result<Box<HttpPeer>> {
        todo!()
    }
}
