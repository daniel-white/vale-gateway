use std::sync::Arc;
use getset::{CloneGetters, Getters};
use typed_builder::TypedBuilder;
use vg_config::http::filter::static_response::{Body as BodyConfig, BodyContent as BodyContentConfig};
use vg_core::http::content_type::ContentTypeBuf;

#[derive(Debug, TypedBuilder, Getters)]
pub struct BodyContent {
    #[getset(get = "pub")]
    content_type: ContentTypeBuf,
    #[getset(get = "pub")]
    data: Vec<u8>,
}

impl From<&BodyConfig> for BodyContent {
    fn from(config: &BodyConfig) -> Self {
        let data = match config.content() {
            BodyContentConfig::Text(text) => text.as_bytes(),
            BodyContentConfig::Binary(data) => data.as_slice(),
            BodyContentConfig::Remote(_uri) => {
                unreachable!("unsupported remote!")
            }
        };

        Self::builder()
            .content_type(config.content_type())
            .data(data.to_vec())
            .build()
    }
}