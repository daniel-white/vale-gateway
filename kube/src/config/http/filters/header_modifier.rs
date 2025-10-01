use crate::api::v1::parameters::listeners::http::filters::HeaderModifier;
use http::header::{InvalidHeaderName, InvalidHeaderValue};
use http::{HeaderMap, HeaderName, HeaderValue};
use thiserror::Error;
use vg_http_config::filters::header_modifier::HeaderModifierFilter;

#[derive(Debug, Error)]
pub enum HeaderModifierFilterConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("Invalid add header name at index {0}: {1}")]
    AddHeaderName(usize, InvalidHeaderName),
    #[error("Invalid add header value at index {0}: {1}")]
    AddHeaderValue(usize, InvalidHeaderValue),
    #[error("Invalid set header name at index {0}: {1}")]
    SetHeaderName(usize, InvalidHeaderName),
    #[error("Invalid set header value at index {0}: {1}")]
    SetHeaderValue(usize, InvalidHeaderValue),
    #[error("Invalid remove header name at index {0}: {1}")]
    RemoveHeaderName(usize, InvalidHeaderName),
}

impl TryFrom<&HeaderModifier> for HeaderModifierFilter {
    type Error = HeaderModifierFilterConversionError;

    fn try_from(value: &HeaderModifier) -> Result<Self, Self::Error> {
        let mut add = HeaderMap::new();
        if let Some(config) = &value.add {
            for (i, header) in config.iter().enumerate() {
                let name: HeaderName = header
                    .name
                    .parse()
                    .map_err(|e| HeaderModifierFilterConversionError::AddHeaderName(i, e))?;
                let value: HeaderValue = header
                    .value
                    .parse()
                    .map_err(|e| HeaderModifierFilterConversionError::AddHeaderValue(i, e))?;

                add.append(name, value);
            }
        }

        let mut set = HeaderMap::new();
        if let Some(config) = &value.set {
            for (i, header) in config.iter().enumerate() {
                let name: HeaderName = header
                    .name
                    .parse()
                    .map_err(|e| HeaderModifierFilterConversionError::SetHeaderName(i, e))?;
                let value: HeaderValue = header
                    .value
                    .parse()
                    .map_err(|e| HeaderModifierFilterConversionError::SetHeaderValue(i, e))?;

                set.insert(name, value);
            }
        }

        let mut remove = Vec::new();
        if let Some(config) = &value.remove {
            for (i, name_str) in config.iter().enumerate() {
                let name: HeaderName = name_str
                    .parse()
                    .map_err(|e| HeaderModifierFilterConversionError::RemoveHeaderName(i, e))?;

                remove.push(name);
            }
        }

        let filter = Self::builder().add(add).set(set).remove(remove).build();

        Ok(filter)
    }
}
