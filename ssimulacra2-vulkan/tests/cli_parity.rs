// M7/M8: CLI parity vs the C++ oracle goldens (oracle/scores_prepatch.txt),
// both GPU default and --cpu paths, plus rejection behaviors.
use std::process::Command;

const GOLDEN: &[(&str, &str)] = &[
    ("photo", "9.07751048"), ("grad", "73.31752176"), ("noise", "86.75561562"),
    ("step", "77.33264073"), ("odd", "14.14816917"), ("s8", "86.60044478"),
    ("s9", "85.34753189"), ("s12", "86.12212472"), ("s15", "61.06475750"),
    ("alpha", "1.24334693"), ("gray", "9.11027071"), ("big", "2.99412696"),
];

fn run(args: &[&str]) -> (bool, String) {
    // tests run with CWD = crate dir; fixtures are repo-root relative
    let out = Command::new(env!("CARGO_BIN_EXE_ssimulacra2-vulkan"))
        .current_dir("..")
        .args(args)
        .output()
        .expect("run cli");
    (out.status.success(), String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[test]
fn cli_gpu_and_cpu_match_goldens() {
    for (pair, want) in GOLDEN {
        let f = |mode: &str, extra: &[&str]| {
            let o = format!("tests/fixtures/{pair}_orig.png");
            let d = format!("tests/fixtures/{pair}_dist.png");
            let mut a: Vec<&str> = extra.to_vec();
            a.push(o.as_str());
            a.push(d.as_str());
            let (ok, got) = run(&a);
            assert!(ok, "{mode} {pair} failed");
            let g: f64 = got.parse().expect("score");
            let w: f64 = want.parse().unwrap();
            assert!((g - w).abs() <= 1e-5, "{mode} {pair}: {g} vs {w}");
        };
        f("gpu", &[]);
        f("cpu", &["--cpu"]);
        println!("{pair}: gpu+cpu ok ({want})");
    }
}

#[test]
fn cli_identity_and_rejections() {
    let (ok, got) = run(&["tests/fixtures/photo_orig.png", "tests/fixtures/photo_orig.png"]);
    assert!(ok && got == "100.00000000", "identity: {got}");
    let (ok, _) = run(&["tests/fixtures/s7_orig.png", "tests/fixtures/s7_dist.png"]);
    assert!(!ok, "sub-8x8 must be rejected");
    let (ok, _) = run(&["tests/fixtures/photo_orig.png", "tests/fixtures/odd_dist.png"]);
    assert!(!ok, "size mismatch must be rejected");
}
