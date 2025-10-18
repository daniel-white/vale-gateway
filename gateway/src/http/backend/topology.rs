use enumflags2::BitFlags;
use getset::Getters;
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend as BackendConfig, BackendRef};
use vg_core::net::topology::{TopologyLocation, TopologyLocationMatch};



