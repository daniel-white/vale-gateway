use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use vg_core::gateways::Gateway;
use vg_core::http::listeners::HttpListener;
use vg_core::{await_ready, continue_on, ReadyState};
use vg_core::ipc::IpcConfiguration;
use vg_core::sync::signal::Receiver;
use vg_core::task::Builder as TaskBuilder;
use crate::gateways::collectors::Gateways;
use crate::kubernetes::objects::ObjectRef;

pub fn gateway_configurations(task_builder: &TaskBuilder, gateways_rx: &Receiver<Arc<Gateways>>, ipc_addr_rx: &Receiver<IpAddr>, http_listeners_rx: &Receiver<HashMap<ObjectRef, Arc<HttpListener>>>) -> Receiver<Vec<(ObjectRef, Arc<Gateway>)>> {
    let (tx, rx) = vg_core::sync::signal::signal(stringify!(gateway_configurations));
    let ipc_addr_rx = ipc_addr_rx.clone();
    let gateways_rx = gateways_rx.clone();
    let http_listeners_rx = http_listeners_rx.clone();
    
    task_builder.new_task(stringify!(gateway_configurations))
        .spawn(async move {
            loop {
                if let ReadyState::Ready((ipc_addr, gateways, http_listeners)) = await_ready!(ipc_addr_rx, gateways_rx, http_listeners_rx) {
                    let ipc_configuration = IpcConfiguration::builder()
                        .addr(SocketAddr::new(*ipc_addr, 0))
                        .build();
                    let ipc_configuration = Arc::new(ipc_configuration);
                    
                    let gateway_configurations = gateways.iter()
                        .map(|(gateway_ref, _, _, _, _)| {
                            let http_listener = http_listeners.get(gateway_ref).cloned();
                            
                            let gateway = Gateway::builder()
                                .ipc(ipc_configuration.clone())
                                .http_listener(http_listener)
                                .build();
                            
                            (gateway_ref.clone(), Arc::new(gateway))
                        })
                        .collect();
                    
                    tx.set(gateway_configurations).await;
                }
                
                continue_on!(gateways_rx.changed(), http_listeners_rx.changed());
            }
        });
    
    
    rx
}