// F4 (BUG_HUNT): localize the llvmpipe identity anomaly. The algebra says
// identical inputs through identical kernels must produce bit-identical
// outputs (=> d == 0 => score exactly 100); the anomaly proves otherwise.
// Run #9 showed the num_s expression change had ZERO effect (drift values
// bit-identical to run #8), so the divergence is NOT fma contraction in
// num_s/denom_s. This file's second test walks the actual identity pipeline
// stage by stage and reports the first pair that differs.
// On IEEE-fma devices everything is asserted; on non-IEEE devices results are
// printed for localization without failing.
use ssimulacra2_vulkan::blur::{blur_planes, create_recursive_gaussian};
use ssimulacra2_vulkan::context::{GpuBuffer, VkContext};
use ssimulacra2_vulkan::oracle_dump::Dump;
use ssimulacra2_vulkan::xyb::xyb_convert;

fn bits_equal(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn gpu_mul(ctx: &VkContext, a: &GpuBuffer, b: &GpuBuffer, n: usize) -> GpuBuffer {
    let o = ctx.create_empty(n).unwrap();
    ctx.run_compute_push(
        include_bytes!("../shaders/mul_planes.spv"),
        c"main",
        &[a, b, &o],
        ((n as u32) + 63) / 64,
        &(n as u32).to_le_bytes(),
    )
    .unwrap();
    o
}

#[test]
fn kernel_dispatch_determinism() {
    let ctx = VkContext::new().expect("vulkan context");
    let strict = ctx.fma_ieee();
    let lin = Dump::read(format!(
        "{}/../dumps/photo/run1/r0_linear_orig_s0.bin",
        env!("CARGO_MANIFEST_DIR")
    ));
    let (w, h) = (lin.xsize as usize, lin.ysize as usize);
    let n = w * h;
    let rg = create_recursive_gaussian(1.5);

    let mut culprit: Option<&str> = None;
    let mut check = |name: &'static str, a: &Vec<f32>, b: &Vec<f32>| {
        let diff = a.iter().zip(b).any(|(x, y)| x.to_bits() != y.to_bits());
        println!("{name}: {}", if diff { "NONDETERMINISTIC" } else { "deterministic" });
        if diff && culprit.is_none() {
            culprit = Some(name);
        }
    };

    // xyb: same content uploaded twice (distinct buffers), one dispatch each.
    let b1 = ctx.create_buffer_f32(&lin.f32_data).unwrap();
    let b2 = ctx.create_buffer_f32(&lin.f32_data).unwrap();
    let x1 = xyb_convert(&ctx, &b1, n, true).unwrap();
    let x2 = xyb_convert(&ctx, &b2, n, true).unwrap();
    let r1 = ctx.readback_f32(&x1).unwrap();
    let r2 = ctx.readback_f32(&x2).unwrap();
    check("xyb_positive", &r1, &r2);

    // blur: same content, two independent H+V passes.
    let u1 = ctx.create_buffer_f32(&r1).unwrap();
    let u2 = ctx.create_buffer_f32(&r1).unwrap();
    let m1 = blur_planes(&ctx, &u1, w, h, &rg).unwrap();
    let m2 = blur_planes(&ctx, &u2, w, h, &rg).unwrap();
    let q1 = ctx.readback_f32(&m1).unwrap();
    let q2 = ctx.readback_f32(&m2).unwrap();
    check("blur_h+blur_v", &q1, &q2);

    // blur again on the SAME buffer (dispatch-to-dispatch stability).
    let m3 = blur_planes(&ctx, &u1, w, h, &rg).unwrap();
    let q3 = ctx.readback_f32(&m3).unwrap();
    check("blur_same_buffer", &q1, &q3);

    for b in [b1, b2, x1, x2, u1, u2, m1, m2, m3] {
        ctx.destroy_buffer(b);
    }
    if strict {
        assert!(culprit.is_none(), "IEEE device kernels nondeterministic: {culprit:?}");
    } else {
        println!("non-IEEE device first diverging kernel: {culprit:?} (BUG_HUNT F4 localization)");
    }
}

