use crate::controllers::instances::InstanceRole;
use crate::controllers::transformers::{Backend, ExtensionFilters, GatewayInstanceConfiguration};
use crate::ipc::IpcServices;
use crate::kubernetes::objects::{ObjectRef, SyncObjectAction};
use crate::kubernetes::KubeClientCell;
use crate::options::Options;
use crate::{sync_objects, watch_objects};
use axum::http::HeaderName;
use gateway_api::apis::standard::httproutes::HTTPRoute;
use getset::CloneGetters;
use gtmpl_derive::Gtmpl;
use k8s_openapi::api::core::v1::{ConfigMap, Service};
use kube::runtime::watcher::Config;
use kube::ResourceExt;
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::select;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error, info, warn};
use typed_builder::TypedBuilder;
use vg_api::v1alpha1::{
    AccessControlFilter, AccessControlFilterEffect, ClientAddressesSource, ErrorResponseKind,
    ProxyIpAddressHeaders, StaticResponseFilter,
};

use crate::kubernetes::adapters::http::rules;
use crate::kubernetes::adapters::http::rules::{
    add_http_route_rules_filters, add_http_route_rules_matches,
};
use vg_core::gateways::{Gateway, GatewayBuilder};
use vg_core::http::filters::client_addr::HttpProxyHeaders;
use vg_core::http::matches::{HttpMethodMatch, HttpRouteRuleMatchesBuilder};
use vg_core::http::routes::rules::{HttpRouteRuleBuilder, HttpRouteRuleFilter};
use vg_core::http::routes::HttpRouteBuilder;
use vg_core::net::Port;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_after, continue_on, ReadyState};

const TEMPLATE: &str = include_str!("./templates/gateway_configmap.kubernetes-helm-yaml");

#[derive(Clone, TypedBuilder, Debug, Gtmpl)]
struct TemplateValues {
    #[builder(setter(into))]
    gateway_name: String,
    #[builder(setter(into))]
    config_yaml: String,
}

#[derive(TypedBuilder, CloneGetters, Clone)]
pub struct SyncGatewayConfigmapsParams {
    #[getset(get_clone = "pub")]
    options: Arc<Options>,
    #[getset(get_clone = "pub")]
    kube_client_rx: Receiver<KubeClientCell>,
    #[getset(get_clone = "pub")]
    ipc_services: Arc<IpcServices>,
    #[getset(get_clone = "pub")]
    instance_role_rx: Receiver<InstanceRole>,
    #[getset(get_clone = "pub")]
    primary_instance_ip_addr_rx: Receiver<IpAddr>,
    #[getset(get_clone = "pub")]
    gateway_instances_rx: Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>>,
    #[getset(get_clone = "pub")]
    http_routes_rx: Receiver<HashMap<ObjectRef, Vec<Arc<HTTPRoute>>>>,
    #[getset(get_clone = "pub")]
    backends_rx: Receiver<HashMap<ObjectRef, Backend>>,
    #[getset(get_clone = "pub")]
    extension_filters_rx: Receiver<HashMap<ObjectRef, ExtensionFilters>>,
}

pub fn sync_gateway_configmaps(task_builder: &TaskBuilder, params: SyncGatewayConfigmapsParams) {
    let options = params.options();
    let kube_client_rx = params.kube_client_rx();
    let instance_role_rx = params.instance_role_rx();

    let (tx, current_refs_rx) = sync_objects!(
        options,
        task_builder,
        ConfigMap,
        kube_client_rx,
        instance_role_rx,
        TemplateValues,
        TEMPLATE
    );

    let params = GenerateGatewayConfigmapsParams::builder()
        .options(params.options())
        .sync_tx(tx)
        .ipc_services(params.ipc_services())
        .current_refs_rx(current_refs_rx)
        .primary_instance_ip_addr_rx(params.primary_instance_ip_addr_rx())
        .gateway_instances_rx(params.gateway_instances_rx())
        .http_routes_rx(params.http_routes_rx())
        .backends_rx(params.backends_rx())
        .extension_filters_rx(params.extension_filters_rx())
        .build();

    generate_gateway_configmaps(task_builder, params);
}

