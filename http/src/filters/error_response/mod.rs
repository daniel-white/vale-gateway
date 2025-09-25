mod error_codes;
mod generators;

use bytes::Bytes;
pub use error_codes::*;
pub use generators::*;
use http::Response;
use typed_builder::TypedBuilder;

#[derive(Debug, TypedBuilder)]
pub struct ErrorResponseFilterHandler {
    generator: ErrorResponseGeneratorType,
}

impl ErrorResponseFilterHandler {
    pub fn generate_response(&self, code: ErrorResponseCode) -> Response<Option<Bytes>> {
        self.generator.generate_response(code)
    }
}
