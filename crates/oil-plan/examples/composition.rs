use sha2::{Digest, Sha256};
use std::io::{self, Read};
fn main() {
    let mut text = String::new();
    io::stdin().read_to_string(&mut text).unwrap();
    let scene = oil_scene::parse(&text).unwrap();
    let t = std::time::Instant::now();
    let (list, report) = oil_plan::plan(
        &oil_mix::OchrellMixer,
        &scene,
        &oil_plan::PlanOptions {
            seed: 1907,
            plan_width: 256,
            strict_engine: false,
        },
        &Default::default(),
        &|| 0.,
    )
    .unwrap();
    let ms = t.elapsed().as_secs_f64() * 1000.;
    println!(
        "{}",
        serde_json::json!({"document":oil_plan::composition(&list,&report),"strokesHash":format!("{:x}",Sha256::digest(list.to_bytes())),"planMs":ms,"report":report})
    );
}
