use gateway_api::httproutes::HTTPRouteRulesFiltersExtensionRef;
use http::{HeaderValue, StatusCode};
use vg_api::v1alpha1::ErrorResponses;
use vg_core::http::filters::error_response::HttpErrorResponseFilterRef;
use vg_core::http::filters::static_response::{HttpStaticResponseBody, HttpStaticResponseFilter};

pub fn convert_error_response_ref(
    extension_ref: &HTTPRouteRulesFiltersExtensionRef,
) -> HttpErrorResponseFilterRef {
    HttpErrorResponseFilterRef::builder()
        .key(&extension_ref.name)
        .build()
}

pub fn convert_error_response(filter: &ErrorResponses) -> HttpStaticResponseFilter {
    let metadata = &filter.metadata;
    let name: &str = metadata.name.as_deref().unwrap();
    let resource_version = metadata.resource_version.as_deref().unwrap();
    let filter = &filter.spec;

    let status_code = StatusCode::from_u16(filter.status_code).unwrap();

    let body = filter.body.as_ref().map(|body| {
        let key = format!("{}-{}", name, resource_version);
        let content_type = HeaderValue::from_str(body.content_type.as_str()).unwrap();

        HttpStaticResponseBody::builder()
            .key(key)
            .content_type(content_type)
            .build()
    });

    HttpStaticResponseFilter::builder()
        .key(name)
        .status_code(status_code)
        .body(body)
        .build()
}
