use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GatewayInstrumentation {
    pub log_level: GatewayInstrumentationLogLevel,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_telemetry: Option<GatewayInstrumentationOpenTelemetry>,
}

#[derive(Deserialize, Serialize, Copy, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum GatewayInstrumentationLogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GatewayInstrumentationOpenTelemetry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collector: Option<GatewayInstrumentationOpenTelemetryCollector>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exporter: Option<GatewayInstrumentationOpenTelemetryExporter>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling: Option<GatewayInstrumentationOpenTelemetrySampling>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GatewayInstrumentationOpenTelemetryCollector {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GatewayInstrumentationOpenTelemetryExporter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GatewayInstrumentationOpenTelemetrySampling {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling_type: Option<GatewayInstrumentationOpenTelemetrySamplingType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_based: Option<GatewayInstrumentationOpenTelemetryParentBased>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id_ratio_based: Option<GatewayInstrumentationOpenTelemetryTraceIdRatioBased>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum GatewayInstrumentationOpenTelemetrySamplingType {
    AlwaysOn,
    AlwaysOff,
    ParentBased,
    TraceIdRatioBased,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GatewayInstrumentationOpenTelemetryParentBased {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_type: Option<GatewayInstrumentationOpenTelemetryParentBasedType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id_ratio_based: Option<GatewayInstrumentationOpenTelemetryTraceIdRatioBased>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum GatewayInstrumentationOpenTelemetryParentBasedType {
    AlwaysOn,
    AlwaysOff,
    TraceIdRatioBased,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GatewayInstrumentationOpenTelemetryTraceIdRatioBased {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ratio: Option<f64>,
}
