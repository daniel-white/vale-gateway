use crate::request::RequestMatchContext;
use crate::rewriting::uri_rewriter::UriRewriter;
use http::Uri;
use http::request::Parts;
use typed_builder::TypedBuilder;

#[derive(Debug, TypedBuilder)]
pub struct UpstreamUriRewriteFilterHandler {
    uri_rewriter: UriRewriter,
}

impl UpstreamUriRewriteFilterHandler {
    pub fn handle(&self, req: &Parts, match_context: &impl RequestMatchContext) -> Uri {
        self.uri_rewriter.rewrite(&req.uri, match_context)
    }
}
