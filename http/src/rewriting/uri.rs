use crate::route::rule::matcher::RequestMatchDetails;
use hickory_proto::ProtoError;
use hickory_proto::rr::Name;
use http::Uri;
use http::uri::{Authority, Scheme};
use std::str::FromStr;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::rewriting::uri::{
    PathRewrite as PathRewriteConfig, UriRewriter as UriRewriterConfig,
};
use vg_core::net::Port;

#[derive(Debug, PartialEq, Eq)]
pub enum PathRewrite {
    Full(String),
    PrefixMatch(String),
}

impl From<PathRewriteConfig> for PathRewrite {
    fn from(value: PathRewriteConfig) -> Self {
        match value {
            PathRewriteConfig::ReplaceWith(value) => PathRewrite::Full(value),
            PathRewriteConfig::ReplacePrefixWith(value) => PathRewrite::PrefixMatch(value),
        }
    }
}

#[derive(Debug, TypedBuilder)]
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

#[derive(Debug, Error)]
pub enum UriRewriterConversionError {
    #[error("No fields set in UriRewriter")]
    Unset,
    #[error("Invalid host: {0}")]
    InvalidHost(
        #[from]
        #[source]
        ProtoError,
    ),
}

impl TryFrom<&UriRewriterConfig> for UriRewriter {
    type Error = UriRewriterConversionError;

    fn try_from(value: &UriRewriterConfig) -> Result<Self, Self::Error> {
        if value.is_unset() {
            return Err(UriRewriterConversionError::Unset);
        }

        let host = match value.host() {
            Some(host) => Some(Name::from_str(host)?),
            None => None,
        };

        let rewriter = Self::builder()
            .scheme(value.scheme())
            .host(host)
            .port(value.port())
            .path(value.path().map(PathRewrite::from))
            .build();

        Ok(rewriter)
    }
}

impl UriRewriter {
    fn apply_full_path_rewrite(original_parts: &http::uri::Parts, new_path: &str) -> String {
        // Preserve original query if new_path doesn't specify one
        let original_query = original_parts
            .path_and_query
            .as_ref()
            .and_then(|pq| pq.query().map(|q| q.to_string()));
        if new_path.contains('?') {
            new_path.to_string()
        } else if let Some(q) = original_query {
            format!("{}?{}", new_path, q)
        } else {
            new_path.to_string()
        }
    }

    fn apply_prefix_rewrite(
        original_parts: &http::uri::Parts,
        matched_prefix: &str,
        new_prefix: &str,
    ) -> Option<String> {
        let path_and_query = original_parts.path_and_query.as_ref()?;
        let original_path = path_and_query.path();
        // Ensure the path actually starts with the matched prefix; otherwise no-op
        if !original_path.starts_with(matched_prefix) {
            return None;
        }
        let mut suffix = &original_path[matched_prefix.len()..];
        // Trim leading slashes from suffix to avoid duplicates unless suffix should be root
        suffix = suffix.trim_start_matches('/');

        let normalized_new = new_prefix.trim_matches('/');
        let new_path = if normalized_new.is_empty() {
            if suffix.is_empty() {
                "/".to_string()
            } else {
                format!("/{}", suffix)
            }
        } else if suffix.is_empty() {
            format!("/{}/", normalized_new)
        } else {
            format!("/{}/{}", normalized_new, suffix)
        };
        Some(new_path)
    }

    pub fn rewrite(&self, original_uri: &Uri, match_context: &impl RequestMatchDetails) -> Uri {
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
            (None, None) => {}
        }

        match (&self.path, match_context.path_prefix()) {
            (Some(PathRewrite::Full(new_path)), _) => {
                let final_path = Self::apply_full_path_rewrite(&parts, new_path);
                parts.path_and_query = Some(final_path.parse().unwrap());
            }
            (Some(PathRewrite::PrefixMatch(new_prefix)), Some(matched_prefix)) => {
                if let Some(new_path) =
                    Self::apply_prefix_rewrite(&parts, matched_prefix.as_str(), new_prefix)
                {
                    // Note: prefix rewriting drops query (documented by tests)
                    parts.path_and_query = Some(new_path.parse().unwrap());
                }
            }
            _ => {}
        }

