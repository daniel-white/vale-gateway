use crate::api::v1::http::route::filter::config::{
    HTTPRouteBackendFilterWrapper, RuleBackendFilterConversionError,
};
use crate::resources::ServiceRef;
use gateway_api::httproutes::HTTPBackendReference;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_core::net::Port;
use vg_http_config::routing::backend::BackendRef;
use vg_http_config::routing::rule::backend::RuleBackend;
use vg_http_config::routing::rule::filter::RuleBackendFilter;

#[derive(Debug, TypedBuilder)]
pub struct HTTPBackendReferenceWrapper<'a> {
    namespace: &'a str,
    backend_ref: &'a HTTPBackendReference,
}

#[derive(Debug, Error)]
pub enum RuleBackendConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("Invalid port")]
    Port,
    #[error("Invalid filter at index {0}: {1}")]
    Filter(usize, #[source] RuleBackendFilterConversionError),
    #[error("Invalid backend reference")]
    BackendRef,
    #[error("Unsupported backend")]
    UnsupportedBackend,
}

impl TryFrom<HTTPBackendReferenceWrapper<'_>> for RuleBackend {
    type Error = RuleBackendConversionError;

    fn try_from(value: HTTPBackendReferenceWrapper) -> Result<Self, Self::Error> {
        let namespace = value.namespace;
        let backend_ref = value.backend_ref;

        let ref_ = match backend_ref.group.as_deref() {
            None | Some("core") => match backend_ref.kind.as_deref() {
                None | Some("Service") => {
                    let namespace = backend_ref.namespace.as_deref().unwrap_or(namespace);
                    let ref_ = ServiceRef::new(namespace, &backend_ref.name);
                    BackendRef::from(ref_.to_string())
                }
                _ => return Err(RuleBackendConversionError::UnsupportedBackend),
            },
            _ => return Err(RuleBackendConversionError::UnsupportedBackend),
        };

        let port = value
            .backend_ref
            .port
            .map(Port::try_from)
            .transpose()
            .map_err(|_| RuleBackendConversionError::Port)?;

        let filters = backend_ref
            .filters
            .iter()
            .flatten()
            .enumerate()
            .map(|(idx, filter)| {
                let filter = HTTPRouteBackendFilterWrapper::builder()
                    .namespace(namespace)
                    .filter(filter)
                    .build();
                RuleBackendFilter::try_from(filter)
                    .map_err(|err| RuleBackendConversionError::Filter(idx, err))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(RuleBackend::builder()
            .ref_(ref_)
            .port(port)
            .weight(backend_ref.weight)
            .filters(filters)
            .build())
    }
}
