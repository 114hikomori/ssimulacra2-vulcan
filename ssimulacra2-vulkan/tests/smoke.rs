// M1 smoke test: x2 shader element-exact on a real GPU (and lavapipe in CI).
use ssimulacra2_vulkan::context::VkContext;

#[test]
fn smoke_double_matches_cpu() {
    let ctx = VkContext::new().expect("vulkan context");
    println!("device: {}", ctx.device_name());

    let mut input: Vec<f32> = (0..1024).map(|i| (i as f32) * 0.25 - 128.0).collect();
    // extremes: 0, -0, max/4 (x2 stays finite), inf. Denormal at [2] is
    // informational: AMD RDNA2 shaders flush denormals to zero (observed
    // 2026-09-08); the SSIMULACRA2 data range (XYB ~0..1, kC2=0.0009 floor)
    // never produces denormals - re-asserted by the Phase E range audit.
    input[0] = 0.0;
    input[1] = -0.0;
    input[2] = f32::from_bits(1); // smallest denormal (FTZ-tolerant slot)
    input[3] = f32::MAX / 4.0;
    input[4] = f32::INFINITY;
    input[5] = f32::NEG_INFINITY;

    let expected: Vec<f32> = input.iter().map(|x| x * 2.0).collect();

    let buf = ctx.create_buffer_f32(&input).expect("upload");
    ctx.run_compute(
        include_bytes!("../shaders/smoke_double.spv"),
        std::ffi::CStr::from_bytes_with_nul(b"main\0").unwrap(),
        &[&buf],
        (input.len() as u32 + 63) / 64,
    )
    .expect("dispatch");
    let got = ctx.readback_f32(&buf).expect("readback");
    ctx.destroy_buffer(buf);

    for (i, (g, e)) in got.iter().zip(expected.iter()).enumerate() {
        if i == 2 {
            println!("denormal x2 -> {g:?} (0 = FTZ, 2.8e-45 = preserved)");
            assert!(g == e || *g == 0.0, "denormal slot got {g}");
            continue;
        }
        assert_eq!(g.to_bits(), e.to_bits(), "index {i}: got {g}, want {e}");
    }
    // -0.0 * 2 must stay -0.0 (bit compare above proves it)
    assert!(got[1].is_sign_negative());
}
