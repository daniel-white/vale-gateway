use opentelemetry::global::{BoxedTracer, tracer};
use std::sync::LazyLock;

pub(crate) static TRACER: LazyLock<BoxedTracer> = LazyLock::new(|| tracer("vg-gateway"));
