use gateway_api::httproutes::HTTPRouteTimeout;
use kube_core::Duration;
use kube_core::duration::ParseError;
use std::str::FromStr;
use thiserror::Error;
use vg_config::http::policy::timeout::TimeoutPolicies;
use vg_config::http::policy::timeout::TimeoutPolicy;
use vg_core::internal_wrapper;

internal_wrapper!(HTTPRouteTimeout);

#[derive(Debug, Error)]
pub enum TimeoutPoliciesConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("Invalid request timeout: {0}")]
    RequestTimeout(#[source] ParseError),
    #[error("Invalid backend request timeout: {0}")]
    BackendRequestTimeout(#[source] ParseError),
}

impl TryFrom<HTTPRouteTimeoutWrapper<'_>> for TimeoutPolicies {
    type Error = TimeoutPoliciesConversionError;

    fn try_from(value: HTTPRouteTimeoutWrapper) -> Result<Self, Self::Error> {
        fn convert(s: Option<&str>) -> Result<Option<TimeoutPolicy>, ParseError> {
            let duration = s.map(Duration::from_str).transpose()?;
            Ok(duration.map(std::time::Duration::from).and_then(|to| {
                if to.is_zero() {
                    None
                } else {
                    let policy = TimeoutPolicy::builder().duration(to).build();
                    Some(policy)
                }
            }))
        }

        let request = convert(value.request.as_deref())
            .map_err(TimeoutPoliciesConversionError::RequestTimeout)?;
        let backend_request = convert(value.backend_request.as_deref())
            .map_err(TimeoutPoliciesConversionError::BackendRequestTimeout)?;

        let policies = Self::builder()
            .request(request)
            .backend_request(backend_request)
            .build();

        Ok(policies)
    }
}
