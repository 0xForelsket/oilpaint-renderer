//! Style presets: named style values that sit between the engine's neutral defaults and a scene's region styles
//! (plan section 11, realism roadmap). They are data (`presets/*.json`, parsed as a `Style`), so a preset can only
//! set keys a style has. The names are working names until Sean settles them.
use crate::spec::Style;

/// (name, JSON) of every preset in this build.
const PRESETS: [(&str, &str); 1] = [("impressionist", include_str!("../presets/impressionist.json"))];

/// Names of the presets in this build.
pub fn names() -> Vec<&'static str> {
    PRESETS.iter().map(|(n, _)| *n).collect()
}

/// The style values of a preset, or None for an unknown name.
pub fn get(name: &str) -> Option<Style> {
    PRESETS.iter().find(|(n, _)| *n == name).map(|(_, json)| serde_json::from_str(json).expect("presets are checked by a test"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_preset_parses_as_a_style() {
        for (name, json) in super::PRESETS {
            serde_json::from_str::<crate::spec::Style>(json).unwrap_or_else(|e| panic!("preset {name}: {e}"));
        }
        assert!(super::get("impressionist").unwrap().l_floor == Some(20.0));
        assert!(super::get("nope").is_none());
    }
}
