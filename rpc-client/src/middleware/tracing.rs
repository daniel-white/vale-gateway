use crate::instrumentation::TRACER;
use jsonrpsee::core::middleware::{Batch, Notification, RpcServiceT};
use jsonrpsee::types::Request;
use opentelemetry::global::get_text_map_propagator;
use opentelemetry::trace::Tracer;
use tower_layer::Layer;
use typed_builder::TypedBuilder;
use vg_rpc::propagation::{ExtensionsExtractor, ExtensionsInjector};

#[derive(Default, Copy, Clone, Debug)]
pub struct TracingLayer;

impl<S> Layer<S> for TracingLayer {
    type Service = Tracing<S>;

    fn layer(&self, service: S) -> Self::Service {
        Tracing::builder().service(service).build()
    }
}

#[derive(Clone, TypedBuilder)]
pub struct Tracing<S> {
    service: S,
}

impl<S> RpcServiceT for Tracing<S>
where
    S: RpcServiceT + Send + Sync + Clone + 'static,
{
    type MethodResponse = S::MethodResponse;
    type NotificationResponse = S::NotificationResponse;
    type BatchResponse = S::BatchResponse;

    fn call<'a>(
        &self,
        mut request: Request<'a>,
    ) -> impl Future<Output = Self::MethodResponse> + Send + 'a {
        let service = self.service.clone();

        async move {
            let span = TRACER.start("call");

            get_text_map_propagator(|propagator| {
                let mut injector = ExtensionsInjector::new(request.extensions_mut());
                propagator.inject(&mut injector);
            });

            service.call(request).await
        }
    }

    fn batch<'a>(
        &self,
        mut requests: Batch<'a>,
    ) -> impl Future<Output = Self::BatchResponse> + Send + 'a {
        let service = self.service.clone();

        async move {
            let span = TRACER.start("call");

            get_text_map_propagator(|propagator| {
                let mut injector = ExtensionsInjector::new(requests.extensions_mut());
                propagator.inject(&mut injector);
            });

            service.batch(requests).await
        }
    }

    fn notification<'a>(
        &self,
        n: Notification<'a>,
    ) -> impl Future<Output = Self::NotificationResponse> + Send + 'a {
        let service = self.service.clone();

        async move {
            let context = get_text_map_propagator(|propagator| {
                let extractor = ExtensionsExtractor::new(n.extensions());
                propagator.extract(&extractor)
            });

            let span = TRACER.start_with_context("notification", &context);
            service.notification(n).await
        }
    }
}
