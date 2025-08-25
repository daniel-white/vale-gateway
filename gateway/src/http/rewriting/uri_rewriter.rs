use hickory_proto::rr::Name;
use http::uri::{Authority, Scheme};
use http::Uri;
use typed_builder::TypedBuilder;
use vg_core::net::Port;

#[derive(Debug, PartialEq, Eq)]
pub enum HttpUriPathRewrite {
    Full(String),
    PrefixMatch(String),
}

#[derive(Debug, Clone, PartialEq, TypedBuilder)]
pub struct HttpUriPathMatch {
    #[builder(setter(into))]
    prefix: Option<String>,
}

#[derive(Debug, PartialEq, Eq, TypedBuilder)]
pub struct HttpUriRewriter {
    #[builder(default, setter(into))]
    scheme: Option<Scheme>,

    #[builder(default, setter(into))]
    host: Option<Name>,

    #[builder(default, setter(into))]
    port: Option<Port>,

    #[builder(default, setter(into))]
    path: Option<HttpUriPathRewrite>,
}

impl HttpUriRewriter {
    pub fn rewrite(&self, original_uri: &Uri, path_match: Option<HttpUriPathMatch>) -> Uri {
        let mut parts = original_uri.clone().into_parts();

        if let Some(scheme) = &self.scheme {
            parts.scheme = Some(scheme.clone());
        }

        match (&self.host, &self.port) {
            (Some(host), Some(port)) => {
                parts.authority = Some(Authority::try_from(format!("{host}:{port}")).unwrap());
            }
            (Some(host), None) => {
                parts.authority = Some(Authority::try_from(host.to_string()).unwrap());
            }
            (None, Some(port)) => {
                if let Some(authority) = &parts.authority {
                    let host = authority.host().to_string();
                    parts.authority = Some(Authority::try_from(format!("{host}:{port}")).unwrap());
                }
            }
            (None, None) => {
                // Noop
            }
        }

        match (
            &self.path,
            path_match.as_ref().and_then(|m| m.prefix.as_ref()),
        ) {
            (Some(HttpUriPathRewrite::Full(new_path)), _) => {
                parts.path_and_query = Some(new_path.parse().unwrap());
            }
            (Some(HttpUriPathRewrite::PrefixMatch(new_prefix)), Some(matched_prefix)) => {
                if let Some(path_and_query) = parts.path_and_query {
                    let suffix =
                        &path_and_query.path()[matched_prefix.len()..].trim_start_matches('/');
                    let new_prefix = new_prefix.trim_end_matches('/');
                    let new_path = format!("/{}/{}", new_prefix, suffix);
                    parts.path_and_query = Some(new_path.parse().unwrap());
                }
            }
            _ => {
                // Noop
            }
        }

        Uri::from_parts(parts).unwrap()
    }
}
