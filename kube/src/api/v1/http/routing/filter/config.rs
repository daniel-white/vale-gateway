use derive_more::{Deref, From};
use gateway_api::httproutes::HTTPRouteFilter;

#[derive(Debug, Deref, From)]
pub struct HTTPRouteFilterWrapper<'a>(&'a HTTPRouteFilter);
