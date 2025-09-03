use crate::kubernetes::objects::{ObjectRef, Objects};
use crate::kubernetes::KubeClientCell;
use crate::options::Options;
use crate::watch_objects;
use gateway_api::apis::standard::gatewayclasses::GatewayClass;
use gateway_api::apis::standard::gateways::Gateway;
use getset::Getters;
use itertools::Itertools;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};
use typed_builder::TypedBuilder;
use vg_api::constants::GATEWAY_CLASS_CONTROLLER_NAME;
use vg_api::v1alpha1::{GatewayClassParameters, GatewayParameters};
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

#[derive(Debug, PartialEq, TypedBuilder, Getters)]
pub struct Gateways {
    class: (Arc<GatewayClass>, Arc<GatewayClassParameters>),
    instances: HashMap<ObjectRef, (Arc<Gateway>, Arc<GatewayParameters>)>,
}

impl Gateways {
    pub fn iter(
        &self,
    ) -> impl Iterator<
        Item = (
            &ObjectRef,
            Arc<GatewayClass>,
            Arc<GatewayClassParameters>,
            Arc<Gateway>,
            Arc<GatewayParameters>,
        ),
    > {
        self.instances.iter().map(|(k, v)| {
            (
                k,
                self.class.0.clone(),
                self.class.1.clone(),
                v.0.clone(),
                v.1.clone(),
            )
        })
    }
}

pub fn gateways(
    task_builder: &TaskBuilder,
    options: Arc<Options>,
    client: &Receiver<KubeClientCell>,
) -> Receiver<Option<Arc<Gateways>>> {
    let (tx, rx) = signal(stringify!(gateways));
    let gateway_class_tx = gateway_class(task_builder, options.clone(), client);
    let gateway_class_parameters_rx =
        gateway_class_parameters(task_builder, options.clone(), client, &gateway_class_tx);
    let gateways_tx =
        gateway_class_instances(task_builder, options.clone(), client, &gateway_class_tx);
    let gateway_parameters_rx = watch_objects!(options, task_builder, GatewayParameters, client);

    task_builder
        .new_task(stringify!(gateways))
        .spawn(async move {
            loop {
                if let ReadyState::Ready((
                    (gateway_class_ref, gateway_class),
                    gateway_class_parameters,
                    gateways,
                    gateway_parameters,
                )) = await_ready!(
                    gateway_class_tx,
                    gateway_class_parameters_rx,
                    gateways_tx,
                    gateway_parameters_rx
                ) {
                    debug!(
                        "Assembling Gateways struct for GatewayClass: {}",
                        gateway_class_ref
                    );

                    let mut instances = HashMap::new();
                    for (gateway_ref, _, gateway) in gateways.iter() {
                        let gateway_spec = &gateway.spec;
                        let gateway_infrastructure = gateway_spec.infrastructure.as_ref();
                        let parameters = if let Some(params_ref) =
                            gateway_infrastructure.and_then(|i| i.parameters_ref.as_ref())
                        {
                            let params_ref = ObjectRef::builder()
                                .group(Some(params_ref.group.clone()))
                                .kind(&params_ref.kind)
                                .name(&params_ref.name)
                                .namespace(gateway_ref.namespace().clone())
                                .version(Some("v1alpha1".to_string()))
                                .build();
                            gateway_parameters
                                .get_by_ref(&params_ref)
                                .unwrap_or_else(|| {
                                    warn!(
                                        "GatewayParameters not found for Gateway: {}",
                                        gateway_ref
                                    );
                                    Arc::new(GatewayParameters::default())
                                })
                        } else {
                            Arc::new(GatewayParameters::default())
                        };
                        instances.insert(gateway_ref, (gateway, parameters));
                    }

                    let gateways = Gateways::builder()
                        .class((gateway_class.clone(), gateway_class_parameters.clone()))
                        .instances(instances)
                        .build();

                    info!(
                        "Assembled Gateways struct for GatewayClass: {}",
                        gateway_class_ref
                    );
                    tx.set(Some(Arc::new(gateways))).await;
                }

                continue_on!(
                    gateway_class_tx.changed(),
                    gateway_class_parameters_rx.changed(),
                    gateways_tx.changed(),
                    gateway_parameters_rx.changed()
                );
            }
        });

    rx
}

