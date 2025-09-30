mod cluster_scoped;
mod collection;
mod kinds;
mod macros;
mod namespace_scoped;

use crate::api::v1::parameters::listeners::http::filters::AccessControlFilter;
use crate::api::v1::parameters::listeners::http::filters::client_addr::ClientAddressFilter;
use crate::api::v1::parameters::listeners::http::filters::error_response::ErrorResponseFilter;
use crate::api::v1::parameters::listeners::http::filters::static_response::StaticResponseFilter;
use crate::api::v1::parameters::{GatewayClassParameters, GatewayParameters};
use crate::{cluster_scope, namespace_scope};
pub use cluster_scoped::*;
pub use collection::*;
use gateway_api::gatewayclasses::GatewayClass;
use gateway_api::gateways::Gateway;
use gateway_api::httproutes::HTTPRoute;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{ConfigMap, Service};
use k8s_openapi::api::discovery::v1::EndpointSlice;
pub use kinds::*;
pub use namespace_scoped::*;
use paste::paste;

impl ClusterScopedResource for GatewayClass {}

impl NamespaceScopedResource for Service {}

cluster_scope!(GatewayClass);
cluster_scope!(GatewayClassParameters);
namespace_scope!(Service);
namespace_scope!(EndpointSlice);
namespace_scope!(ConfigMap);
namespace_scope!(Deployment);
namespace_scope!(GatewayParameters);
namespace_scope!(Gateway);
namespace_scope!(HTTPRoute);
namespace_scope!(AccessControlFilter);
namespace_scope!(ErrorResponseFilter);
namespace_scope!(ClientAddressFilter);
namespace_scope!(StaticResponseFilter);
