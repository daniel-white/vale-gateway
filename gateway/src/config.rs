use getset::{CloneGetters, CopyGetters};
use http::Uri;
use std::env;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::gateway::GatewayRef;
use vg_core::net::Port;

#[derive(Debug, Clone, TypedBuilder, CloneGetters, CopyGetters)]
pub struct GatewayConfig {
    #[getset(get_copy = "pub")]
    http_port: Port,
    #[getset(get_clone = "pub")]
    gateway_instance: GatewayRef,
    #[getset(get_clone = "pub")]
    controller_endpoint: Uri,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Missing required environment variable: {name}")]
    MissingEnvVar { name: String },
    #[error("Invalid value for environment variable {name}: {value}")]
    InvalidEnvVar { name: String, value: String },
}

impl GatewayConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let http_port = env::var("VALE_GATEWAY_HTTP_PORT")
            .unwrap_or_else(|_| "80".to_string())
            .parse::<u16>()
            .map_err(|_| ConfigError::InvalidEnvVar {
                name: "VALE_GATEWAY_HTTP_PORT".to_string(),
                value: env::var("VALE_GATEWAY_HTTP_PORT").unwrap_or_default(),
            })?
            .try_into()
            .map_err(|_| ConfigError::InvalidEnvVar {
                name: "VALE_GATEWAY_HTTP_PORT".to_string(),
                value: env::var("VALE_GATEWAY_HTTP_PORT").unwrap_or_default(),
            })?;

        let gateway_instance = env::var("VALE_GATEWAY_INSTANCE")
            .unwrap_or_else(|_| "gateway1".to_string())
            .into();

        let controller_endpoint = env::var("VALE_GATEWAY_CONTROLLER_ENDPOINT")
            .unwrap_or_else(|_| "ws://localhost:9000".to_string())
            .parse::<Uri>()
            .map_err(|_| ConfigError::InvalidEnvVar {
                name: "VALE_GATEWAY_CONTROLLER_ENDPOINT".to_string(),
                value: env::var("VALE_GATEWAY_CONTROLLER_ENDPOINT").unwrap_or_default(),
            })?;

        Ok(Self::builder()
            .http_port(http_port)
            .gateway_instance(gateway_instance)
            .controller_endpoint(controller_endpoint)
            .build())
    }
}
