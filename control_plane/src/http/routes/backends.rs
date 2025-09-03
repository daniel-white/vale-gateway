use gateway_api::httproutes::HTTPRoute;
use std::collections::{HashMap, HashSet};
use std::num::NonZeroU16;
use std::vec::Vec;
use vg_core::http::routes::backends::HttpRouteBackend;
use vg_core::http::routes::rules::{HttpRouteRuleBuilder, HttpRouteRuleKey};
use vg_core::net::Port;
use crate::http::routes::controllers::HttpRouteInfo;
use crate::kubernetes::objects::ObjectRef;

pub fn collect_http_backends(
    http_route: &HTTPRoute,
    http_backend_refs: &mut HashSet<ObjectRef>
) -> HashMap<HttpRouteRuleKey, Vec<HttpRouteBackend>> {
    let mut backends = HashMap::new();

    if let Some(rules) = &http_route.spec.rules {
        for (rule_idx, rule) in rules.iter().enumerate() {
            let rule_key: HttpRouteRuleKey = format!(
                "{}-{}-rule-{}",
                http_route.metadata.namespace.as_ref().unwrap(),
                http_route.metadata.name.as_ref().unwrap(),
                rule_idx
            )
            .into();
            let backends = backends.entry(rule_key).or_insert_with(Vec::new);
            if let Some(backend_refs) = &rule.backend_refs {
                for backend in backend_refs {
                    match backend.kind {
                        Some(ref kind) if kind == "Service" => {
                            let name = backend.name.as_str();
                            let namespace = backend
                                .namespace
                                .as_ref()
                                .or_else(|| http_route.metadata.namespace.as_ref())
                                .unwrap();
                            let backend_ref = ObjectRef::builder()
                                .kind("Service")
                                .name(name)
                                .namespace(Some(namespace.clone()))
                                .build();
                            let route_backend = HttpRouteBackend::builder()
                                .weight(backend.weight.map(|w| w as u32))
                                .port(backend.port.and_then(|p| NonZeroU16::new(p as u16)).map(Port::new))
                                .kind("Service")
                                .name(name)
                                .namespace(namespace)
                                .build();
                            backends.push(route_backend);
                            http_backend_refs.insert(backend_ref);
                        }
                        _ => { /* Ignore unsupported kinds */ }
                    }
                }
            }
        }
    }

    backends
}

pub fn add_http_route_rule_backends(
    key: &HttpRouteRuleKey,
    route: &HttpRouteInfo,
    builder: &mut HttpRouteRuleBuilder,
) {
    if let Some(backends) = route.backends().get(key) {
        for backend in backends {
            builder.add_backend(backend.clone());
        }
    }
}
