use crate::kubernetes::KubeClientCell;
use async_trait::async_trait;
use axum_health::{HealthDetail, HealthIndicator};
use getset::Getters;
use kube::api::ListParams;
use kube::Api;
use tracing::{info_span, Instrument};
use typed_builder::TypedBuilder;
use vg_api::v1alpha1::GatewayClassParameters;
use vg_core::sync::signal::Receiver;

#[derive(TypedBuilder, Getters)]
pub struct KubernetesApiHealthIndicator {
    #[getset(get = "pub")]
    kube_client_rx: Receiver<KubeClientCell>,
}

impl KubernetesApiHealthIndicator {
    pub fn new(kube_client_rx: &Receiver<KubeClientCell>) -> Self {
        Self::builder()
            .kube_client_rx(kube_client_rx.clone())
            .build()
    }
}

#[async_trait]
impl HealthIndicator for KubernetesApiHealthIndicator {
    fn name(&self) -> String {
        "KubernetesAPI".to_string()
    }

    async fn details(&self) -> HealthDetail {
        if let Some(kube_client) = self.kube_client_rx().get().await.as_deref().cloned() {
            let api = Api::<GatewayClassParameters>::all(kube_client);
            match api
                .list(&ListParams::default())
                .instrument(info_span!("list_gateway_class_parameters"))
                .await
            {
                Ok(_) => HealthDetail::up(),
                Err(e) => {
                    let mut health = HealthDetail::down();
                    health.with_detail("error".to_string(), e.to_string());
                    health
                }
            }
        } else {
            let mut health = HealthDetail::down();
            health.with_detail("error".to_string(), "Kube client not available".to_string());
            health
        }
    }
}
