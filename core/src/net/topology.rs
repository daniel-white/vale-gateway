use enumflags2::{BitFlag, BitFlags, bitflags};
use getset::Getters;
use typed_builder::TypedBuilder;

#[derive(Default, Getters, Clone, Debug, PartialEq, Eq, TypedBuilder)]
pub struct TopologyLocation {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    node: Option<String>,

    #[getset(get = "pub")]
    #[builder(setter(into))]
    zone: Option<String>,
}

#[bitflags]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TopologyLocationMatch {
    Zone = 1 << 0,
    Node = 1 << 1,
}

impl TopologyLocationMatch {
    pub fn matches(lhs: &TopologyLocation, rhs: &TopologyLocation) -> BitFlags<Self> {
        let mut score = BitFlags::empty();
        if lhs.zone.is_some() && lhs.zone == rhs.zone {
            score |= Self::Zone;
        }
        if lhs.node.is_some() && lhs.node == rhs.node {
            score |= Self::Node;
        }
        score
    }
}
