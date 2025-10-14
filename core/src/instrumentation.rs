use opentelemetry::global::{meter, set_text_map_propagator, set_tracer_provider};
use opentelemetry::metrics::Meter;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::TracerProviderBuilder;
use opentelemetry_stdout::SpanExporter;
use std::sync::LazyLock;
use opentelemetry_sdk::trace::Sampler::AlwaysOn;

pub(crate) static METER: LazyLock<Meter> = LazyLock::new(|| meter("vg-core"));

pub fn init() {
    let exporter = SpanExporter::default(); // Creates a default stdout exporter

    let tracer_provider = TracerProviderBuilder::default()
        .with_simple_exporter(exporter)
        .with_sampler(AlwaysOn)
        .build();

    set_tracer_provider(tracer_provider);
    set_text_map_propagator(TraceContextPropagator::new());
}
