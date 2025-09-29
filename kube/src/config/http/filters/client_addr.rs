use crate::api::v1::parameters::listeners::http::filters::{
    ClientAddressFilterProxies, ClientAddressFilterProxiesTrustedHeaders,
    ClientAddressFilterSource, ClientAddressFilterSpec,
};
use http::HeaderName;
use http::header::InvalidHeaderName;
use thiserror::Error;
use vg_core::net::IpRef;
use vg_http_config::IpRef as IpRefConfig;
use vg_http_config::filters::client_addr::{
    ClientAddrExtractor, ClientAddrFilter, TrustedHeaderClientAddrExtractor,
    TrustedProxiesClientAddrExtractor, TrustedProxyHeaderName,
};

#[derive(Debug, Error)]
pub enum ClientAddrFilterConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("Invalid backend header name: {0}")]
    InvalidBackendHeaderName(InvalidHeaderName),
    #[error("Source header is required for 'Header' source")]
    MissingHeader,
    #[error("Invalid source header name: {0}")]
    InvalidHeaderName(InvalidHeaderName),
    #[error("Proxies configuration is required for 'Proxies' source")]
    MissingProxies,
    #[error("Invalid source proxies configuration: {0}")]
    InvalidProxies(#[from] TrustedProxiesClientAddrExtractorConversionError),
}

impl TryFrom<&ClientAddressFilterSpec> for ClientAddrFilter {
    type Error = ClientAddrFilterConversionError;

    fn try_from(value: &ClientAddressFilterSpec) -> Result<Self, Self::Error> {
        let backend_header = match &value.backend_header {
            Some(header) => {
                let backend_header: HeaderName = header.parse().map_err(|err| {
                    ClientAddrFilterConversionError::InvalidBackendHeaderName(err)
                })?;
                Some(backend_header)
            }
            None => None,
        };

        let extractor = match (
            &value.source,
            value.header.as_deref(),
            value.proxies.as_ref(),
        ) {
            (ClientAddressFilterSource::None, None, None) => ClientAddrFilter::builder()
                .extractor(ClientAddrExtractor::None)
                .upstream_header(backend_header)
                .build(),
            (ClientAddressFilterSource::DirectConnection, None, None) => {
                ClientAddrFilter::builder()
                    .extractor(ClientAddrExtractor::Direct)
                    .upstream_header(backend_header)
                    .build()
            }
            (ClientAddressFilterSource::Header, Some(header), None) => {
                let trusted_header = header
                    .parse()
                    .map_err(ClientAddrFilterConversionError::InvalidHeaderName)?;
                let extractor = TrustedHeaderClientAddrExtractor::builder()
                    .trusted_header(trusted_header)
                    .build();
                ClientAddrFilter::builder()
                    .extractor(ClientAddrExtractor::from(extractor))
                    .upstream_header(backend_header)
                    .build()
            }
            (ClientAddressFilterSource::Header, None, _) => {
                return Err(ClientAddrFilterConversionError::MissingHeader);
            }
            (ClientAddressFilterSource::Proxies, _, Some(proxies)) => {
                let extractor: TrustedProxiesClientAddrExtractor = proxies.try_into()?;
                ClientAddrFilter::builder()
                    .extractor(ClientAddrExtractor::from(extractor))
                    .upstream_header(backend_header)
                    .build()
            }
            (ClientAddressFilterSource::Proxies, _, None) => {
                return Err(ClientAddrFilterConversionError::MissingProxies);
            }
            _ => return Err(ClientAddrFilterConversionError::InvalidConfiguration),
        };

        Ok(extractor)
    }
}

#[derive(Debug, Error)]
pub enum TrustedProxiesClientAddrExtractorConversionError {}

#[allow(clippy::infallible_try_from)]
impl TryFrom<&ClientAddressFilterProxies> for TrustedProxiesClientAddrExtractor {
    type Error = TrustedProxiesClientAddrExtractorConversionError;

    fn try_from(value: &ClientAddressFilterProxies) -> Result<Self, Self::Error> {
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

            proxies.iter().map(IpRefConfig::from).collect()
        };

        let trusted_headers = value
            .trusted_headers
            .iter()
            .map(TrustedProxyHeaderName::from)
            .collect();

        let extractor = Self::builder()
            .proxies(proxies)
            .trusted_headers(trusted_headers)
            .build();

        Ok(extractor)
    }
}

impl From<&ClientAddressFilterProxiesTrustedHeaders> for TrustedProxyHeaderName {
    fn from(value: &ClientAddressFilterProxiesTrustedHeaders) -> Self {
        match value {
            ClientAddressFilterProxiesTrustedHeaders::Forwarded => Self::Forwarded,
            ClientAddressFilterProxiesTrustedHeaders::XForwardedFor => Self::XForwardedFor,
            ClientAddressFilterProxiesTrustedHeaders::XForwardedHost => Self::XForwardedHost,
            ClientAddressFilterProxiesTrustedHeaders::XForwardedProto => Self::XForwardedProto,
            ClientAddressFilterProxiesTrustedHeaders::XForwardedBy => Self::XForwardedBy,
        }
    }
}
