use getset::Getters;
use hickory_proto::rr::{IntoName, Name};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;

#[derive(
    Validate, Getters, Debug, Clone, PartialEq, Hash, Eq, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpHostHeaderMatch {
    #[getset(get = "pub")]
    #[serde(rename = "type")]
    kind: HttpHostHeaderMatchKind,

    #[getset(get = "pub")]
    #[schemars(schema_with = "crate::schemars::dns_name")]
    name: Name,
}

impl HttpHostHeaderMatch {
    pub fn builder() -> HttpHostHeaderMatchBuilder {
        HttpHostHeaderMatchBuilder { result: None }
    }
}

#[derive(Debug)]
pub struct HttpHostHeaderMatchBuilder {
    result: Option<HttpHostHeaderMatch>,
}

impl HttpHostHeaderMatchBuilder {
    pub fn build(self) -> HttpHostHeaderMatch {
        self.result.expect("HttpHostHeaderMatch is not fully built")
    }

    pub fn fully_qualified<N: IntoName>(&mut self, name: N) -> &mut Self {
        let mut name: Name = name.into_name().unwrap();
        name.set_fqdn(true);
        self.result = Some(HttpHostHeaderMatch {
            kind: HttpHostHeaderMatchKind::FullyQualified,
            name,
        });
        self
    }

    pub fn in_zone<Z: IntoName>(&mut self, zone: Z) -> &mut Self {
        let mut zone: Name = zone.into_name().unwrap();
        zone.set_fqdn(false);
        self.result = Some(HttpHostHeaderMatch {
            kind: HttpHostHeaderMatchKind::InZone,
            name: zone,
        });
        self
    }
}

#[derive(Validate, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum HttpHostHeaderMatchKind {
    FullyQualified,
    InZone,
}
