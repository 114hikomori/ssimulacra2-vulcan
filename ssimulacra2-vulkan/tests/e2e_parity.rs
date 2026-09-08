// M5+M6: end-to-end GPU pipeline vs oracle dumps - per-scale norms, weighted
// sum, and final score on every dumped fixture.
use ssimulacra2_vulkan::context::VkContext;
use ssimulacra2_vulkan::gpu_pipeline::compute_ssimulacra2_gpu;
use ssimulacra2_vulkan::oracle_dump::Dump;
use ssimulacra2_vulkan::score::{score, weighted_sum};

fn max_abs(a: &[f64], b: &[f64]) -> f64 {
    // F3 (BUG_HUNT): NaN-aware - f64::max would silently drop a NaN drift.
    let mut worst = 0.0f64;
    for (x, y) in a.iter().zip(b) {
        let d = (x - y).abs();
        if d.is_nan() || d > worst {
            worst = d;
        }
    }
    worst
}

fn check(ctx: &VkContext, fixture: &str, run: u32, expect_exact_100: bool) {
    // ulp-level bars only on IEEE-fma devices. On non-IEEE devices (llvmpipe)
    // the meaningful gate is the norms level (CI run #4: norms <=3.9e-5 while
    // the score drifts 3.3e-3..>1e-2 because smooth-region denom_s ~ kC2
    // amplifies driver fma noise ~1100x through the division - score-level
    // assertions are meaningless there and are printed, not asserted).
    // Structural bugs still get caught on any device: norms 1e-3, weighted
    // 5e-2. Identity exactness holds on IEEE-fma devices; on llvmpipe it is
    // an OPEN ANOMALY (99.984, see CHECKPOINT 2026-09-08 run #5 and
    // BUG_HUNT.md F4) - identity is NOT driver-independent as once claimed.
    let strict = ctx.fma_ieee();
    let (norm_bar, w_bar) = if strict { (1e-6, 1e-6) } else { (1e-3, 5e-2) };
    let p = |name: &str| -> Dump {
        Dump::read(format!(
            "{}/../dumps/{fixture}/run1/r{}_{name}.bin",
            env!("CARGO_MANIFEST_DIR"),
            run
        ))
    };
    let l1 = p("linear_orig_s0");
    let (w, h) = (l1.xsize as usize, l1.ysize as usize);
    let l2 = p("linear_dist_s0");
    let scales = compute_ssimulacra2_gpu(ctx, &l1.f32_data, &l2.f32_data, w, h).expect("pipeline");
    for (s, sn) in scales.iter().enumerate() {
        let g = p(&format!("ssim_norms_s{s}"));
        let e = p(&format!("edge_norms_s{s}"));
        let d1 = max_abs(&sn.avg_ssim, &g.f64_data);
        let d2 = max_abs(&sn.avg_edgediff, &e.f64_data);
        println!("{fixture} r{run} s{s}: ssim_norms {d1:e} edge_norms {d2:e}");
        assert!(d1 <= norm_bar && d2 <= norm_bar, "{fixture} s{s} norms drift");
    }
    let got = score(&scales);
    let want = p("score_final_s0").f64_data[0];
    let wd = (weighted_sum(&scales) - p("score_weighted_s0").f64_data[0]).abs();
    println!("{fixture} r{run}: weighted drift {wd:e}, score {got:.8} vs {want:.8} (drift {:e})", (got - want).abs());
    // Identity is driver-independent BY CONSTRUCTION after the num_s fix in
    // maps_combine.comp (BUG_HUNT F4 root cause, CI run #8): identical inputs
    // make every kernel output bitwise equal and num_s/denom_s the same
    // expression tree, so d == 0 exactly on any device. Asserted everywhere.
    if expect_exact_100 {
        assert_eq!(got, 100.0, "identity must be exactly 100");
        assert_eq!(wd, 0.0, "identity weighted sum must be bit-exact 0 drift");
    }
    if strict {
        assert!((got - want).abs() <= 1e-5, "{fixture} score drift");
        assert!(wd <= w_bar, "{fixture} weighted drift {wd:e}");
    } else {
        // Non-IEEE-fma devices (llvmpipe fma measured ~46 ulp sloppy): ulp-level
        // score parity is impossible by driver semantics; norms/weighted sanity
        // still gate and score drift is printed for visibility.
        assert!(wd <= w_bar, "{fixture} weighted drift {wd:e}");
        println!("NOTE: {fixture} score assert skipped (non-IEEE-fma device)");
    }
}

#[test]
fn e2e_matches_oracle() {
    let ctx = VkContext::new().expect("vulkan context");
    for fixture in ["photo", "grad", "noise", "step", "odd", "s8", "s9", "s12", "s15", "gray"] {
        check(&ctx, fixture, 0, false);
    }
    check(&ctx, "identical", 0, true);
    // alpha: CLI = min(score@bg0.1, score@bg0.9); dumps have r0 and r1
    check(&ctx, "alpha", 0, false);
    check(&ctx, "alpha", 1, false);
}
