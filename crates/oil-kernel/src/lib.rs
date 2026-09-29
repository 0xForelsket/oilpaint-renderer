//! oilpaint engine kernel: canvas planes and the bristle-lane brush, generic over the mixer.
#![forbid(unsafe_code)]
// v1's clamp01 and range checks are kept verbatim (same results as f32::clamp, including NaN and signed zero).
// Its literals (6.2831853) and compound updates (bf *= bf * g) are also kept as v1 wrote them.
#![allow(clippy::manual_clamp, clippy::manual_range_contains, clippy::approx_constant, clippy::misrefactored_assign_op)]

pub mod brush;
pub mod planes;

pub use brush::{render_stroke, BrushParams, Load, StrokeStats};
pub use planes::{Canvas, Planes};

/// The engine version (spec/README.md): bump it with every change that can move an output bit, and regenerate
/// `golden/<version>.json` with it. History: spec/ENGINE_VERSIONS.md.
pub const ENGINE_VERSION: &str = "2.0.0-dev.1";

#[cfg(test)]
mod tests {
    #[test]
    fn engine_version_fits_the_strokelist_header() {
        let v = super::ENGINE_VERSION;
        assert!((1..=51).contains(&v.len()));
        assert!(v.bytes().all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b)));
    }
}
