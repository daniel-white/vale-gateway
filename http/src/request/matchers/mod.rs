pub mod basic;
pub mod header;
pub mod host_header;
pub mod method;
pub mod path;
pub mod query_param;
pub mod request;
pub mod scoring;

use http::request::Parts;
use scoring::RequestMatcherScorer;

trait Matcher {
    fn matches(&self, score: &RequestMatcherScorer, req: &Parts) -> bool;
}

pub trait RequestMatchDetails {
    fn path_prefix(&self) -> Option<String>;
}
