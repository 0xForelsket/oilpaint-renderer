//! Native side of the cross-host check: run every case, write `{host, engine, runtime, cases}` JSON.
//! Usage: xhost [--out FILE] [--host NAME] [--dump-dir DIR]   (--dump-dir also writes each case's bytes, to diagnose a difference)
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::time::Instant;

fn main() {
    let mut out: Option<String> = None;
    let mut dump: Option<String> = None;
    let mut host = format!("native-{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => out = args.next(),
            "--host" => host = args.next().expect("--host needs a name"),
            "--dump-dir" => dump = args.next(),
            _ => {
                eprintln!("usage: xhost [--out FILE] [--host NAME] [--dump-dir DIR]");
                std::process::exit(2);
            }
        }
    }
    let mut json = String::new();
    write!(json, "{{\"host\":\"{host}\",\"engine\":\"{}\",\"runtime\":\"native\",\"cases\":{{", oil_xhost::engine_version()).unwrap();
    for (i, case) in oil_xhost::cases().iter().enumerate() {
        let t0 = Instant::now();
        let bytes = (case.run)();
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        if let Some(dir) = &dump {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(std::path::Path::new(dir).join(format!("{}.bin", case.name)), &bytes).unwrap();
        }
        let digest: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
        let expect = match case.expect {
            oil_xhost::Expect::Identical => "identical",
            oil_xhost::Expect::MayDiffer => "may_differ",
        };
        if i > 0 {
            json.push(',');
        }
        write!(json, "\"{}\":{{\"sha256\":\"{digest}\",\"bytes\":{},\"expect\":\"{expect}\",\"ms\":{ms:.3}}}", case.name, bytes.len()).unwrap();
    }
    json.push_str("}}\n");
    match out {
        Some(path) => {
            if let Some(dir) = std::path::Path::new(&path).parent() {
                std::fs::create_dir_all(dir).unwrap();
            }
            std::fs::write(&path, &json).unwrap();
            eprintln!("xhost: {} cases -> {path}", oil_xhost::cases().len());
        }
        None => print!("{json}"),
    }
}
