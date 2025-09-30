use crate::api::v1::parameters::listeners::http::filters::static_response::{
    StaticResponseFilterBody, StaticResponseFilterBodyFormat, StaticResponseFilterSpec,
};
use base64ct::{Base64Unpadded, Encoding};
use http::StatusCode;
use http::status::InvalidStatusCode;
use thiserror::Error;
use vg_core::http::content_type::{ContentTypeBuf, ContentTypeConversionError};
use vg_http_config::filters::static_response::{Body, BodyContent, StaticResponseFilter};

#[derive(Debug, Error)]
pub enum StaticResponseFilterConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("`status_code` is invalid")]
    StatusCode(#[from] InvalidStatusCode),
    #[error("Body configuration error: {0}")]
    Body(#[from] BodyConversionError),
}

impl TryFrom<&StaticResponseFilterSpec> for StaticResponseFilter {
    type Error = StaticResponseFilterConversionError;

    fn try_from(value: &StaticResponseFilterSpec) -> Result<Self, Self::Error> {
        let status_code: StatusCode = value.status_code.try_into()?;
        let body = match &value.body {
            Some(body_config) => Some(Body::try_from(body_config)?),
            None => None,
        };

        let filter = Self::builder().status_code(status_code).body(body).build();

        Ok(filter)
    }
}

#[derive(Debug, Error)]
pub enum BodyConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("Invalid content-type: {0}")]
    ContentType(#[from] ContentTypeConversionError),
    #[error("Missing `text` for 'text' format")]
    MissingText,
    #[error("Missing `binary` for 'binary' format")]
    MissingBinary,
    #[error("Invalid base64 encoding: {0}")]
    Base64(#[from] base64ct::Error),
}

impl TryFrom<&StaticResponseFilterBody> for Body {
    type Error = BodyConversionError;

    fn try_from(value: &StaticResponseFilterBody) -> Result<Self, Self::Error> {
        let content_type: ContentTypeBuf = value.content_type.parse()?;
        let content: BodyContent = match (&value.format, &value.text, &value.binary) {
            (StaticResponseFilterBodyFormat::Text, Some(text), None) => text.as_bytes().into(),
            (StaticResponseFilterBodyFormat::Text, None, _) => {
                return Err(BodyConversionError::MissingText);
            }
            (StaticResponseFilterBodyFormat::Binary, None, Some(binary)) => {
                // TODO determine whether or not to store for fetch or not
                let src: String = binary.chars().filter(|c| !c.is_whitespace()).collect();
                let buf = Base64Unpadded::decode_vec(&src)?;
                buf.as_slice().into()
            }
            (StaticResponseFilterBodyFormat::Binary, _, None) => {
                return Err(BodyConversionError::MissingBinary);
            }
            _ => return Err(BodyConversionError::InvalidConfiguration),
        };

        let body = Body::builder()
            .content_type(content_type)
            .content(content)
            .build();

        Ok(body)
    }
}
