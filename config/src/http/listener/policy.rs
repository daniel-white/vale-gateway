use crate::http::policy::client_addrs::ClientAddressesPolicy;
use crate::http::policy::error_response::ErrorResponsePolicy;
use crate::http::policy::retry::RetryPolicy;
use crate::http::policy::timeout::TimeoutPolicies;
use getset::{CloneGetters, CopyGetters, Getters};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(
    Default,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Getters,
    CopyGetters,
    CloneGetters,
    TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct ListenerPolicies {
    #[getset(get_clone = "pub")]
    #[serde(default, skip_serializing_if = "TimeoutPolicies::is_none")]
    #[builder(default)]
    timeouts: TimeoutPolicies,

    #[getset(get_clone = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    retries: Option<RetryPolicy>,

    #[getset(get_clone = "pub")]
    #[serde(default = "default_client_addresses_policy", skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    client_addresses: Option<ClientAddressesPolicy>,

    #[getset(get_clone = "pub")]
    #[builder(default)]
    #[serde(default)]
    error_response: ErrorResponsePolicy,
}

impl ListenerPolicies {
    pub fn is_default(&self) -> bool {
        self.timeouts.is_none()
            && self.retries.is_none()
            && self.client_addresses.as_ref().is_some_and(|p| p.is_default())
            && self.error_response.is_default()
    }
}

fn default_client_addresses_policy() -> Option<ClientAddressesPolicy> {
    Some(ClientAddressesPolicy::default())
}
