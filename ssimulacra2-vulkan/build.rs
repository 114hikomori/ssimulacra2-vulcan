// Build script: extract the 108 score weights from the oracle source verbatim
// (never retyped) and emit them as exact bit patterns.
use std::io::Write;

fn main() {
    println!("cargo::rerun-if-changed=../src/ssimulacra2.cc");
    let src = std::fs::read_to_string("../src/ssimulacra2.cc")
        .expect("read src/ssimulacra2.cc (workspace root build)");
    let start = src
        .find("constexpr double weight[108] = {")
        .expect("weight array marker");
    let body = &src[start + "constexpr double weight[108] = {".len()..];
    let end = body.find("};").expect("weight array end");
    let nums: Vec<f64> = body[..end]
        .split(',')
        .map(|t| t.trim().parse::<f64>().expect("weight literal"))
        .collect();
    assert_eq!(nums.len(), 108, "exactly 108 weights");
    let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("weights.rs");
    let mut f = std::fs::File::create(out).unwrap();
    writeln!(f, "// GENERATED from ../src/ssimulacra2.cc by build.rs - do not edit.").unwrap();
    writeln!(f, "pub static WEIGHTS: [f64; 108] = [").unwrap();
    for w in &nums {
        writeln!(f, "    f64::from_bits(0x{:016x}),", w.to_bits()).unwrap();
    }
    writeln!(f, "];").unwrap();
}
