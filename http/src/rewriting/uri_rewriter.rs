use crate::request::RequestMatchContext;
use hickory_proto::rr::Name;
use http::Uri;
use http::uri::{Authority, Scheme};
use typed_builder::TypedBuilder;
use vg_core::net::Port;

#[derive(Debug, PartialEq, Eq)]
pub enum PathRewrite {
    Full(String),
    PrefixMatch(String),
}

#[derive(Debug, PartialEq, Eq, TypedBuilder)]
pub struct UriRewriter {
    #[builder(default, setter(into))]
    scheme: Option<Scheme>,

    #[builder(default, setter(into))]
    host: Option<Name>,

    #[builder(default, setter(into))]
    port: Option<Port>,

    #[builder(default, setter(into))]
    path: Option<PathRewrite>,
}

impl UriRewriter {
    pub fn rewrite(&self, original_uri: &Uri, match_context: &impl RequestMatchContext) -> Uri {
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

        match (&self.path, match_context.path_prefix()) {
            (Some(PathRewrite::Full(new_path)), _) => {
                parts.path_and_query = Some(new_path.parse().unwrap());
            }
            (Some(PathRewrite::PrefixMatch(new_prefix)), Some(matched_prefix)) => {
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