#[derive(TypedBuilder, CloneGetters, Clone)]
struct GenerateGatewayConfigmapsParams {
    #[getset(get_clone = "pub")]
    options: Arc<Options>,
    #[getset(get_clone = "pub")]
    sync_tx: UnboundedSender<SyncObjectAction<TemplateValues, ConfigMap>>,
    #[getset(get_clone = "pub")]
    ipc_services: Arc<IpcServices>,
    #[getset(get_clone = "pub")]
    current_refs_rx: Receiver<HashSet<ObjectRef>>,
    #[getset(get_clone = "pub")]
    primary_instance_ip_addr_rx: Receiver<IpAddr>,
    #[getset(get_clone = "pub")]
    gateway_instances_rx: Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>>,
    #[getset(get_clone = "pub")]
    http_routes_rx: Receiver<HashMap<ObjectRef, Vec<Arc<HTTPRoute>>>>,
    #[getset(get_clone = "pub")]
    backends_rx: Receiver<HashMap<ObjectRef, Backend>>,
    #[getset(get_clone = "pub")]
    extension_filters_rx: Receiver<HashMap<ObjectRef, ExtensionFilters>>,
}

fn generate_gateway_configmaps(
    task_builder: &TaskBuilder,
    params: GenerateGatewayConfigmapsParams,
) {
    let gateway_configurations_rx = generate_gateway_configurations(
        task_builder,
        params.ipc_services(),
        params.primary_instance_ip_addr_rx(),
        params.gateway_instances_rx(),
        params.http_routes_rx(),
        params.backends_rx(),
        params.extension_filters_rx(),
    );

    task_builder
        .new_task(stringify!(sync_gateway_configmaps))
        .spawn(async move {
            let current_refs_rx = params.current_refs_rx();
            loop {
                if let ReadyState::Ready((gateway_configurations, current_configmap_refs)) =
                    await_ready!(gateway_configurations_rx, current_refs_rx)
                {
                    info!("Reconciling Gateway ConfigMaps");
                    let desired_gateway_configurations = expand(gateway_configurations);

                    let desired_configmap_refs: HashSet<_> = desired_gateway_configurations
                        .iter()
                        .map(|state| state.configmap_ref.clone())
                        .collect();

                    let deleted_refs = current_configmap_refs.difference(&desired_configmap_refs);
                    for deleted_ref in deleted_refs {
                        let _ = params
                            .sync_tx
                            .send(SyncObjectAction::Delete(deleted_ref.clone()))
                            .inspect(|()| {
                                params
                                    .ipc_services()
                                    .remove_gateway_configuration(deleted_ref);
                            })
                            .inspect_err(|err| {
                                error!("Failed to send delete action: {}", err);
                            });
                    }

                    'send_and_insert: for gateway_state in desired_gateway_configurations {
                        let Some((template_values, config)) = &gateway_state.values else {
                            continue 'send_and_insert;
                        };

                        if let Err(err) = params.sync_tx.send(SyncObjectAction::Upsert(
                            gateway_state.configmap_ref.clone(),
                            gateway_state.gateway_ref.clone(),
                            template_values.clone(),
                            None,
                        )) {
                            warn!("Failed to send upsert action: {}", err);
                            continue 'send_and_insert;
                        }

                        if let Err(err) = params.ipc_services().try_insert_gateway_configuration(
                            gateway_state.gateway_ref.clone(),
                            config.clone(),
                        ) {
                            warn!("Failed to insert gateway configuration: {}", err);
                            continue 'send_and_insert;
                        }
                    }
                }

                continue_after!(
                    params.options.auto_cycle_duration(),
                    gateway_configurations_rx.changed(),
                    params.current_refs_rx.changed()
                );
            }
        });
}

#[derive(Debug, TypedBuilder)]
struct GatewayState {
    gateway_ref: ObjectRef,
    configmap_ref: ObjectRef,
    values: Option<(TemplateValues, Gateway)>,
}

