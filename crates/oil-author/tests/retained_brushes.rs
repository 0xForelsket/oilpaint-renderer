//! Frozen six-brush behavior from dev.5. Only the test fixture's engine stamp is rebound;
//! public loaders still reject mismatched engine versions and no saved input is rewritten.
use oil_author::{Catalog, Document};
use oil_mix::{Mixer, OchrellMixer, RgbMixer};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn compare<M: Mixer>(m: M, d: &Document, expected: &Value) {
    let list = d.compile().unwrap();
    let (cv, _) = oil_paint::paint(&m, &list, 384, |_, _| {});
    for plane in oil_paint::PLANES {
        let got = format!("{:x}", Sha256::digest(oil_paint::plane_bytes(&cv, plane)));
        assert_eq!(
            expected[plane].as_str().unwrap(),
            got,
            "retained brush plane {plane}"
        );
    }
}

#[test]
fn retained_six_match_dev5_in_both_mixers() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../spec/examples/retained-brushes.json")).unwrap();
    let reference: Catalog = serde_json::from_value(fixture["catalog"].clone()).unwrap();
    let current = oil_author::catalog();
    let mut count = 0;
    for p in &reference.presets {
        if p.id == "scumble" {
            continue;
        }
        assert_eq!(
            Some(p),
            current.presets.iter().find(|c| c.id == p.id),
            "retained preset {} changed",
            p.id
        );
        count += 1;
    }
    assert_eq!(count, 6);
    for c in fixture["cases"].as_array().unwrap() {
        let mut v = c["document"].clone();
        v["catalog"] = fixture["catalog"].clone();
        v["engine"] = json!(oil_kernel::ENGINE_VERSION);
        let doc: Document = serde_json::from_value(v).unwrap();
        compare(RgbMixer, &doc, &c["expected"]["rgb"]);
        compare(OchrellMixer, &doc, &c["expected"]["ochrell"]);
    }
}
