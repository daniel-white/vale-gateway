use strum::{EnumString, IntoStaticStr};

pub mod access_control;
pub mod client_addr;
pub mod error_response;
pub mod header_modifier;
pub mod redirect_response;
pub mod static_response;
pub mod upstream_uri_rewrite;

#[derive(Debug, Clone, PartialEq, Eq, EnumString, IntoStaticStr)]
#[strum(serialize_all = "camelCase")]
pub enum ExtensionFilterKind {
    StaticResponse,
    AccessControl,
}