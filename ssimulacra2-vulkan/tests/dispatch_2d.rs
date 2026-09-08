// Judge finding #3 (2026-09-08): the 2D dispatch path (gy>1) had no strict
// gate - it only ran on CI's big fixture behind the 0.5 sanity bar. Two fixes:
// (a) pure unit tests for split_groups covering gy>1 arithmetic on any host;
// (b) an integration test that runs mul_planes (OpFMul only - correctly
// rounded on every conformant driver, no fma involved) at 2048^2 size, which
// forces gy=4 on minimum-spec drivers (llvmpipe: max x 65535) and asserts
// BIT-EXACTNESS against the host product on every device.
use ssimulacra2_vulkan::context::VkContext;
use ssimulacra2_vulkan::pipeline::split_groups;

#[test]
fn split_groups_arithmetic() {
    // fits in x: single row
    assert_eq!(split_groups(576, 65535), (576, 1));
    // llvmpipe minimum at 2048^2 mul_planes (3*2048^2/64 = 196608 groups)
    assert_eq!(split_groups(196608, 65535), (65535, 4));
    // flat index reconstruction must round-trip: gx*64 is the row stride
    let (gx, gy) = split_groups(196608, 65535);
    assert!((gx as u64) * (gy as u64) >= 196608);
    assert!((gx as u64) * ((gy - 1) as u64) < 196608, "gy minimal");
    // edge cases
    assert_eq!(split_groups(1, 65535), (1, 1));
    assert_eq!(split_groups(0, 65535), (1, 0)); // never dispatched; must not panic
    assert_eq!(split_groups(131070, 65535), (65535, 2));
    assert_eq!(split_groups(65536, 65535), (65535, 2));
    // degenerate max_x clamps to 1
    assert_eq!(split_groups(5, 0), (1, 5));
}

#[test]
fn mul_planes_2d_dispatch_bit_exact_at_2048() {
    let ctx = VkContext::new().expect("vulkan context");
    let (w, h) = (2048usize, 2048usize);
    let n3 = 3 * w * h; // 12,582,912 elements -> 196,608 groups
    let groups = ((n3 as u32) + 63) / 64;
    let (gx, gy) = split_groups(groups, ctx.max_groups_x());
    println!(
        "device '{}' max_x={} -> dispatch ({gx},{gy}) for {groups} groups",
        ctx.device_name(),
        ctx.max_groups_x()
    );

    // deterministic pattern, plane-major
    let a: Vec<f32> = (0..n3).map(|i| ((i % 997) as f32) * 0.001 + 0.5).collect();
    let b: Vec<f32> = (0..n3).map(|i| ((i % 883) as f32) * 0.0013 + 0.25).collect();
    let expected: Vec<f32> = a.iter().zip(&b).map(|(x, y)| x * y).collect();

    let ba = ctx.create_buffer_f32(&a).unwrap();
    let bb = ctx.create_buffer_f32(&b).unwrap();
    let bo = ctx.create_empty(n3).unwrap();
    ctx.run_compute_push(
        include_bytes!("../shaders/mul_planes.spv"),
        c"main",
        &[&ba, &bb, &bo],
        groups,
        &(n3 as u32).to_le_bytes(),
    )
    .expect("2D dispatch");
    let got = ctx.readback_f32(&bo).unwrap();
    ctx.destroy_buffer(ba);
    ctx.destroy_buffer(bb);
    ctx.destroy_buffer(bo);

    // OpFMul is IEEE-mandated: bit-exact on every conformant driver, IEEE or not.
    let bad: Vec<usize> = (0..n3).filter(|&i| got[i].to_bits() != expected[i].to_bits()).collect();
    assert!(
        bad.is_empty(),
        "mul_planes 2D path differs at {} of {n3} (first {:?}): got {:?} want {:?}",
        bad.len(),
        bad.first(),
        bad.first().map(|&i| got[i]),
        bad.first().map(|&i| expected[i])
    );
    println!("mul_planes bit-exact over {n3} elements, gy={gy}");
}
