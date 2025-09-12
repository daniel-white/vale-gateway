use std::ffi::OsString;
use getset::Getters;
use schemars::{JsonSchema};
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use std::fmt::Display;
use std::num::NonZeroU16;
use std::str::FromStr;

#[derive(
    Validate, Getters, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Hash,
)]
pub struct Port(#[getset(get = "pub")] NonZeroU16);

impl Port {
    pub fn new<P: Into<Port>>(port: P) -> Self {
        port.into()
    }
}

impl From<NonZeroU16> for Port {
    fn from(port: NonZeroU16) -> Self {
        Self(port)
    }
}

impl From<Port> for u16 {
    fn from(port: Port) -> Self {
        port.0.into()
    }
}

impl From<Port> for NonZeroU16 {
    fn from(port: Port) -> Self {
        port.0
    }
}

impl Display for Port {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Port {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<NonZeroU16>() {
            Ok(port) => Ok(Self::new(port)),
            Err(_) => Err(()),
        }
    }
}

impl From<OsString> for Port {
    fn from(s: OsString) -> Self {
        let s = s.to_str().unwrap_or("1");
        s.parse::<Port>().unwrap_or(Port::new(NonZeroU16::new(1).unwrap()))
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::NonZeroU16;
    use rstest::*;
    use serde_valid::Validate;

    // Port tests
    #[rstest]
    #[case(80, true)]
    #[case(443, true)]
    #[case(8080, true)]
    #[case(1, true)]
    #[case(65535, true)]
    fn test_port_creation_valid(#[case] port_num: u16, #[case] should_be_valid: bool) {
        let port_num = NonZeroU16::try_from(port_num).unwrap_or(NonZeroU16::new(1).unwrap());
        let port = Port::new(port_num);
        assert_eq!(port.validate().is_ok(), should_be_valid);
        assert_eq!(port.0, port_num); // Direct field access
    }

    #[test]
    fn test_port_validation_boundaries() {
        let port_min = Port::new(NonZeroU16::try_from(1).unwrap());
        let port_max = Port::new(NonZeroU16::try_from(65535).unwrap());

        assert!(port_min.validate().is_ok());
        assert!(port_max.validate().is_ok());
    }
}
