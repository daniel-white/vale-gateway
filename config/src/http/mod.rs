pub mod filter;
pub mod policy;
pub mod rewriting;

pub mod backend;
pub mod listener;
pub mod route;


#[cfg(test)]
mod test {
    use serde_with::json;
    use crate::http::backend::BackendRef;
    use crate::http::filter::SharedFilterRef;
    use crate::http::filter::static_response::StaticResponseFilterRef;
    use crate::http::listener::{Listener, ListenerRef};
    use crate::http::listener::policy::ListenerPolicies;
    use crate::http::route::RouteRef;

    #[test]
    fn should_round_trip() {
        let l = ListenerRef::from("l".to_string());
        let beref = BackendRef::from("be1".to_string());
        let r = RouteRef::from("r".to_string());
        let f = StaticResponseFilterRef::from("f".to_string());
        let f = SharedFilterRef::StaticResponse(f);
        let l = Listener::builder()
            .ref_(l.clone())
            .policies(ListenerPolicies::default())
            .backend_refs(vec![beref])
            .route_refs(vec![r])
            .shared_filter_refs(vec![f])
            .filters(Vec::new())
            .build();
        
        let json = serde_json::to_string(&l).unwrap();
        println!("{}", json);
    }
}