use gateway_api::common::{HTTPRouteUrlRewrite, RequestOperationType};
use thiserror::Error;
use vg_config::http::filter::backend_uri_rewriter::BackendUriRewriterFilter;
use vg_config::http::rewriting::uri::{PathRewrite, UriRewriter};
use vg_core::internal_wrapper;

#[derive(Debug, Error)]
pub enum BackendUriRewriterConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("Path rewrite is invalid")]
    Path,
}

internal_wrapper!(HTTPRouteUrlRewrite);

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
