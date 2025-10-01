use crate::api::v1::parameters::listeners::http::filters::HTTPRouteUrlRewrite;
use gateway_api::common::RequestOperationType;
use thiserror::Error;
use vg_http_config::filters::upstream_uri_rewrite::UpstreamUriRewriteFilter;
use vg_http_config::rewriting::uri::{PathRewrite, UriRewriter};

#[derive(Debug, Error)]
pub enum UpstreamUriRewriteConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("Path rewrite is invalid")]
    Path,
}

impl TryFrom<&HTTPRouteUrlRewrite> for UpstreamUriRewriteFilter {
    type Error = UpstreamUriRewriteConversionError;

    fn try_from(value: &HTTPRouteUrlRewrite) -> Result<Self, Self::Error> {
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
                    _ => Err(UpstreamUriRewriteConversionError::Path),
                }
            })
            .transpose()?;

        let uri = UriRewriter::builder()
            .scheme(None)
            .host(value.hostname.clone())
            .port(None)
            .path(path)
            .build();

        let filter = Self::builder().uri(uri).build();

        Ok(filter)
    }
}
