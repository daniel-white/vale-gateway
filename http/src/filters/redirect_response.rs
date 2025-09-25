use crate::request::RequestMatchContext;
use crate::rewriting::uri_rewriter::UriRewriter;
use http::header::LOCATION;
use http::request::Parts;
use http::{Response, StatusCode};
use typed_builder::TypedBuilder;

#[derive(Debug, TypedBuilder)]
pub struct RedirectResponseFilterHandler {
    #[builder(setter(into))]
    status_code: StatusCode,
    uri_rewriter: UriRewriter,
}

impl RedirectResponseFilterHandler {
    pub fn handle(&self, req: &Parts, match_context: &impl RequestMatchContext) -> Response<()> {
        let new_uri = self.uri_rewriter.rewrite(&req.uri, match_context);
        Response::builder()
            .status(self.status_code)
            .header(LOCATION, new_uri.to_string())
            .body(())
            .unwrap()
    }
}
