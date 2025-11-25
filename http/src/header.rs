use http::{HeaderName, HeaderValue};

pub trait HeaderModifier {
    fn remove(&mut self, key: &HeaderName);

    fn insert(&mut self, key: &HeaderName, value: &HeaderValue);

    fn append(&mut self, key: &HeaderName, value: &HeaderValue);
}

impl HeaderModifier for http::HeaderMap {
    fn remove(&mut self, key: &HeaderName) {
        self.remove(key);
    }

    fn insert(&mut self, key: &HeaderName, value: &HeaderValue) {
        self.insert(key, value.clone());
    }

    fn append(&mut self, key: &HeaderName, value: &HeaderValue) {
        self.append(key, value.clone());
    }
}

impl HeaderModifier for pingora::http::RequestHeader {
    fn remove(&mut self, header: &HeaderName) {
        self.remove_header(header.as_str());
    }

    fn insert(&mut self, header: &HeaderName, value: &HeaderValue) {
        self.insert_header(header, value).expect("Invalid header value");
    }

    fn append(&mut self, header: &HeaderName, value: &HeaderValue) {
        self.append_header(header, value).expect("Invalid header value");
    }
}

impl HeaderModifier for pingora::http::ResponseHeader {
    fn remove(&mut self, header: &HeaderName) {
        self.remove_header(header.as_str());
    }

    fn insert(&mut self, header: &HeaderName, value: &HeaderValue) {
        self.insert_header(header, value).expect("Invalid header value");
    }

    fn append(&mut self, header: &HeaderName, value: &HeaderValue) {
        self.append_header(header, value).expect("Invalid header value");
    }
}
