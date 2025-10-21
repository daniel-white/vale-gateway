use opentelemetry::global::{BoxedTracer, meter, tracer};
use opentelemetry::metrics::Meter;
use std::sync::LazyLock;

pub(crate) static TRACER: LazyLock<BoxedTracer> = LazyLock::new(|| tracer("vg-rpc-server"));
#[allow(dead_code)]
pub(crate) static METER: LazyLock<Meter> = LazyLock::new(|| meter("vg-rpc-server"));
