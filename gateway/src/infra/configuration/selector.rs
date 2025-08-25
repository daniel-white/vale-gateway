use crate::infra::configuration::fs::{fs_source, FsSourceParams};
use crate::infra::configuration::ipc::{ipc_source, IpcSourceParams};
use getset::Getters;
use std::sync::Arc;
use tracing::debug;
use typed_builder::TypedBuilder;
use vg_core::continue_on;
use vg_core::gateways::Gateway;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;

#[derive(Getters, Debug, TypedBuilder)]
pub struct GatewayConfigurationParams {
    ipc_source_params: IpcSourceParams,
    fs_source_params: FsSourceParams,
}

pub fn gateway_configuration(
    task_builder: &TaskBuilder,
    params: GatewayConfigurationParams,
) -> Receiver<Arc<Gateway>> {
    let (tx, rx) = signal(stringify!(gateway_configuration));

    let ipc_source_rx = ipc_source(task_builder, params.ipc_source_params);
    let fs_source_rx = fs_source(task_builder, params.fs_source_params);

    task_builder
        .new_task(stringify!(gateway_configuration))
        .spawn(async move {
            loop {
                let gateway = match (
                    ipc_source_rx.get().await.as_ref(),
                    fs_source_rx.get().await.as_ref(),
                ) {
                    (Some((_, ipc)), None) => {
                        debug!("Using IPC configuration");
                        Some(ipc.clone())
                    }
                    (None, Some((_, fs))) => {
                        debug!("Using file-based configuration");
                        Some(fs.clone())
                    }
                    (Some((ipc_serial, ipc)), Some((fs_serial, _))) if fs_serial < ipc_serial => {
                        debug!("Using IPC configuration, newer");
                        Some(ipc.clone())
                    }
                    (_, Some((_, fs))) => {
                        debug!("Using file-based configuration, newer");
                        Some(fs.clone())
                    }
                    _ => {
                        debug!("No configuration available from either source");
                        None
                    }
                };

                tx.replace(gateway).await;

                continue_on!(ipc_source_rx.changed(), fs_source_rx.changed());
            }
        });

    rx
}
