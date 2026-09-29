//! oilpaint engine kernel. L0 holds only the engine version; L1 ports the bristle-lane brush.
#![forbid(unsafe_code)]

/// The engine version (spec/README.md): bump it with every change that can move an output bit, and regenerate
/// `golden/<version>.json` with it. `2.0.0-dev.0` means "no engine output yet" (the L0 skeleton).
pub const ENGINE_VERSION: &str = "2.0.0-dev.0";

#[cfg(test)]
mod tests {
    #[test]
    fn engine_version_fits_the_strokelist_header() {
        let v = super::ENGINE_VERSION;
        assert!((1..=51).contains(&v.len()));
        assert!(v.bytes().all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b)));
    }
}
