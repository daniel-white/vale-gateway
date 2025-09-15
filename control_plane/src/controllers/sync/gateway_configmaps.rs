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
    AccessControlFilter, AccessControlFilterEffect, ClientAddressFilterProxiesTrustedHeaders,
    ClientAddressesSource, ErrorResponseFilterKind, StaticResponseFilter,
};

use crate::http::routes::rules;
use crate::http::routes::rules::{add_http_route_rule_filters, add_http_route_rule_matches};
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
        ErrorResponseFilterKind::Empty => ConfigErrorResponses::builder()
            .kind(ConfigErrorResponseKind::Empty)
            .build(),
        ErrorResponseFilterKind::Html => ConfigErrorResponses::builder()
            .kind(ConfigErrorResponseKind::Html)
            .build(),
        ErrorResponseFilterKind::ProblemDetail => {
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

#[cfg(test)]
mod tests {
    use super::TemplateValues;
    use gtmpl::{Context, Template};
    use k8s_openapi::api::core::v1::ConfigMap;
    use serde_yaml;

    const TEMPLATE_CONTENT: &str =
        include_str!("./templates/gateway_configmap.kubernetes-helm-yaml");

    #[test]
    fn test_configmap_template_is_not_empty() {
        // Ensure the template file is not empty (this was the root cause of the bug)
        assert!(
            !TEMPLATE_CONTENT.trim().is_empty(),
            "ConfigMap template must not be empty"
        );

        // Ensure it contains expected ConfigMap structure
        assert!(TEMPLATE_CONTENT.contains("apiVersion: v1"));
        assert!(TEMPLATE_CONTENT.contains("kind: ConfigMap"));
        assert!(TEMPLATE_CONTENT.contains("metadata:"));
        assert!(TEMPLATE_CONTENT.contains("data:"));
    }

    #[test]
    fn test_configmap_template_renders_correctly() {
        // Create a template instance
        let mut template = Template::default();

        // Add required template functions (same as in sync_objects macro)
        use sprig::{
            defaults::default,
            strings::{indent, nindent},
        };
        template.add_func("default", default);
        template.add_func("indent", indent);
        template.add_func("nindent", nindent);

        // Parse the template
        template
            .parse(TEMPLATE_CONTENT)
            .expect("ConfigMap template should parse successfully");

        // Create test template values
        let template_values = TemplateValues::builder()
            .gateway_name("test-gateway")
            .config_yaml("listeners:\n  - name: http\n    port: 8080")
            .build();

        // Render the template
        let context = Context::from(template_values);
        let rendered = template
            .render(&context)
            .expect("ConfigMap template should render successfully");

        // Parse the rendered YAML to ensure it's valid
        let configmap: ConfigMap = serde_yaml::from_str(&rendered)
            .expect("Rendered template should be valid ConfigMap YAML");

        // Verify the rendered ConfigMap has expected properties
        assert_eq!(
            configmap.metadata.name,
            Some("test-gateway-configuration".to_string())
        );
        assert!(configmap.data.is_some());

        let data = configmap.data.unwrap();
        assert!(data.contains_key("gateway.yaml"));

        let gateway_yaml = data.get("gateway.yaml").unwrap();
        assert!(gateway_yaml.contains("listeners:"));
        assert!(gateway_yaml.contains("- name: http"));
        assert!(gateway_yaml.contains("port: 8080"));
    }

    #[test]
    fn test_configmap_template_handles_special_characters() {
        let mut template = Template::default();

        // Add required template functions
        use sprig::{
            defaults::default,
            strings::{indent, nindent},
        };
        template.add_func("default", default);
        template.add_func("indent", indent);
        template.add_func("nindent", nindent);

        template
            .parse(TEMPLATE_CONTENT)
            .expect("ConfigMap template should parse successfully");

        // Test with config containing special characters
        let template_values = TemplateValues::builder()
            .gateway_name("test-gateway-with-dashes")
            .config_yaml("config:\n  special: \"quotes and \\\"escapes\\\"\"\n  unicode: \"测试\"")
            .build();

        let context = Context::from(template_values);
        let rendered = template
            .render(&context)
            .expect("ConfigMap template should handle special characters");

        // Ensure the rendered YAML is still valid
        let configmap: ConfigMap = serde_yaml::from_str(&rendered)
            .expect("Rendered template with special characters should be valid");

        assert_eq!(
            configmap.metadata.name,
            Some("test-gateway-with-dashes-configuration".to_string())
        );
    }

    #[test]
    fn test_configmap_template_has_required_labels() {
        let mut template = Template::default();

        // Add required template functions
        use sprig::{
            defaults::default,
            strings::{indent, nindent},
        };
        template.add_func("default", default);
        template.add_func("indent", indent);
        template.add_func("nindent", nindent);

        template
            .parse(TEMPLATE_CONTENT)
            .expect("ConfigMap template should parse successfully");

        let template_values = TemplateValues::builder()
            .gateway_name("test-gateway")
            .config_yaml("listeners: []")
            .build();

        let context = Context::from(template_values);
        let rendered = template
            .render(&context)
            .expect("ConfigMap template should render successfully");

        let configmap: ConfigMap = serde_yaml::from_str(&rendered)
            .expect("Rendered template should be valid ConfigMap YAML");

        // Verify required labels are present
        let labels = configmap
            .metadata
            .labels
            .expect("ConfigMap should have labels");

        assert!(labels.contains_key("vale-gateway.whitefamily.in/configmap-role"));
        assert_eq!(
            labels.get("vale-gateway.whitefamily.in/configmap-role"),
            Some(&"gateway-configuration".to_string())
        );

        assert!(labels.contains_key("app.kubernetes.io/name"));
        assert_eq!(
            labels.get("app.kubernetes.io/name"),
            Some(&"test-gateway".to_string())
        );

        assert!(labels.contains_key("gateway.networking.k8s.io/gateway"));
        assert_eq!(
            labels.get("gateway.networking.k8s.io/gateway"),
            Some(&"test-gateway".to_string())
        );

        assert!(labels.contains_key("app"));
        assert_eq!(labels.get("app"), Some(&"test-gateway".to_string()));
    }
}
