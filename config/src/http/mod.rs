pub mod filter;
pub mod policy;
pub mod rewriting;

pub mod backend;
pub mod gateway;
pub mod listener;
pub mod route;

#[cfg(test)]
mod test {
    use crate::http::backend::BackendRef;
    use crate::http::filter::SharedFilterRef;
    use crate::http::filter::static_response::StaticResponseFilterRef;
    use crate::http::gateway::{Gateway, GatewayFilter, GatewayRef, ListenerProtocol};
    use crate::http::listener::policy::ListenerPolicies;
    use crate::http::listener::{
        Listener, ListenerRef, ListenerTransport, ListenerTransportProtocols,
    };
    use crate::http::route::RouteRef;
    use std::sync::Arc;
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

    #[test]
    fn should_create_gateway_with_listeners() {
        let gateway_ref = GatewayRef::from("test-gateway".to_string());
        let backend_ref = BackendRef::from("test-backend".to_string());

        let listener_ref = ListenerRef::from("test-listener".to_string());
        let protocols = ListenerTransportProtocols::builder()
            .http(Port::HTTP)
            .build();
        let transport = ListenerTransport::builder().protocols(protocols).build();
        let listener = Listener::builder()
            .ref_(listener_ref)
            .transport(transport)
            .policies(ListenerPolicies::default())
            .backend_refs(vec![])
            .route_refs(vec![])
            .shared_filter_refs(vec![])
            .filters(vec![])
            .build();

        let gateway_filter = GatewayFilter {
            name: "test-filter".to_string(),
        };

        let shared_filter_ref = StaticResponseFilterRef::from("shared-filter".to_string());
        let shared_filter = SharedFilterRef::StaticResponse(shared_filter_ref);

        let gateway = Gateway::builder()
            .ref_(gateway_ref.clone())
            .listeners(vec![Arc::new(listener)])
            .filters(vec![gateway_filter])
            .shared_filter_refs(vec![shared_filter])
            .backend_refs(vec![backend_ref])
            .build();

        assert_eq!(gateway.ref_(), gateway_ref);
        assert_eq!(gateway.listeners().len(), 1);
        assert_eq!(gateway.filters().len(), 1);
        assert_eq!(gateway.shared_filter_refs().len(), 1);
        assert_eq!(gateway.backend_refs().len(), 1);

        // Test serialization - just verify it can serialize and deserialize
        let json = serde_json::to_string(&gateway).unwrap();
        let deserialized: Gateway = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.ref_(), gateway_ref);
        assert_eq!(deserialized.listeners().len(), 1);
        assert_eq!(deserialized.filters().len(), 1);
        assert_eq!(deserialized.shared_filter_refs().len(), 1);
        assert_eq!(deserialized.backend_refs().len(), 1);
    }

    #[test]
    fn should_create_listener_protocol() {
        let protocol = ListenerProtocol::HTTP;
        let json = serde_json::to_string(&protocol).unwrap();
        assert_eq!(json, "\"hTTP\"");

        let deserialized: ListenerProtocol = serde_json::from_str(&json).unwrap();
        assert_eq!(protocol, deserialized);
    }
}
