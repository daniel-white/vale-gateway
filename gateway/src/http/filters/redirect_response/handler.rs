use crate::http::filters::HttpRequestMatchContext;
use crate::http::rewriting::uri_rewriter::{HttpUriPathMatch, HttpUriPathRewrite, HttpUriRewriter};
use http::header::LOCATION;
use http::request::Parts;
use http::{Response, StatusCode};
use typed_builder::TypedBuilder;
use vg_core::http::filters::redirect_response::{
    HttpRedirectResponseFilter, HttpRedirectResponsePathRewrite,
};

#[derive(Debug, PartialEq, Eq, TypedBuilder)]
pub struct HttpRedirectResponseFilterHandler {
    #[builder(setter(into))]
    status_code: StatusCode,

    uri_rewriter: HttpUriRewriter,
}

impl HttpRedirectResponseFilterHandler {
    pub fn from(filter: &HttpRedirectResponseFilter) -> Self {
        let path = filter.path().as_ref().map(|path| match path {
            HttpRedirectResponsePathRewrite::Full(path) => HttpUriPathRewrite::Full(path.clone()),
            HttpRedirectResponsePathRewrite::PrefixMatch(path) => {
                HttpUriPathRewrite::PrefixMatch(path.clone())
            }
        });

        let uri_rewriter = HttpUriRewriter::builder()
            .scheme(filter.scheme().clone())
            .host(filter.host().clone())
            .port(*filter.port())
            .path(path)
            .build();

        let status_code: StatusCode = (*filter.kind()).into();

        Self::builder()
            .status_code(status_code)
            .uri_rewriter(uri_rewriter)
            .build()
    }

    pub fn handle(
        &self,
        req: &Parts,
        match_context: Option<&impl HttpRequestMatchContext>,
    ) -> Response<()> {
        let path_match = match_context
            .and_then(|match_context| match_context.matched_path_prefix())
            .map(|matched_path_prefix| {
                HttpUriPathMatch::builder()
                    .prefix(matched_path_prefix.clone())
                    .build()
            });

        let new_uri = self.uri_rewriter.rewrite(&req.uri, path_match);
        Response::builder()
            .status(self.status_code)
            .header(LOCATION, new_uri.to_string())
            .body(())
            .unwrap()
    }
}
