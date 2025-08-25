pub mod configuration;
pub mod ipc;

use enumflags2::{bitflags, BitFlags};
use getset::{CloneGetters, Getters};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use typed_builder::TypedBuilder;

static REGEX_CACHE: Lazy<Mutex<HashMap<String, Regex>>> = Lazy::new(Mutex::default);

pub fn get_regex(pattern: &str) -> Regex {
    let mut map = REGEX_CACHE.lock().expect("Failed to lock regex cache");
    map.entry(pattern.to_string())
        .or_insert_with_key(|p| Regex::new(p).expect("Failed to compile regex"))
        .clone()
}

#[derive(Debug, TypedBuilder, Getters, CloneGetters)]
pub struct InstanceContext {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    pod_name: String,

    #[getset(get = "pub")]
    #[builder(setter(into))]
    gateway_namespace: String,

    #[getset(get = "pub")]
    #[builder(setter(into))]
    gateway_name: String,

    #[getset(get_clone = "pub")]
    location: Arc<TopologyLocation>,
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
        if lhs.zone == rhs.zone {
            score |= Self::Zone;
        }
        if lhs.node == rhs.node {
            score |= Self::Node;
        }
        score
    }
}

#[derive(Default, Getters, Debug, PartialEq, Eq, TypedBuilder)]
pub struct TopologyLocation {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    node: Option<String>,

    #[getset(get = "pub")]
    #[builder(setter(into))]
    zone: Option<String>,
}
