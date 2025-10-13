use opentelemetry::global::{meter, set_text_map_propagator, set_tracer_provider};
use opentelemetry::metrics::Meter;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use std::sync::LazyLock;
use opentelemetry_sdk::trace::TracerProviderBuilder;

pub(crate) static METER: LazyLock<Meter> = LazyLock::new(|| meter("vg-core"));

pub fn init() {
    let tracer_provider = TracerProviderBuilder::default()
        .build();

    set_tracer_provider(tracer_provider);
    set_text_map_propagator(TraceContextPropagator::new());
}