fn expand(configurations: &HashMap<ObjectRef, Option<Gateway>>) -> Vec<GatewayState> {
    configurations
        .iter()
        .map(|(gateway_ref, config)| {
            let configmap_ref = ObjectRef::of_kind::<ConfigMap>()
                .namespace(gateway_ref.namespace().clone())
                .name(format!("{}-configuration", gateway_ref.name()))
                .build();

            let state = GatewayState::builder()
                .gateway_ref(gateway_ref.clone())
                .configmap_ref(configmap_ref);

            let state = if let Some(config) = config {
                let config_yaml = match serde_yaml::to_string(config) {
                    Ok(yaml) => yaml,
                    Err(err) => {
                        warn!(
                            "Failed to serialize configuration for gateway {}: {}",
                            gateway_ref, err
                        );
                        return state.values(None).build();
                    }
                };

                let template_values = TemplateValues::builder()
                    .gateway_name(gateway_ref.name())
                    .config_yaml(config_yaml)
                    .build();

                state.values(Some((template_values, config)))
            } else {
                warn!("No configuration found for gateway: {}", gateway_ref);
                state.values(None)
            };

            state.build()
        })
        .collect::<Vec<_>>()
}
fn generate_gateway_configurations(
    task_builder: &TaskBuilder,
    ipc_services: Arc<IpcServices>,
    primary_instance_ip_addr_rx: Receiver<IpAddr>,
    gateway_instances_rx: Receiver<HashMap<ObjectRef, GatewayInstanceConfiguration>>,
    http_routes_rx: Receiver<HashMap<ObjectRef, Vec<Arc<HTTPRoute>>>>,
    backends_rx: Receiver<HashMap<ObjectRef, Backend>>,
    extension_filters_rx: Receiver<HashMap<ObjectRef, ExtensionFilters>>,
) -> Receiver<HashMap<ObjectRef, Gateway>> {
    let (tx, rx) = signal("generated_gateway_configurations");

    task_builder
        .new_task(stringify!(generate_gateway_configurations))
        .spawn(async move {
            loop {
                if let ReadyState::Ready((
                    primary_instance_ip_addr,
                    gateway_instances,
                    http_routes,
                    backends,
                    extension_filters,
                )) = await_ready!(
                    primary_instance_ip_addr_rx,
                    gateway_instances_rx,
                    http_routes_rx,
                    backends_rx,
                    extension_filters_rx
                ) {
                    let configs: HashMap<ObjectRef, Option<Gateway>> = gateway_instances
                        .iter()
                        .map(|(gateway_ref, gateway_instance)| {
                            let mut gateway = Gateway::builder();
                            let extension_filters = extension_filters.get(gateway_ref);

                            set_ipc(&mut gateway, &ipc_services, *primary_instance_ip_addr);
                            set_client_addrs_strategy(&mut gateway, gateway_instance);
                            set_error_responses_strategy(&mut gateway, gateway_instance);
                            if let Some(extension_filters) = extension_filters {
                                apply_static_response_filters(&mut gateway, extension_filters);
                                apply_access_control_filters(&mut gateway, extension_filters);
                            }

                            add_listeners(&mut gateway, gateway_instance);

                            process_http_routes(
                                gateway_ref,
                                gateway_instance,
                                http_routes,
                                backends,
                                &mut gateway,
                            );

                            (gateway_ref.clone(), Some(gateway.build()))
                        })
                        .collect();

                    tx.set(configs).await;
                }
            }
        });

    rx
}

fn format_rule_id(gateway: &Gateway, route: &HTTPRoute, idx: usize) -> Option<String> {
    let gateway_uid = gateway.metadata.uid.as_ref()?;
    let route_uid = route.metadata.uid.as_ref()?;

    Some(format!("{gateway_uid}:{route_uid}:{idx}"))
}

fn add_backend(backend: &Backend, target: &mut HttpRouteRuleBuilder) {
    target.add_backend(|target| {
        let object_ref = backend.object_ref();
        target
            .named(object_ref.name())
            .with_namespace(object_ref.namespace().as_ref())
            .with_port(backend.port())
            .with_weight(backend.weight());

        for endpoint in backend.endpoints() {
            for address in endpoint.addresses().iter().copied() {
                target.add_endpoint(address, |target| {
                    let zone_ref = endpoint.location();
                    if let Some(node) = zone_ref.node() {
                        target.with_node(node);
                    }
                    if let Some(zone) = zone_ref.zone() {
                        target.with_zone(zone);
                    }
                });
            }
        }
    });
}