        Uri::from_parts(parts).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::str::FromStr;

    struct MockMatchContext {
        prefix: Option<String>,
    }
    impl RequestMatchDetails for MockMatchContext {
        fn path_prefix(&self) -> Option<String> {
            self.prefix.clone()
        }
    }

    fn name(host: &str) -> Name {
        if let Ok(n) = Name::from_str(host) {
            return n;
        }
        Name::from_str(&format!("{host}.")).unwrap()
    }

    // ---------- Full path rewriting tests ----------
    #[test]
    fn full_path_rewrite_overrides_existing_query() {
        let r = UriRewriter::builder()
            .path(PathRewrite::Full("/dest?b=2".into()))
            .build();
        let original = Uri::from_str("http://h/svc?a=1").unwrap();
        let ctx = MockMatchContext { prefix: None };
        let out = r.rewrite(&original, &ctx);
        assert_eq!(out.path_and_query().unwrap().as_str(), "/dest?b=2");
    }

    #[rstest]
    #[case("http://h/a?x=1&y=2", "/new", "/new?x=1&y=2")]
    #[case("http://h/a", "/new", "/new")]
    #[case("http://h/a?x=1", "/new/?", "/new/?")]
    #[case("http://h/a?x=1", "/new?p=9", "/new?p=9")]
    fn full_path_query_preservation(
        #[case] original: &str,
        #[case] new_path: &str,
        #[case] expected: &str,
    ) {
        let r = UriRewriter::builder()
            .path(PathRewrite::Full(new_path.into()))
            .build();
        let ctx = MockMatchContext { prefix: None };
        let out = r.rewrite(&Uri::from_str(original).unwrap(), &ctx);
        assert_eq!(out.path_and_query().unwrap().as_str(), expected);
    }

    // ---------- Scheme/host/port rewrites ----------
    #[test]
    fn scheme_only() {
        let r = UriRewriter::builder().scheme(Scheme::HTTPS).build();
        let u = Uri::from_str("http://h/p?q=1").unwrap();
        let out = r.rewrite(&u, &MockMatchContext { prefix: None });
        assert_eq!(out.scheme_str(), Some("https"));
        assert_eq!(out.authority().unwrap().as_str(), "h");
        assert_eq!(out.path_and_query().unwrap().as_str(), "/p?q=1");
    }

    #[test]
    fn host_only() {
        let r = UriRewriter::builder().host(name("x.test.")).build();
        let u = Uri::from_str("http://old:8080/p").unwrap();
        let out = r.rewrite(&u, &MockMatchContext { prefix: None });
        assert_eq!(out.authority().unwrap().as_str(), "x.test.");
    }

    #[test]
    fn port_only_adds_port() {
        let r = UriRewriter::builder()
            .port(Port::try_from(8443).unwrap())
            .build();
        let u = Uri::from_str("http://svc/path").unwrap();
        let out = r.rewrite(&u, &MockMatchContext { prefix: None });
        assert_eq!(out.authority().unwrap().as_str(), "svc:8443");
    }

    #[test]
    fn host_and_port() {
        let r = UriRewriter::builder()
            .host(name("api."))
            .port(Port::try_from(9000).unwrap())
            .build();
        let u = Uri::from_str("http://old:80/v1").unwrap();
        let out = r.rewrite(&u, &MockMatchContext { prefix: None });
        assert_eq!(out.authority().unwrap().as_str(), "api.:9000");
    }

