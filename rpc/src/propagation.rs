use derive_more::{Deref, DerefMut};
use http::{Extensions, HeaderMap};
use opentelemetry::propagation::{Extractor, Injector};
use opentelemetry_http::{HeaderExtractor, HeaderInjector};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Deref, DerefMut, Serialize, Deserialize)]
#[serde(transparent)]
struct TracingPropagationExtension(#[serde(with = "http_serde_ext::header_map")] HeaderMap);

pub struct ExtensionsInjector<'a> {
    injector: HeaderInjector<'a>,
}

impl<'a> ExtensionsInjector<'a> {
    pub fn new(extensions: &'a mut Extensions) -> Self {
        let extension = extensions.get_or_insert_default();
        Self {
            injector: HeaderInjector(extension),
        }
    }
}

impl Injector for ExtensionsInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        self.injector.set(key, value);
    }
}

pub struct ExtensionsExtractor<'a> {
    extractor: Option<HeaderExtractor<'a>>,
}

impl<'a> ExtensionsExtractor<'a> {
    pub fn new(extensions: &'a Extensions) -> Self {
        let extension: Option<&TracingPropagationExtension> = extensions.get();

        let extractor = extension.map(|extension| HeaderExtractor(extension));

        Self { extractor }
    }
}

impl Extractor for ExtensionsExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.extractor.as_ref()?.get(key)
    }

    fn keys(&self) -> Vec<&str> {
        match self.extractor.as_ref() {
            Some(extractor) => extractor.keys(),
            None => Vec::with_capacity(0),
        }
    }
}
