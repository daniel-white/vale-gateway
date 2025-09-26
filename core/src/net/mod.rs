use std::fmt::Display;
use std::num::NonZeroU16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Port(NonZeroU16);

impl Port {
    // Create a new Port from a u16, returning None if the value is zero (invalid NonZeroU16)
    pub fn new(port: u16) -> Option<Self> {
        NonZeroU16::new(port).map(Self)
    }
    // Access underlying numeric value
    pub fn get(self) -> u16 {
        self.0.get()
    }
}

impl Display for Port {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