fn gateway_class(
    task_builder: &TaskBuilder,
    options: Arc<Options>,
    client: &Receiver<KubeClientCell>,
) -> Receiver<(ObjectRef, Arc<GatewayClass>)> {
    let (tx, rx) = signal(stringify!(gateway_class));
    let gateway_classes_rx = watch_objects!(options, task_builder, GatewayClass, client);

    task_builder
        .new_task(stringify!(gateway_class))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(gateway_classes) = await_ready!(gateway_classes_rx) {
                    debug!("Filtering GatewayClasses");
                    let gateway_class = gateway_classes
                        .iter()
                        .filter_map(|(_, _, gateway_class)| {
                            if gateway_class.spec.controller_name == GATEWAY_CLASS_CONTROLLER_NAME {
                                Some(gateway_class)
                            } else {
                                None
                            }
                        })
                        .exactly_one();
                    match gateway_class {
                        Ok(gateway_class) => match ObjectRef::for_object(gateway_class.as_ref()) {
                            Ok(gateway_class_ref) => {
                                info!("Found GatewayClass: object.ref={}", gateway_class_ref);
                                tx.set((gateway_class_ref, gateway_class)).await;
                            }
                            Err(e) => {
                                warn!("Error filtering GatewayClass creating ref: {}", e);
                            }
                        },
                        Err(e) => {
                            warn!("Error filtering GatewayClass: {}", e);
                        }
                    }
                }

                continue_on!(gateway_classes_rx.changed());
            }
        });

    rx
}

fn gateway_class_parameters(
    task_builder: &TaskBuilder,
    options: Arc<Options>,
    client: &Receiver<KubeClientCell>,
    gateway_class_rx: &Receiver<(ObjectRef, Arc<GatewayClass>)>,
) -> Receiver<Arc<GatewayClassParameters>> {
    let (tx, rx) = signal(stringify!(gateway_class_parameters));
    let gateway_class_rx = gateway_class_rx.clone();
    let gateway_class_parameters_rx =
        watch_objects!(options, task_builder, GatewayClassParameters, client);

    task_builder
        .new_task(stringify!(gateway_class_parameters))
        .spawn(async move {
            loop {
                if let ReadyState::Ready((
                    (gateway_class_ref, gateway_class),
                    gateway_class_parameters,
                )) = await_ready!(gateway_class_rx, gateway_class_parameters_rx)
                {
                    debug!(
                        "Filtering GatewayClassParameters for GatewayClass: {}",
                        gateway_class_ref
                    );

                    if let Some(parameters_ref) = &gateway_class.spec.parameters_ref {
                        let parameters_ref = ObjectRef::builder()
                            .group(Some(parameters_ref.group.clone()))
                            .kind(&parameters_ref.kind)
                            .name(&parameters_ref.name)
                            .version(Some("v1alpha1".to_string()))
                            .build();

                        if let Some(parameters) =
                            gateway_class_parameters.get_by_ref(&parameters_ref)
                        {
                            info!(
                                "Found GatewayClassParameters: object.ref={}",
                                parameters_ref
                            );
                            tx.set(parameters).await;
                        } else {
                            warn!(
                                "GatewayClassParameters not found: object.ref={}",
                                parameters_ref
                            );
                            tx.set(Default::default()).await;
                        }
                    } else {
                        info!(
                            "No parametersRef specified for GatewayClass: {}",
                            gateway_class_ref
                        );
                        tx.set(Default::default()).await;
                    }
                }

                continue_on!(
                    gateway_class_rx.changed(),
                    gateway_class_parameters_rx.changed()
                );
            }
        });

    rx
}

fn gateway_class_instances(
    task_builder: &TaskBuilder,
    options: Arc<Options>,
    client: &Receiver<KubeClientCell>,
    gateway_class_tx: &Receiver<(ObjectRef, Arc<GatewayClass>)>,
) -> Receiver<Objects<Gateway>> {
    let (tx, rx) = signal(stringify!(gateway_class_instances));
    let gateway_class_rx = gateway_class_tx.clone();
    let gateways_rx = watch_objects!(options, task_builder, Gateway, client);

    task_builder
        .new_task(stringify!(gateway_class_instances))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(((gateway_class_ref, _), gateways)) =
                    await_ready!(gateway_class_rx, gateways_rx)
                {
                    debug!("Filtering Gateways for GatewayClass: {}", gateway_class_ref);
                    let gateways = gateways
                        .iter()
                        .filter(|(_, _, gateway)| {
                            let current_ref = ObjectRef::of_kind::<GatewayClass>()
                                .name(&gateway.spec.gateway_class_name)
                                .build();
                            &current_ref == gateway_class_ref
                        })
                        .collect();

                    tx.set(gateways).await;
                }

                continue_on!(gateway_class_rx.changed(), gateways_rx.changed());
            }
        });

    rx
}
