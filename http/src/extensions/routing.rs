use crate::route::rule::matcher::RequestMatchDetails;
use getset::Getters;
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, TypedBuilder, Getters)]
pub struct RequestMatch {
    #[getset(get = "pub")]
    path_prefix: Option<String>,
}

impl RequestMatchDetails for RequestMatch {
    fn path_prefix(&self) -> Option<String> {
        self.path_prefix.clone()
    }
}