    #[rstest]
    #[case(
        "http://h:8080/p",
        Some(Scheme::HTTPS),
        None,
        None,
        "h:8080",
        Some("https")
    )]
    #[case(
        "http://h:8080/p",
        None,
        Some("new."),
        None,
        "new.",
        Some("configuration")
    )]
    #[case(
        "http://h:8080/p",
        None,
        None,
        Some(9001),
        "h:9001",
        Some("configuration")
    )]
    #[case(
        "http://h:8080/p",
        Some(Scheme::HTTPS),
        Some("n."),
        Some(9002),
        "n.:9002",
        Some("https")
    )]
    fn combined_authority_mods(
        #[case] original: &str,
        #[case] scheme: Option<Scheme>,
        #[case] host: Option<&str>,
        #[case] port: Option<u16>,
        #[case] expected_authority: &str,
        #[case] expected_scheme: Option<&str>,
    ) {
        let r = UriRewriter {
            scheme,
            host: host.map(name),
            port: port.map(Port::try_from).transpose().unwrap(),
            path: None,
        };
        let out = r.rewrite(
            &Uri::from_str(original).unwrap(),
            &MockMatchContext { prefix: None },
        );
        assert_eq!(out.authority().unwrap().as_str(), expected_authority);
        assert_eq!(out.scheme_str(), expected_scheme);
    }

    // ---------- Prefix rewriting tests ----------
    #[rstest]
    #[case("/api/v1/users/123", "/api/v1", "/service", "/service/users/123")]
    #[case("/api/v1/users/123", "/api/v1", "/service/", "/service/users/123")]
    #[case("/api/v1", "/api/v1", "/svc", "/svc/")]
    #[case("/api/v1/", "/api/v1", "/svc", "/svc/")]
    #[case("/api/v1//users//123", "/api/v1", "/svc", "/svc/users//123")]
    #[case("/api/v1", "/api/v1", "/", "/")] // removing prefix entirely
    #[case("/api/v1/", "/api/v1", "/", "/")] // trailing slash normalization
    #[case("/api/v1", "/api/v1", "///svc///", "/svc/")] // excessive slashes in new prefix
    #[case("/", "/", "/root", "/root/")] // root path to new segment
    #[case("/", "/", "/", "/")] // root to root
    fn prefix_rewrite_variations(
        #[case] original_path: &str,
        #[case] matched_prefix: String,
        #[case] new_prefix: &str,
        #[case] expected_path: &str,
    ) {
        let r = UriRewriter::builder()
            .path(PathRewrite::PrefixMatch(new_prefix.into()))
            .build();
        let u = Uri::from_str(&format!("http://h{original_path}")).unwrap();
        let ctx = MockMatchContext {
            prefix: Some(matched_prefix),
        };
        let out = r.rewrite(&u, &ctx);
        assert_eq!(out.path(), expected_path);
    }

    #[test]
    fn prefix_rewrite_mismatched_prefix_is_noop() {
        let r = UriRewriter::builder()
            .path(PathRewrite::PrefixMatch("/svc".into()))
            .build();
        let u = Uri::from_str("http://h/api/users").unwrap();
        let ctx = MockMatchContext {
            prefix: Some("/different".to_string()),
        };
        let out = r.rewrite(&u, &ctx);
        assert_eq!(out.path(), "/api/users"); // unchanged
    }

    #[test]
    fn prefix_rewrite_drops_query_canary() {
        let r = UriRewriter::builder()
            .path(PathRewrite::PrefixMatch("/svc".into()))
            .build();
        let u = Uri::from_str("http://h/api/v1/users?id=10&debug=true").unwrap();
        let ctx = MockMatchContext {
            prefix: Some("/api/v1".to_string()),
        };
        let out = r.rewrite(&u, &ctx);
        assert_eq!(out.path(), "/svc/users");
        assert!(out.query().is_none());
    }

    #[test]
    fn no_op_when_no_configuration() {
        let r = UriRewriter::builder().build();
        let u = Uri::from_str("http://h/path/one?x=1").unwrap();
        let out = r.rewrite(
            &u,
            &MockMatchContext {
                prefix: Some("/api".to_string()),
            },
        );
        assert_eq!(out, u);
    }
}
