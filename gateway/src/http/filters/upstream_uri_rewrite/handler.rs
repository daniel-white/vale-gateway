use crate::http::filters::HttpRequestMatchContext;
use crate::http::rewriting::uri_rewriter::{HttpUriPathMatch, HttpUriPathRewrite, HttpUriRewriter};
use http::request::Parts;
use http::Uri;
use typed_builder::TypedBuilder;
use vg_core::http::filters::upstream_uri_rewrite::{
    HttpUpstreamUriPathRewrite, HttpUpstreamUriRewriteFilter,
};

#[derive(Debug, PartialEq, Eq, TypedBuilder)]
pub struct HttpUpstreamUriRewriteFilterHandler {
    uri_rewriter: HttpUriRewriter,
}

impl HttpUpstreamUriRewriteFilterHandler {
    pub fn from(filter: &HttpUpstreamUriRewriteFilter) -> Self {
        let path = filter.path().as_ref().map(|path| match path {
            HttpUpstreamUriPathRewrite::Full(path) => HttpUriPathRewrite::Full(path.clone()),
            HttpUpstreamUriPathRewrite::PrefixMatch(path) => {
                HttpUriPathRewrite::PrefixMatch(path.clone())
            }
        });

        let uri_rewriter = HttpUriRewriter::builder()
            .host(filter.host().clone())
            .path(path)
            .build();

        Self::builder().uri_rewriter(uri_rewriter).build()
    }

    pub fn handle(&self, req: &Parts, match_context: &impl HttpRequestMatchContext) -> Uri {
        let path_match = HttpUriPathMatch::builder()
            .prefix(match_context.matched_path_prefix().cloned())
            .build();

        self.uri_rewriter.rewrite(&req.uri, Some(path_match))
    }
}
