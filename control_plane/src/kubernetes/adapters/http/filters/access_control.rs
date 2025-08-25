use vg_api::v1alpha1::{ GatewayConfiguration};
use vg_core::http::filters::access_control::{HttpAccessControlFilter, HttpAccessControlFilterRef};

fn convert_access_control(
    key: &str,
    gateway_configuration: &GatewayConfiguration
) -> Option<(HttpAccessControlFilterRef, HttpAccessControlFilter)> {
    None
    // if !extension_filters.access_controls().is_empty() {
    //     let filters = extension_filters
    //         .access_controls()
    //         .iter()
    //         .map(|(ref_, _, filter)| {
    //             let effect = match filter.spec.effect {
    //                 AccessControlFilterEffect::Allow => AccessControlEffect::Allow,
    //                 AccessControlFilterEffect::Deny => AccessControlEffect::Deny,
    //             };
    // 
    //             let clients = HttpAccessControlClients::builder()
    //                 .ip_ranges(filter.spec.clients.ip_ranges.clone())
    //                 .ips(filter.spec.clients.ips.clone())
    //                 .build();
    // 
    //             HttpAccessControlFilter::builder()
    //                 .key(ref_.to_string())
    //                 .effect(effect)
    //                 .clients(clients)
    //                 .build()
    //         })
    //         .collect();
    // 
    //     gateway.with_access_control_filters(filters);
    // }
}