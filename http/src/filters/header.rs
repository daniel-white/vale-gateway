use crate::header::HeaderModifier;
use http::{HeaderMap, HeaderName};
use std::collections::HashSet;
use typed_builder::TypedBuilder;

#[derive(Debug, TypedBuilder)]
pub struct HeaderModifierFilterHandler {
    add: HeaderMap,
    set: HeaderMap,
    remove: HashSet<HeaderName>,
}

impl HeaderModifierFilterHandler {
    pub fn apply(&self, modifier: &mut impl HeaderModifier) {
        // Remove headers first
        for name in &self.remove {
            modifier.remove(name);
        }

        // Set headers (overwrite existing)
        for (name, value) in &self.set {
            modifier.insert(name, value);
        }

        // Add headers (append to existing)
        for (name, value) in &self.add {
            modifier.append(name, value);
        }
    }
}
