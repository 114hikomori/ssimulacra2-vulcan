// M10-batch: the single-pair CLI size-routing contract (threshold
// GPU_ROUTE_MIN_PIXELS in main.rs; calibration in bench/routing_calib.py).
// Routing must be transparent: default picks the engine, --gpu/--cpu keep
// forcing theirs, and score-many is never routed (GPU-always, batch math).
use std::process::Command;

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_ssimulacra2-vulkan"))
        .current_dir("..")
        .args(args)
        .output()
        .expect("run cli");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

#[test]
fn small_pair_default_matches_cpu_engine_byte_for_byte() {
    // photo is 128x96 = 0.012 MP, far under the 0.5 MP threshold.
    let d = run(&["tests/fixtures/photo_orig.png", "tests/fixtures/photo_dist.png"]);
    let c = run(&["--cpu", "tests/fixtures/photo_orig.png", "tests/fixtures/photo_dist.png"]);
    assert!(d.0 && c.0, "runs failed: {d:?} {c:?}");
    assert_eq!(d.1, c.1, "small default must be the CPU engine exactly");
    assert!(d.2.contains("routing threshold"), "routing note missing: {}", d.2);
}

#[test]
fn gpu_override_forces_gpu_engine_on_small_pair() {
    let g = run(&["--gpu", "tests/fixtures/photo_orig.png", "tests/fixtures/photo_dist.png"]);
    let c = run(&["--cpu", "tests/fixtures/photo_orig.png", "tests/fixtures/photo_dist.png"]);
    assert!(g.0 && c.0);
    assert!(!g.2.contains("routing threshold"), "--gpu must not route");
    // Same metric within the device-gated GPU bar (1e-5 IEEE / 0.5 elsewhere,
    // same policy as cli_parity) - engines agree, they are not bit-equal.
    let bar = match ssimulacra2_vulkan::context::VkContext::new() {
        Ok(ctx) => {
            if ctx.fma_ieee() {
                1e-5
            } else {
                0.5
            }
        }
        Err(_) => 0.5, // no Vulkan device: --gpu fell back to CPU by contract
    };
    let (gv, cv): (f64, f64) = (g.1.parse().unwrap(), c.1.parse().unwrap());
    assert!((gv - cv).abs() <= bar, "gpu-vs-cpu on photo: {gv} vs {cv} (bar {bar:e})");
}

#[test]
fn large_pair_default_matches_gpu_engine_byte_for_byte() {
    // big is 2048x2048 = 4.19 MP, above the threshold: default IS the GPU run.
    let d = run(&["tests/fixtures/big_orig.png", "tests/fixtures/big_dist.png"]);
    let g = run(&["--gpu", "tests/fixtures/big_orig.png", "tests/fixtures/big_dist.png"]);
    assert!(d.0 && g.0, "runs failed: {d:?} {g:?}");
    assert_eq!(d.1, g.1, "large default must be the GPU engine exactly");
    assert!(!d.2.contains("routing threshold"), "large must not route: {}", d.2);
}

#[test]
fn engine_flags_are_exclusive() {
    let r = run(&["--cpu", "--gpu", "tests/fixtures/photo_orig.png", "tests/fixtures/photo_dist.png"]);
    assert!(!r.0, "--cpu + --gpu must be rejected");
}

#[test]
fn usage_apis_advertise_batch_mode() {
    // Regression: a sibling project read the old one-line usage and concluded
    // "batch isn't in this binary's CLI". Both discovery paths must name
    // score-many from now on.
    let h = run(&["--help"]);
    assert!(h.0, "--help must exit 0");
    assert!(h.1.contains("score-many"), "--help stdout hides batch: {}", h.1);
    assert!(h.1.contains("normalize"), "--help stdout hides normalize: {}", h.1);
    let u = run(&[]);
    assert!(!u.0, "no-args must exit nonzero");
    assert!(u.2.contains("score-many"), "usage stderr hides batch: {}", u.2);
    assert!(u.2.contains("normalize"), "usage stderr hides normalize: {}", u.2);
}

#[test]
fn score_many_stays_gpu_always_below_threshold() {
    // A tiny-original batch must NOT route: score-many builds its own context.
    // Three same-size (128x96) variants vs the photo original, one line each,
    // no routing note on stderr.
    let dir = std::env::temp_dir().join("s2v_route_batch");
    let vars = dir.join("vars");
    std::fs::create_dir_all(&vars).unwrap();
    // in-process reads resolve from the crate dir; fixtures are repo-root.
    let src = std::fs::read("../tests/fixtures/photo_dist.png").unwrap();
    let org = std::fs::read("../tests/fixtures/photo_orig.png").unwrap();
    std::fs::write(vars.join("v0.png"), &org).unwrap();
    std::fs::write(vars.join("v1.png"), &src).unwrap();
    std::fs::write(vars.join("v2.png"), &org).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_ssimulacra2-vulkan"))
        .current_dir("..")
        .args([
            "score-many",
            "--orig",
            "tests/fixtures/photo_orig.png",
            "--vars",
            vars.to_str().unwrap(),
        ])
        .output()
        .expect("run score-many");
    let _ = std::fs::remove_dir_all(&dir);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "score-many failed: {stderr}");
    assert_eq!(stdout.lines().count(), 3, "one line per variant");
    assert!(!stderr.contains("routing threshold"), "score-many must not route: {stderr}");
}