#[allow(clippy::too_many_lines)]
fn process_http_routes(
    gateway_ref: &ObjectRef,
    gateway_instance: &GatewayInstanceConfiguration,
    http_routes: &HashMap<ObjectRef, Vec<Arc<HTTPRoute>>>,
    backends: &HashMap<ObjectRef, Backend>,
    gateway_configuration: &mut GatewayConfigurationBuilder,
) {
    // Find routes that reference this gateway
    for http_routes_for_ref in http_routes.values() {
        for http_route in http_routes_for_ref {
            // Check if this route references our gateway
            let references_this_gateway =
                http_route
                    .spec
                    .parent_refs
                    .as_ref()
                    .is_some_and(|parent_refs| {
                        parent_refs.iter().any(|parent_ref| {
                            parent_ref.name == *gateway_ref.name()
                                && parent_ref.namespace.as_ref().unwrap_or(
                                    &http_route.metadata.namespace.clone().unwrap_or_default(),
                                ) == gateway_ref.namespace().as_ref().unwrap_or(&String::new())
                        })
                    });

            if !references_this_gateway {
                continue;
            }

            gateway_configuration.add_http_route(|r| {
                add_host_header_matches_for_route(http_route, r);

                // Process rules - handle the Option<Vec<HTTPRouteRules>> properly
                if let Some(rules) = &http_route.spec.rules {
                    for (rule_idx, rule) in rules.iter().enumerate() {
                        let rule_id = format_rule_id(gateway_instance.gateway(), http_route, rule_idx)
                            .unwrap_or_else(|| format!("rule-{rule_idx}"));

                        r.add_rule(rule_id, |builder| {
                            add_http_route_rules_filters(http_route, (rule, rule_idx), builder);
                            add_http_route_rules_matches(rule, builder);

                            // Process backend references
                            if let Some(backend_refs) = &rule.backend_refs {
                                for backend_ref in backend_refs {
                                    let source_ref = ObjectRef::of_kind::<Service>()
                                        .namespace(
                                            backend_ref.namespace.clone().or_else(|| {
                                                http_route.metadata.namespace.clone()
                                            }),
                                        )
                                        .name(&backend_ref.name)
                                        .build();

                                    match backends.get(&source_ref) {
                                        Some(source) => {
                                            add_backend(source, builder);
                                        }
                                        None => {
                                            warn!(
                                                "Backend reference {} not found for HTTPRoute {:?} at rule index {}",
                                                backend_ref.name, http_route.metadata.name, rule_idx
                                            );
                                        }
                                    }
                                }
                            }
                        });
                    }
                }
            });
        }
    }
}

fn add_host_header_matches_for_route(route: &Arc<HTTPRoute>, builder: &mut HttpRouteBuilder) {
    for hostname in route.spec.hostnames.iter().flatten() {
        builder.add_host_header_match(|builder| {});

        match map_hostname_match_to_type(Some(hostname)) {
            Some(HostnameMatchType::Exact(hostname)) => {
                builder.a(hostname);
            }
            Some(HostnameMatchType::Suffix(hostname)) => {
                builder.add_host_header_with_suffix(hostname);
            }
            None => {}
        }
    }
}

fn set_ipc(
    gateway_configuration: &mut GatewayConfigurationBuilder,
    ipc_services: &IpcServices,
    primary_instance_ip_addr: IpAddr,
) {
    gateway_configuration.with_ipc(|cp| {
        cp.with_endpoint(primary_instance_ip_addr, ipc_services.port());
    });
}

fn add_listeners(
    gateway_configuration: &mut GatewayConfigurationBuilder,
    instance: &GatewayInstanceConfiguration,
) {
    for (idx, listener) in instance.gateway().spec.listeners.iter().enumerate() {
        let port = if let Ok(port) = u16::try_from(listener.port) {
            Port::new(port)
        } else {
            warn!(
                "Invalid port {} for listener {} at index {} in gateway {:?}",
                listener.port,
                listener.name,
                idx,
                instance.gateway().metadata.name
            );
            continue;
        };
        gateway_configuration.add_listener(|l| {
            l.with_name(&listener.name)
                .with_port(port)
                .with_protocol(&listener.protocol);

            match map_hostname_match_to_type(listener.hostname.as_deref()) {
                Some(HostnameMatchType::Exact(hostname)) => {
                    l.with_exact_hostname(hostname);
                }
                Some(HostnameMatchType::Suffix(hostname)) => {
                    l.with_hostname_suffix(hostname);
                }
                None => {}
            }
        });
    }
}

fn set_error_responses_strategy(
    gateway_configuration: &mut GatewayConfigurationBuilder,
    instance: &GatewayInstanceConfiguration,
) {
    let error_responses = instance
        .configuration()
        .error_responses
        .clone()
        .unwrap_or_default();

    let error_responses = match error_responses.kind {
        ErrorResponseKind::Empty => ConfigErrorResponses::builder()
            .kind(ConfigErrorResponseKind::Empty)
            .build(),
        ErrorResponseKind::Html => ConfigErrorResponses::builder()
            .kind(ConfigErrorResponseKind::Html)
            .build(),
        ErrorResponseKind::ProblemDetail => {
            let problem_detail = match error_responses.problem_detail {
                Some(problem_detail) => ProblemDetailErrorResponse::builder()
                    .authority(problem_detail.authority)
                    .build(),
                None => ProblemDetailErrorResponse::default(),
            };

            ConfigErrorResponses::builder()
                .kind(ConfigErrorResponseKind::ProblemDetail)
                .problem_detail(problem_detail)
                .build()
        }
    };

    gateway_configuration.with_error_responses(error_responses);
}
