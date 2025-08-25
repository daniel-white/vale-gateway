use std::collections::HashSet;
use http::{HeaderMap, HeaderName};
use typed_builder::TypedBuilder;
use vg_core::http::filters::header_modifier::HttpHeaderModifierFilter;
use crate::http::filters::HttpHeaders;

#[derive(Debug, PartialEq, Eq, TypedBuilder)]
pub struct HttpHeaderModifierFilterHandler {
    add: HeaderMap,
    set: HeaderMap,
    remove: HashSet<HeaderName>,
}

impl HttpHeaderModifierFilterHandler {
    pub fn from(filter: &HttpHeaderModifierFilter) -> Self {
        Self::builder()
            .add(filter.add().clone())
            .set(filter.set().clone())
            .remove(filter.remove().clone())
            .build()
    }
    
    
    pub fn apply(&self, headers: &mut impl HttpHeaders) {
        // Remove headers first
        for name in &self.remove {
            headers.remove(name);
        }
        
        // Set headers (overwrite existing)
        for (name, value) in &self.set {
            headers.insert(name.clone(), value.clone());
        }
        
        // Add headers (append to existing)
        for (name, value) in &self.add {
            headers.append(name.clone(), value.clone());
        }
    }
}