use vg_http::request::matchers::request::RequestMatcher;

#[test]
pub fn loads_request_matcher_from_file() {
    use vg_http_config::request::matchers::RequestMatcher as RequestMatcherConfig;

    // let header_matcher = HeaderMatcher::builder()
    //     .name(CONTENT_TYPE)
    //     .value(HeaderValueMatcher::Exact(HeaderValue::from_static("application/json")))
    //     .build();
    // let header_matcher2 = HeaderMatcher::builder()
    //     .name(ACCEPT)
    //     .value(HeaderValueMatcher::RegularExpression("^text/.*".to_string()))
    //     .build();
    //
    // let headers = vec![header_matcher, header_matcher2];
    //
    // let p = PathMatcher::RegularExpression("^/api/v1/.*$".to_string());
    //
    //
    // let qp = QueryParamMatcher::builder()
    //     .name("pageSize".into())
    //     .value( QueryParamValueMatcher::RegularExpression("^\\d$".to_string()))
    //     .build();
    //
    // let query_params = vec![qp];
    //
    //
    // let x = RequestMatcher::builder()
    //     .method(Some(MethodMatcher::builder().method( Method::PATCH).build()))
    //     .path(Some(p))
    //     .headers(HeadersMatcher::builder().headers(headers).build())
    //     .query_params(QueryParamsMatcher::builder().query_params(query_params).build())
    //     .build();
    //
    // let config = serde_json::to_string_pretty(&x).unwrap_or_default();
    // println!("{}", config);


    let config = include_str!("request_matcher.json");
    let config = serde_json::from_str::<RequestMatcherConfig>(config)
        .expect("Failed to parse JSON config");

    let config = RequestMatcher::try_from(&config).expect("Failed to convert config");

    println!("{:#?}", config);
}