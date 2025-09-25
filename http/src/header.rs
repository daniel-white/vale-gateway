use http::{HeaderName, HeaderValue};

pub trait HeaderModifier {
    fn remove(&mut self, header: &HeaderName);

    fn insert(&mut self, header: &HeaderName, value: &HeaderValue);

    fn append(&mut self, header: &HeaderName, value: &HeaderValue);
}

impl HeaderModifier for http::HeaderMap {
    fn remove(&mut self, header: &HeaderName) {
        self.remove(header);
    }

    fn insert(&mut self, header: &HeaderName, value: &HeaderValue) {
        self.insert(header, value.clone());
    }

    fn append(&mut self, header: &HeaderName, value: &HeaderValue) {
        self.append(header, value.clone());
    }
}

impl HeaderModifier for pingora::http::RequestHeader {
    fn remove(&mut self, header: &HeaderName) {
        self.remove_header(header.as_str());
    }

    fn insert(&mut self, header: &HeaderName, value: &HeaderValue) {
        self.insert_header(header, value)
            .expect("Invalid header value");
    }

    fn append(&mut self, header: &HeaderName, value: &HeaderValue) {
        self.append_header(header, value)
            .expect("Invalid header value");
    }
}

impl HeaderModifier for pingora::http::ResponseHeader {
    fn remove(&mut self, header: &HeaderName) {
        self.remove_header(header.as_str());
    }

    fn insert(&mut self, header: &HeaderName, value: &HeaderValue) {
        self.insert_header(header, value)
            .expect("Invalid header value");
    }

    fn append(&mut self, header: &HeaderName, value: &HeaderValue) {
        self.append_header(header, value)
            .expect("Invalid header value");
    }
}
