pub mod filter;
pub mod policy;
pub mod rewriting;

pub mod backend;
pub mod listener;
pub mod route;

#[cfg(test)]
mod test {
    use crate::http::backend::BackendRef;
    use crate::http::filter::SharedFilterRef;
    use crate::http::filter::static_response::StaticResponseFilterRef;
    use crate::http::listener::policy::ListenerPolicies;
    use crate::http::listener::{
        Listener, ListenerRef, ListenerTransport, ListenerTransportProtocols,
    };
    use crate::http::route::RouteRef;
    use vg_core::net::Port;

    #[test]
    fn should_round_trip() {
        let l = ListenerRef::from("l".to_string());
        let beref = BackendRef::from("be1".to_string());
        let r = RouteRef::from("r".to_string());
        let f = StaticResponseFilterRef::from("f".to_string());
        let f = SharedFilterRef::StaticResponse(f);
        let p = ListenerTransportProtocols::builder()
            .http(Port::HTTP)
            .build();
        let t = ListenerTransport::builder().protocols(p).build();
        let l = Listener::builder()
            .ref_(l.clone())
            .transport(t)
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
