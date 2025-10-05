use gateway_api::common::{HTTPRouteUrlRewrite, RequestOperationType};
use thiserror::Error;
use vg_core::internal_wrapper;
use vg_http_config::filter::backend_uri_rewriter::BackendUriRewriterFilter;
use vg_http_config::rewriting::uri::{PathRewrite, UriRewriter};

#[derive(Debug, Error)]
pub enum BackendUriRewriterConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("Path rewrite is invalid")]
    Path,
}

impl TryFrom<HTTPRouteUrlRewriteWrapper<'_>> for BackendUriRewriterFilter {
    type Error = BackendUriRewriterConversionError;

    fn try_from(value: HTTPRouteUrlRewriteWrapper) -> Result<Self, Self::Error> {
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
                        Ok(PathRewrite::ReplaceWith(r))
                    }
                    (RequestOperationType::ReplacePrefixMatch, None, Some(r)) => {
                        Ok(PathRewrite::ReplacePrefixWith(r))
                    }
                    _ => Err(BackendUriRewriterConversionError::Path),
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

internal_wrapper!(HTTPRouteUrlRewrite);
