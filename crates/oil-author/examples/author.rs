use std::io::{self, BufRead};
fn main() {
    let mut s = oil_author::session::Session::default();
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let result = match s.call(&line) {
            Ok(v) => v,
            Err(e) => serde_json::json!({"error":e}),
        };
        println!(
            "{}",
            serde_json::json!({"result":result,"pixels":s.pixels,"strokes":s.strokes})
        );
    }
}
