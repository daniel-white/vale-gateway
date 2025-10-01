use crate::api::v1::parameters::listeners::http::filters::client_addr::{
    ClientAddressFilterProxies, ClientAddressFilterProxiesTrustedHeaders,
    ClientAddressFilterSource, ClientAddressFilterSpec,
};
use http::HeaderName;
use http::header::InvalidHeaderName;
use thiserror::Error;
use vg_core::net::IpRef;
use vg_http_config::filters::client_addr::{
    ClientAddrExtractor, ClientAddrFilter, TrustedHeaderClientAddrExtractor,
    TrustedProxiesClientAddrExtractor, TrustedProxyHeaderName,
};

#[derive(Debug, Error)]
pub enum ClientAddrFilterConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("Invalid backend header name: {0}")]
    BackendHeaderName(InvalidHeaderName),
    #[error("`header` is required for 'Header' source")]
    MissingHeader,
    #[error("Invalid source header name: {0}")]
    Header(InvalidHeaderName),
    #[error("`proxies` is required for 'Proxies' source")]
    MissingProxies,
    #[error("Invalid source proxies configuration: {0}")]
    Proxies(#[from] TrustedProxiesClientAddrExtractorConversionError),
}

impl TryFrom<&ClientAddressFilterSpec> for ClientAddrFilter {
    type Error = ClientAddrFilterConversionError;

    fn try_from(value: &ClientAddressFilterSpec) -> Result<Self, Self::Error> {
        let builder = Self::builder();

        let builder = match &value.backend_header {
            Some(header) => {
                let backend_header: HeaderName = header
                    .parse()
                    .map_err(ClientAddrFilterConversionError::BackendHeaderName)?;
                builder.upstream_header(Some(backend_header))
            }
            None => builder.upstream_header(None),
        };

        let builder = match (
            &value.source,
            value.header.as_deref(),
            value.proxies.as_ref(),
        ) {
            (ClientAddressFilterSource::None, None, None) => {
                builder.extractor(ClientAddrExtractor::None)
            }
            (ClientAddressFilterSource::DirectConnection, None, None) => {
                builder.extractor(ClientAddrExtractor::Direct)
            }
            (ClientAddressFilterSource::Header, Some(header), None) => {
                let trusted_header = header
                    .parse()
                    .map_err(ClientAddrFilterConversionError::Header)?;
                let extractor = TrustedHeaderClientAddrExtractor::builder()
                    .trusted_header(trusted_header)
                    .build();
                builder.extractor(extractor)
            }
            (ClientAddressFilterSource::Header, None, _) => {
                return Err(ClientAddrFilterConversionError::MissingHeader);
            }
            (ClientAddressFilterSource::Proxies, _, Some(proxies)) => {
                let extractor: TrustedProxiesClientAddrExtractor = proxies.try_into()?;
                builder.extractor(extractor)
            }
            (ClientAddressFilterSource::Proxies, _, None) => {
                return Err(ClientAddrFilterConversionError::MissingProxies);
            }
            _ => return Err(ClientAddrFilterConversionError::InvalidConfiguration),
        };

        let filter = builder.build();

        Ok(filter)
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

impl From<ClientAddressFilterProxiesTrustedHeaders> for TrustedProxyHeaderName {
    fn from(value: ClientAddressFilterProxiesTrustedHeaders) -> Self {
        match value {
            ClientAddressFilterProxiesTrustedHeaders::Forwarded => Self::Forwarded,
            ClientAddressFilterProxiesTrustedHeaders::XForwardedFor => Self::XForwardedFor,
            ClientAddressFilterProxiesTrustedHeaders::XForwardedHost => Self::XForwardedHost,
            ClientAddressFilterProxiesTrustedHeaders::XForwardedProto => Self::XForwardedProto,
            ClientAddressFilterProxiesTrustedHeaders::XForwardedBy => Self::XForwardedBy,
        }
    }
}
