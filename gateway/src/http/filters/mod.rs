pub mod access_control;
pub mod client_addr;
mod controllers;
pub mod error_response;
pub mod handlers;
pub mod headers;
pub mod redirect_response;
pub mod static_response;
pub mod upstream_uri_rewrite;

pub use handlers::{
    HttpFilterHandlers, HttpFilterHandlersDependencies,
};
use http::{HeaderName, HeaderValue};

pub trait HttpRequestMatchContext {
    fn matched_path_prefix(&self) -> Option<&String>;
}

pub trait HttpHeaders {
    fn remove(&mut self, header: &HeaderName);

    fn insert(&mut self, header: HeaderName, value: HeaderValue);

    fn append(&mut self, header: HeaderName, value: HeaderValue);
}

impl HttpHeaders for http::HeaderMap {
    fn remove(&mut self, header: &HeaderName) {
        self.remove(header);
    }

    fn insert(&mut self, header: HeaderName, value: HeaderValue) {
        self.insert(header, value);
    }

    fn append(&mut self, header: HeaderName, value: HeaderValue) {
        self.append(header, value);
    }
}
