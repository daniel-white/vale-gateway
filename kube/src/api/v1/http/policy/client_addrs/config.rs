use crate::api::v1::http::policy::client_addrs::{
    ClientAddressesPolicy, ClientAddressesPolicyProxies,
    ClientAddressesPolicyProxiesTrustedHeaders, ClientAddressesPolicySource,
};
use http::HeaderName;
use http::header::InvalidHeaderName;
use thiserror::Error;
use vg_config::http::policy::client_addrs::{
    ClientAddressExtractor, ClientAddressesPolicy as ClientAddressesPolicyConfig,
    TrustedHeaderClientAddressExtractor, TrustedProxiesClientAddressExtractor,
    TrustedProxyHeaderName,
};
use vg_core::net::IpRef;

#[derive(Debug, Error)]
pub enum ClientAddressesPolicyConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("Invalid backend header name: {0}")]
    BackendHeaderName(#[source] InvalidHeaderName),
    #[error("`header` is required for 'Header' source")]
    MissingHeader,
    #[error("Invalid source header name: {0}")]
    Header(#[source] InvalidHeaderName),
    #[error("`proxies` is required for 'Proxies' source")]
    MissingProxies,
    #[error("Invalid source proxies configuration: {0}")]
    Proxies(
        #[from]
        #[source]
        TrustedProxiesClientAddrExtractorConversionError,
    ),
}

impl TryFrom<&ClientAddressesPolicy> for ClientAddressesPolicyConfig {
    type Error = ClientAddressesPolicyConversionError;

    fn try_from(value: &ClientAddressesPolicy) -> Result<Self, Self::Error> {
        let builder = Self::builder();

        let builder = match &value.backend_header {
            Some(header) => {
                let backend_header: HeaderName = header
                    .parse()
                    .map_err(ClientAddressesPolicyConversionError::BackendHeaderName)?;
                builder.backend_header(Some(backend_header))
            }
            None => builder.backend_header(None),
        };

        let builder = match (
            &value.source,
            value.header.as_deref(),
            value.proxies.as_ref(),
        ) {
            (ClientAddressesPolicySource::None, None, None) => {
                unreachable!("layers")
            }
            (ClientAddressesPolicySource::DirectConnection, None, None) => {
                builder.extractor(ClientAddressExtractor::Direct)
            }
            (ClientAddressesPolicySource::Header, Some(header), None) => {
                let trusted_header = header
                    .parse()
                    .map_err(ClientAddressesPolicyConversionError::Header)?;
                let extractor = TrustedHeaderClientAddressExtractor::builder()
                    .trusted_header(trusted_header)
                    .build();
                builder.extractor(extractor)
            }
            (ClientAddressesPolicySource::Header, None, _) => {
                return Err(ClientAddressesPolicyConversionError::MissingHeader);
            }
            (ClientAddressesPolicySource::Proxies, _, Some(proxies)) => {
                let extractor: TrustedProxiesClientAddressExtractor = proxies.try_into()?;
                builder.extractor(extractor)
            }
            (ClientAddressesPolicySource::Proxies, _, None) => {
                return Err(ClientAddressesPolicyConversionError::MissingProxies);
            }
            _ => return Err(ClientAddressesPolicyConversionError::InvalidConfiguration),
        };

        let policy = builder.build();

        Ok(policy)
    }
}

#[derive(Debug, Error)]
pub enum TrustedProxiesClientAddrExtractorConversionError {}

#[allow(clippy::infallible_try_from)]
impl TryFrom<&ClientAddressesPolicyProxies> for TrustedProxiesClientAddressExtractor {
    type Error = TrustedProxiesClientAddrExtractorConversionError;

    fn try_from(value: &ClientAddressesPolicyProxies) -> Result<Self, Self::Error> {
        let proxies = {
            let mut proxies: Vec<IpRef> = if value.trust_local_ranges {
                Vec::with_capacity(
                    IpRef::trusted_private().len()
                        + value.trusted_ips.len()
                        + value.trusted_ranges.len(),
                )
            } else {
                Vec::with_capacity(value.trusted_ips.len() + value.trusted_ranges.len())
            };

            if value.trust_local_ranges {
                proxies.extend_from_slice(IpRef::trusted_private());
            }

            proxies.extend(value.trusted_ips.iter().map(IpRef::from));
            proxies.extend(value.trusted_ranges.iter().map(IpRef::from));

            proxies
        };

        let trusted_headers = value
            .trusted_headers
            .iter()
            .cloned()
            .map(TrustedProxyHeaderName::from)
            .collect();

        let extractor = Self::builder()
            .proxies(proxies)
            .trusted_headers(trusted_headers)
            .build();

        Ok(extractor)
    }
}

impl From<ClientAddressesPolicyProxiesTrustedHeaders> for TrustedProxyHeaderName {
    fn from(value: ClientAddressesPolicyProxiesTrustedHeaders) -> Self {
        match value {
            ClientAddressesPolicyProxiesTrustedHeaders::Forwarded => Self::Forwarded,
            ClientAddressesPolicyProxiesTrustedHeaders::XForwardedFor => Self::XForwardedFor,
            ClientAddressesPolicyProxiesTrustedHeaders::XForwardedHost => Self::XForwardedHost,
            ClientAddressesPolicyProxiesTrustedHeaders::XForwardedProto => Self::XForwardedProto,
            ClientAddressesPolicyProxiesTrustedHeaders::XForwardedBy => Self::XForwardedBy,
        }
    }
}
