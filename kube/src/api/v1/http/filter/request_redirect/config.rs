use super::RequestRedirect;
use gateway_api::common::{RequestOperationType, RequestRedirectScheme};
use http::StatusCode;
use http::uri::Scheme;
use thiserror::Error;
use vg_core::net::Port;
use vg_http_config::filter::redirect_response::RedirectResponseFilter;
use vg_http_config::rewriting::uri::{PathRewrite, UriRewriter};

#[derive(Debug, Error)]
pub enum RedirectResponseFilterConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("Invalid redirect status code")]
    StatusCode,
    #[error("Path rewrite is invalid")]
    Path,
    #[error("Port rewrite is invalid")]
    Port,
}

impl TryFrom<&RequestRedirect> for RedirectResponseFilter {
    type Error = RedirectResponseFilterConversionError;

    fn try_from(value: &RequestRedirect) -> Result<Self, Self::Error> {
        let status_code = value
            .status_code
            .map(u16::try_from)
            .transpose()
            .map_err(|_| RedirectResponseFilterConversionError::StatusCode)?;
        let status_code: StatusCode = status_code
            .map(StatusCode::from_u16)
            .transpose()
            .map_err(|_| RedirectResponseFilterConversionError::StatusCode)?
            .unwrap_or(StatusCode::FOUND);

        if !status_code.is_redirection() {
            return Err(RedirectResponseFilterConversionError::StatusCode);
        }

        let uri: UriRewriter = value.try_into()?;

        let filter = Self::builder().status_code(status_code).uri(uri).build();

        Ok(filter)
    }
}

impl TryFrom<&RequestRedirect> for UriRewriter {
    type Error = RedirectResponseFilterConversionError;

    fn try_from(value: &RequestRedirect) -> Result<Self, Self::Error> {
        let scheme = value.scheme.as_ref().map(|scheme| match scheme {
            RequestRedirectScheme::Http => Scheme::HTTP,
            RequestRedirectScheme::Https => Scheme::HTTPS,
        });

        let path = value
            .path
            .as_ref()
            .map(|path| {
                match (
                    &path.r#type,
                    path.replace_full_path.clone(),
                    path.replace_prefix_match.clone(),
                ) {
                    (RequestOperationType::ReplaceFullPath, Some(r), None) => {
                        Ok(PathRewrite::Full(r))
                    }
                    (RequestOperationType::ReplacePrefixMatch, None, Some(r)) => {
                        Ok(PathRewrite::PrefixMatch(r))
                    }
                    _ => Err(RedirectResponseFilterConversionError::Path),
                }
            })
            .transpose()?;

        let port = value
            .port
            .map(|port| {
                port.try_into()
                    .map_err(|_| RedirectResponseFilterConversionError::Port)
                    .and_then(|port| {
                        Port::new(port).ok_or(RedirectResponseFilterConversionError::Port)
                    })
            })
            .transpose()?;

        let uri = Self::builder()
            .scheme(scheme)
            .host(value.hostname.clone())
            .port(port)
            .path(path)
            .build();

        Ok(uri)
    }
}
