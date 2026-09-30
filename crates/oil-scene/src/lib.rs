//! ScenePlan v1 (spec/SCENEPLAN_V1.md): the types, their JSON Schema, validation, and the scene compiler that
//! rasterises a plan into guide maps (target image, regions, flow, light) at any width.
//!
//! Everything here is deterministic: the same plan, width and mixer give the same guide bits on every host (no
//! platform libm, fixed summation orders, no hash-map iteration in outputs).
#![forbid(unsafe_code)]
// Structured errors are large (136 bytes) but only travel on cold paths.
#![allow(clippy::result_large_err)]

pub mod color;
pub mod compile;
pub mod flows;
pub mod presets;
pub mod preview;
pub mod raster;
pub mod spec;
pub mod validate;

pub use oil_errors::Error;
pub use compile::{compile, FieldData, Guides, Mask, Timings};
pub use flows::{FlowEval, Sampled};
pub use spec::ScenePlan;
pub use validate::{load, parse, validate, Report};

/// Published `$id` of the schema.
pub const SCHEMA_ID: &str = "https://oilpaint.dev/schema/sceneplan-1.json";

/// The JSON Schema of ScenePlan v1 (draft-07, which most editors and code generators read), generated from the
/// Rust types (`spec/sceneplan-1.schema.json`).
pub fn schema() -> serde_json::Value {
    let gen = schemars::generate::SchemaSettings::draft07().into_generator();
    let mut s = serde_json::to_value(gen.into_root_schema_for::<ScenePlan>()).unwrap_or_default();
    if let Some(o) = s.as_object_mut() {
        o.insert("$id".into(), SCHEMA_ID.into());
        o.insert("title".into(), "ScenePlan".into());
    }
    s
}

#[cfg(test)]
mod tests {
    #[test]
    fn schema_is_in_sync_with_the_committed_file() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../spec/sceneplan-1.schema.json");
        let now = serde_json::to_string_pretty(&super::schema()).unwrap() + "\n";
        if std::env::var_os("OIL_WRITE_SCHEMA").is_some() {
            std::fs::write(path, &now).unwrap();
        }
        let committed = std::fs::read_to_string(path).unwrap_or_default();
        assert!(committed == now, "spec/sceneplan-1.schema.json is stale: run with OIL_WRITE_SCHEMA=1");
    }
}
