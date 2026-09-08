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
    // GPU-mode bar is device-gated by the fma fingerprint (same policy as
    // e2e_parity); CPU mode is driver-independent and always strict.
    let gpu_bar = match ssimulacra2_vulkan::context::VkContext::new() {
        Ok(ctx) => {
            if ctx.fma_ieee() {
                        1e-5
                    } else {
                        0.5
                    }
                }
                Err(_) => 0.5,
    };
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
            let bar = if extra.contains(&"--cpu") { 1e-5 } else { gpu_bar };
            assert!((g - w).abs() <= bar, "{mode} {pair}: {g} vs {w} (bar {bar:e})");
        };
        f("gpu", &[]);
        f("cpu", &["--cpu"]);
        println!("{pair}: gpu+cpu ok ({want})");
    }
}

#[test]
fn cli_identity_and_rejections() {
    let (ok, got) = run(&["tests/fixtures/photo_orig.png", "tests/fixtures/photo_orig.png"]);
    assert!(ok, "identity run failed");
    // Exact 100.00000000 is only assertable on IEEE-fma devices (llvmpipe
    // returns 99.984 - see CHECKPOINT 2026-09-08 run #5 open question).
    let strict = ssimulacra2_vulkan::context::VkContext::new().map(|c| c.fma_ieee()).unwrap_or(false);
    if strict {
        assert_eq!(got, "100.00000000", "identity: {got}");
    } else {
        let g: f64 = got.parse().unwrap();
        assert!((g - 100.0).abs() <= 0.5, "identity sanity: {g}");
    }
    let (ok, _) = run(&["tests/fixtures/s7_orig.png", "tests/fixtures/s7_dist.png"]);
    assert!(!ok, "sub-8x8 must be rejected");
    let (ok, _) = run(&["tests/fixtures/photo_orig.png", "tests/fixtures/odd_dist.png"]);
    assert!(!ok, "size mismatch must be rejected");
}

#[test]
fn cli_asymmetric_alpha_dispatch_matches_oracle() {
    // Regression (found by adversarial verification 2026-09-08): the oracle's
    // worst-of-bg dual pass fires ONLY when the ORIGINAL has alpha
    // (ssimulacra2_main.cc:105). When only the distorted image has alpha it is
    // a single bg=0.5 pass. Goldens: oracle/scores_asymmetric.txt.
    let cases = [
        ("gray_orig", "alpha_dist", -53.27997245f64),
        ("alpha_orig", "gray_dist", -108.32956807f64),
    ];
    for (a, b, want) in cases {
        for extra in [&[] as &[&str], &["--cpu"][..]] {
            let oa = format!("tests/fixtures/{a}.png");
            let ob = format!("tests/fixtures/{b}.png");
            let mut args: Vec<&str> = extra.to_vec();
            args.push(oa.as_str());
            args.push(ob.as_str());
            let (ok, got) = run(&args);
            assert!(ok, "{a}/{b} {extra:?} failed");
            let g: f64 = got.parse().unwrap();
            let bar = if extra.contains(&"--cpu") {
                1e-5
            } else {
                match ssimulacra2_vulkan::context::VkContext::new() {
                    Ok(ctx) => {
                        if ctx.fma_ieee() {
                            1e-5
                        } else {
                            0.5
                        }
                    }
                    Err(_) => 0.5,
                }
            };
            assert!((g - want).abs() <= bar, "{a} vs {b} ({extra:?}): {g} vs oracle {want}");
        }
    }
}
