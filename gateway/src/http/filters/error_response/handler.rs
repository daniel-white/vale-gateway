use crate::http::filters::error_response::error_codes::ErrorResponseCode;
use crate::http::filters::error_response::generators::{
    EmptyErrorResponseGenerator, HtmlErrorResponseGenerator, HttpErrorResponseGeneratorType,
    ProblemDetailErrorResponseGenerator, ProblemDetailErrorResponseGeneratorBuilder,
};
use bytes::Bytes;
use http::Response;
use vg_core::http::filters::error_response::{HttpErrorResponseFilter, HttpErrorResponseKind};

#[derive(Debug, PartialEq, Eq, Default)]
pub struct HttpErrorResponseFilterHandler {
    generator: HttpErrorResponseGeneratorType,
}

impl HttpErrorResponseFilterHandler {
    pub fn builder() -> HttpErrorResponseFilterHandlerBuilder {
        let generator = HtmlErrorResponseGenerator::builder().build();
        HttpErrorResponseFilterHandlerBuilder {
            generator: HttpErrorResponseGeneratorType::Html(generator),
        }
    }

    pub fn from(filter: &HttpErrorResponseFilter) -> Self {
        let mut builder = Self::builder();

        match filter.kind() {
            HttpErrorResponseKind::Empty => {
                builder.empty_responses();
            }
            HttpErrorResponseKind::Html => {
                builder.html_responses();
            }
            HttpErrorResponseKind::ProblemDetail => {
                builder.problem_detail_responses(|builder| {
                    if let Some(authority) = filter
                        .problem_detail()
                        .as_ref()
                        .and_then(|pd| pd.authority().clone())
                    {
                        builder.authority(authority);
                    }
                });
            }
        }

        builder.build()
    }

    pub fn generate_response(&self, code: ErrorResponseCode) -> Response<Option<Bytes>> {
        self.generator.generate_response(code)
    }
}

pub struct HttpErrorResponseFilterHandlerBuilder {
    generator: HttpErrorResponseGeneratorType,
}

impl HttpErrorResponseFilterHandlerBuilder {
    pub fn build(self) -> HttpErrorResponseFilterHandler {
        HttpErrorResponseFilterHandler {
            generator: self.generator,
        }
    }

    pub fn html_responses(&mut self) -> &mut Self {
        let generator = HtmlErrorResponseGenerator::builder().build();
        self.generator = HttpErrorResponseGeneratorType::Html(generator);
        self
    }

    pub fn empty_responses(&mut self) -> &mut Self {
        let generator = EmptyErrorResponseGenerator::builder().build();
        self.generator = HttpErrorResponseGeneratorType::Empty(generator);
        self
    }

    pub fn problem_detail_responses<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut ProblemDetailErrorResponseGeneratorBuilder),
    {
        let mut builder = ProblemDetailErrorResponseGenerator::builder();
        factory(&mut builder);
        let generator = builder.build();
        self.generator = HttpErrorResponseGeneratorType::ProblemDetail(generator);
        self
    }
}
