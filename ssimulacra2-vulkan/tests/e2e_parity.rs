// M5+M6: end-to-end GPU pipeline vs oracle dumps - per-scale norms, weighted
// sum, and final score on every dumped fixture.
use ssimulacra2_vulkan::context::VkContext;
use ssimulacra2_vulkan::gpu_pipeline::compute_ssimulacra2_gpu;
use ssimulacra2_vulkan::oracle_dump::Dump;
use ssimulacra2_vulkan::score::{score, weighted_sum};

fn max_abs(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0f64, f64::max)
}

fn check(ctx: &VkContext, fixture: &str, run: u32, expect_exact_100: bool) {
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
        assert!(d1 <= 1e-6 && d2 <= 1e-6, "{fixture} s{s} norms drift");
    }
    let got = score(&scales);
    let want = p("score_final_s0").f64_data[0];
    let wd = (weighted_sum(&scales) - p("score_weighted_s0").f64_data[0]).abs();
    println!("{fixture} r{run}: weighted drift {wd:e}, score {got:.8} vs {want:.8} (drift {:e})", (got - want).abs());
    if expect_exact_100 {
        assert_eq!(got, 100.0, "identity must be exactly 100");
        assert_eq!(wd, 0.0, "identity weighted sum must be bit-exact 0 drift");
    }
    assert!((got - want).abs() <= 1e-5, "{fixture} score drift");
    assert!(wd <= 1e-6, "{fixture} weighted drift {wd:e}");
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
