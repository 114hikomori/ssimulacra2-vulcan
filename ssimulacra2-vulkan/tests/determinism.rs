// F4 (BUG_HUNT): localize the llvmpipe identity anomaly. The algebra says
// identical inputs through identical kernels must produce bit-identical
// outputs (=> d == 0 => score exactly 100); the anomaly proves some kernel
// differs between two dispatches on that driver. This test runs each suspect
// kernel twice on identical content and reports the first that differs.
// On IEEE-fma devices it asserts full determinism; on non-IEEE devices it
// prints the culprit (localization for the open question) without failing.
use ssimulacra2_vulkan::blur::{blur_planes, create_recursive_gaussian};
use ssimulacra2_vulkan::context::VkContext;
use ssimulacra2_vulkan::oracle_dump::Dump;
use ssimulacra2_vulkan::xyb::xyb_convert;

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