/// Walk the identity pipeline stage by stage (mirroring gpu_pipeline's scale-0
/// path) and report the first pair of supposedly-equal buffers that differs.
#[test]
fn identity_stage_comparison() {
    let ctx = VkContext::new().expect("vulkan context");
    let strict = ctx.fma_ieee();
    let lin1 = Dump::read(format!(
        "{}/../dumps/identical/run1/r0_linear_orig_s0.bin",
        env!("CARGO_MANIFEST_DIR")
    ));
    let lin2 = Dump::read(format!(
        "{}/../dumps/identical/run1/r0_linear_dist_s0.bin",
        env!("CARGO_MANIFEST_DIR")
    ));
    assert!(bits_equal(&lin1.f32_data, &lin2.f32_data), "identity linear dumps differ?!");
    let (w, h) = (lin1.xsize as usize, lin1.ysize as usize);
    let n = w * h;
    let rg = create_recursive_gaussian(1.5);

    let mut first_diff: Option<&str> = None;
    let mut note = |name: &'static str, eq: bool| {
        println!("identity stage {name}: {}", if eq { "equal" } else { "DIFFERS" });
        if !eq && first_diff.is_none() {
            first_diff = Some(name);
        }
    };

    let l1 = ctx.create_buffer_f32(&lin1.f32_data).unwrap();
    let l2 = ctx.create_buffer_f32(&lin2.f32_data).unwrap();
    let x1 = xyb_convert(&ctx, &l1, n, true).unwrap();
    let x2 = xyb_convert(&ctx, &l2, n, true).unwrap();
    note("x1_vs_x2", bits_equal(&ctx.readback_f32(&x1).unwrap(), &ctx.readback_f32(&x2).unwrap()));

    let m11 = gpu_mul(&ctx, &x1, &x1, 3 * n);
    let m12 = gpu_mul(&ctx, &x1, &x2, 3 * n);
    let m22 = gpu_mul(&ctx, &x2, &x2, 3 * n);
    note("m11_vs_m12", bits_equal(&ctx.readback_f32(&m11).unwrap(), &ctx.readback_f32(&m12).unwrap()));
    note("m11_vs_m22", bits_equal(&ctx.readback_f32(&m11).unwrap(), &ctx.readback_f32(&m22).unwrap()));

    let s11 = blur_planes(&ctx, &m11, w, h, &rg).unwrap();
    let s12 = blur_planes(&ctx, &m12, w, h, &rg).unwrap();
    let s22 = blur_planes(&ctx, &m22, w, h, &rg).unwrap();
    let mu1 = blur_planes(&ctx, &x1, w, h, &rg).unwrap();
    let mu2 = blur_planes(&ctx, &x2, w, h, &rg).unwrap();
    let rs11 = ctx.readback_f32(&s11).unwrap();
    let rs12 = ctx.readback_f32(&s12).unwrap();
    let rs22 = ctx.readback_f32(&s22).unwrap();
    let rmu1 = ctx.readback_f32(&mu1).unwrap();
    let rmu2 = ctx.readback_f32(&mu2).unwrap();
    note("s11_vs_s12", bits_equal(&rs11, &rs12));
    note("s11_vs_s22", bits_equal(&rs11, &rs22));
    note("mu1_vs_mu2", bits_equal(&rmu1, &rmu2));

    let sd = ctx.create_empty(3 * n).unwrap();
    let ed = ctx.create_empty(3 * n).unwrap();
    ctx.run_compute_push(
        include_bytes!("../shaders/maps_combine.spv"),
        c"main",
        &[&x1, &x2, &mu1, &mu2, &s11, &s22, &s12, &sd, &ed],
        ((3 * n) as u32 + 63) / 64,
        &(n as u32).to_le_bytes(),
    )
    .unwrap();
    let sd_r = ctx.readback_f32(&sd).unwrap();
    let nz: Vec<usize> = sd_r.iter().enumerate().filter(|(_, d)| **d != 0.0).map(|(i, _)| i).collect();
    println!("identity ssim_d nonzero: {} of {}", nz.len(), sd_r.len());
    if let Some(&i) = nz.first() {
        println!(
            "  first at {i} (c {} x {} y {}): d={:e} m1={:e} m2={:e} s11={:e} s12={:e} mu12prod={:e}",
            i / n, i % w, i / w, sd_r[i], rmu1[i], rmu2[i], rs11[i], rs12[i],
            rmu1[i] * rmu2[i]
        );
    }
    note("ssim_d_all_zero", nz.is_empty());

    for b in [l1, l2, x1, x2, m11, m12, m22, s11, s12, s22, mu1, mu2, sd, ed] {
        ctx.destroy_buffer(b);
    }
    if strict {
        assert!(first_diff.is_none(), "IEEE identity stages differ: {first_diff:?}");
    } else {
        println!("non-IEEE identity first differing stage: {first_diff:?} (BUG_HUNT F4)");
    }
}
