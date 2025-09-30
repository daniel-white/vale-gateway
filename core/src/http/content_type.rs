use http::HeaderValue;
use http::header::InvalidHeaderValue;
use mediatype::names::{APPLICATION, CHARSET, JSON, TEXT};
use mediatype::values::UTF_8;
use mediatype::{MediaType, MediaTypeBuf, MediaTypeError, Name, Value, names};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::ops::Deref;
use std::str::FromStr;
use thiserror::Error;

const PROBLEM: Name = Name::new_unchecked("problem");

pub const PROBLEM_DETAIL: ContentType =
    ContentType::from_parts(APPLICATION, PROBLEM, Some(JSON), [].as_slice());

pub const HTML: ContentType =
    ContentType::from_parts(TEXT, names::HTML, None, [(CHARSET, UTF_8)].as_slice());

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ContentType<'a>(MediaType<'a>);

impl<'a> ContentType<'a> {
    #[must_use]
    pub const fn new(ty: Name<'a>, subty: Name<'a>) -> Self {
        Self(MediaType::new(ty, subty))
    }

    #[must_use]
    pub const fn from_parts(
        ty: Name<'a>,
        subty: Name<'a>,
        suffix: Option<Name<'a>>,
        params: &'a [(Name<'a>, Value<'a>)],
    ) -> Self {
        Self(MediaType::from_parts(ty, subty, suffix, params))
    }
}

impl<'a> Deref for ContentType<'a> {
    type Target = MediaType<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> TryFrom<ContentType<'a>> for HeaderValue {
    type Error = InvalidHeaderValue;

    fn try_from(value: ContentType<'a>) -> Result<Self, Self::Error> {
        HeaderValue::from_str(&value.0.to_string())
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentTypeBuf(MediaTypeBuf);

impl Deref for ContentTypeBuf {
    type Target = MediaTypeBuf;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<MediaTypeBuf> for ContentTypeBuf {
    fn from(value: MediaTypeBuf) -> Self {
        Self(value)
    }
}

impl From<MediaType<'_>> for ContentTypeBuf {
    fn from(value: MediaType<'_>) -> Self {
        Self(MediaTypeBuf::from(value))
    }
}

impl<'a> From<&'a ContentTypeBuf> for ContentType<'a> {
    fn from(value: &'a ContentTypeBuf) -> Self {
        Self((&value.0).into())
    }
}

impl TryFrom<ContentTypeBuf> for HeaderValue {
    type Error = InvalidHeaderValue;

    fn try_from(value: ContentTypeBuf) -> Result<Self, Self::Error> {
        HeaderValue::from_str(value.as_str())
    }
}

impl TryFrom<&ContentTypeBuf> for HeaderValue {
    type Error = InvalidHeaderValue;

    fn try_from(value: &ContentTypeBuf) -> Result<Self, Self::Error> {
        HeaderValue::from_str(value.as_str())
    }
}

#[derive(Debug, Error, PartialEq, Eq, Clone, Copy)]
pub enum ContentTypeConversionError {
    #[error("Invalid header value")]
    HeaderValue,
    #[error("{0}")]
    MediaType(#[from] MediaTypeError),
}

impl TryFrom<HeaderValue> for ContentTypeBuf {
    type Error = ContentTypeConversionError;

    fn try_from(value: HeaderValue) -> Result<Self, Self::Error> {
        let value = value
            .to_str()
            .map_err(|_| ContentTypeConversionError::HeaderValue)?;
        let value = value.parse()?;
        Ok(value)
    }
}

impl FromStr for ContentTypeBuf {
    type Err = ContentTypeConversionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = MediaTypeBuf::from_string(s.to_string())?;
        Ok(value.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assertables::{assert_err_eq_x, assert_some_eq_x};
    use http::HeaderValue;
    use mediatype::names::{APPLICATION, CHARSET, JSON};
    use mediatype::values::UTF_8;
    use mediatype::{MediaType, MediaTypeBuf, ReadParams};

    #[test]
    fn test_content_type_new_and_from_parts() {
        let ct = ContentType::new(APPLICATION, JSON);
        assert_eq!(ct.ty, APPLICATION);
        assert_eq!(ct.subty, JSON);

        let params = [(CHARSET, UTF_8)];
        let ct2 = ContentType::from_parts(APPLICATION, JSON, None, &params);
        assert_eq!(ct2.ty, APPLICATION);
        assert_eq!(ct2.subty, JSON);
        assert_eq!(ct2.suffix, None);
        let params_vec: Vec<_> = ct.params().collect();
        assert_eq!(params_vec, vec![]);
    }

    #[test]
    fn test_content_type_try_from_to_headervalue() {
        let ct = ContentType::from_parts(APPLICATION, JSON, None, &[(CHARSET, UTF_8)]);
        let hv = HeaderValue::try_from(ct.clone()).unwrap();
        assert_eq!(hv.to_str().unwrap(), ct.to_string());
    }

    #[test]
    fn test_content_type_buf_from_mediatypebuf() {
        let mtb = MediaTypeBuf::from_string("application/json".to_string()).unwrap();
        let ctb: ContentTypeBuf = mtb.clone().into();
        assert_eq!(ctb.0, mtb);
    }

    #[test]
    fn test_content_type_buf_from_mediatype() {
        let mt = MediaType::new(APPLICATION, JSON);
        let ctb: ContentTypeBuf = mt.into();
        assert_eq!(
            ctb.0,
            MediaTypeBuf::from_string("application/json".to_string()).unwrap()
        );
    }

    #[test]
    fn test_content_type_from_content_type_buf() {
        let mtb = MediaTypeBuf::from_string("application/json".to_string()).unwrap();
        let ctb: ContentTypeBuf = mtb.into();
        let ct: ContentType = (&ctb).into();
        assert_eq!(ct.ty, APPLICATION);
        assert_eq!(ct.subty, JSON);
    }

    #[test]
    fn test_headervalue_try_from_content_type_buf() {
        let mtb = MediaTypeBuf::from_string("application/json".to_string()).unwrap();
        let ctb: ContentTypeBuf = mtb.into();
        let hv = HeaderValue::try_from(ctb).unwrap();
        assert_eq!(hv.to_str().unwrap(), "application/json");
    }

    #[test]
    fn test_content_type_buf_try_from_headervalue() {
        let hv = HeaderValue::from_static("application/json");
        let ctb = ContentTypeBuf::try_from(hv).unwrap();
        assert_eq!(
            MediaTypeBuf::from_string("application/json".to_string()).unwrap(),
            ctb.0,
        );
    }

    #[test]
    fn test_content_type_buf_try_from_headervalue_invalid() {
        let hv = HeaderValue::from_static("not a valid media type");
        let err = ContentTypeBuf::try_from(hv);

        assert_err_eq_x!(
            err,
            ContentTypeConversionError::MediaType(MediaTypeError::InvalidTypeName)
        );
    }

    #[test]
    fn test_problem_detail_constant() {
        let ct = PROBLEM_DETAIL;
        assert_eq!(ct.ty, APPLICATION);
        assert_eq!(ct.subty, PROBLEM);
        assert_some_eq_x!(ct.suffix, JSON);
        let params: Vec<_> = ct.params().collect();
        assert_eq!(params, vec![]);
        assert_eq!(ct.to_string(), "application/problem+json");
    }
}
