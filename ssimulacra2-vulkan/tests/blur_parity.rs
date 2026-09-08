// M3: GPU recursive-Gaussian blur vs oracle dumps + synthetic battery.
// Per-device bars: RDNA2 evaluates the shader's fma chain bit-exactly like
// the CPU (measured drift 0.0); llvmpipe's fma differs by ~0.5 ulp per step
// and the marginally-stable IIR accumulates it along the row (CI run #1:
// 4.35e-6 on 128-wide rows). Cause classified as driver FP evaluation
// (plan 7: fix cause before widening - there is no cause to fix here, the
// shader already uses explicit fma matching the oracle's SIMD MulAdd), so the
// lavapipe bar is the measured value with headroom, not a hidden widening.
use ssimulacra2_vulkan::blur::{blur_planes, blur_planes_cpu, create_recursive_gaussian};
use ssimulacra2_vulkan::context::VkContext;
use ssimulacra2_vulkan::oracle_dump::{max_abs_diff, Dump};

fn blur_bar(ctx: &VkContext) -> f32 {
    if ctx.device_name().to_lowercase().contains("llvmpipe") {
        1e-5
    } else {
        2e-6
    }
}

fn dump_path(fixture: &str, name: &str) -> String {
    format!(
        "{}/../dumps/{fixture}/run1/r0_{name}.bin",
        env!("CARGO_MANIFEST_DIR")
    )
}

#[test]
fn rg_constants_bit_match_oracle() {
    let rg = create_recursive_gaussian(1.5);
    let d = Dump::read(dump_path("photo", "rg_s0"));
    let radius = Dump::read(dump_path("photo", "rg_radius_s0"));
    // dump layout: n2[12], d1[12], mul_prev[12], mul_prev2[12], mul_in[12]
    // (each: 3 sections broadcast x4, index 4*k+lane)
    for i in 0..12 {
        assert_eq!(rg.n2[i / 4].to_bits(), d.f32_data[i].to_bits(), "n2[{i}]");
        assert_eq!(rg.d1[i / 4].to_bits(), d.f32_data[12 + i].to_bits(), "d1[{i}]");
        assert_eq!(rg.mul_prev[i].to_bits(), d.f32_data[24 + i].to_bits(), "mp[{i}]");
        assert_eq!(rg.mul_prev2[i].to_bits(), d.f32_data[36 + i].to_bits(), "mp2[{i}]");
        assert_eq!(rg.mul_in[i].to_bits(), d.f32_data[48 + i].to_bits(), "mi[{i}]");
    }
    assert_eq!(rg.radius, radius.u32_data[0], "radius");
}

#[test]
fn blur_matches_oracle_dumps() {
    let ctx = VkContext::new().expect("vulkan context");
    let bar = blur_bar(&ctx);
    println!("device {} -> blur bar {bar:e}", ctx.device_name());
    let rg = create_recursive_gaussian(1.5);
    for fixture in ["photo", "step", "gray", "s8"] {
        let lin = Dump::read(dump_path(fixture, "xyb_orig_s0"));
        let (w, h) = (lin.xsize as usize, lin.ysize as usize);
        let buf = ctx.create_buffer_f32(&lin.f32_data).expect("upload");
        let out = blur_planes(&ctx, &buf, w, h, &rg).expect("gpu blur");
        let got = ctx.readback_f32(&out).expect("readback");
        ctx.destroy_buffer(out);
        ctx.destroy_buffer(buf);
        let golden = Dump::read(dump_path(fixture, "mu1_s0"));
        let (d, i) = max_abs_diff(&got, &golden.f32_data);
        assert!(d <= bar, "{fixture} mu1: max abs {d:e} at {i}");
        println!("{fixture} mu1: max abs {d:e}");

        let lin2 = Dump::read(dump_path(fixture, "xyb_dist_s0"));
        let buf2 = ctx.create_buffer_f32(&lin2.f32_data).expect("upload");
        let out2 = blur_planes(&ctx, &buf2, w, h, &rg).expect("gpu blur");
        let got2 = ctx.readback_f32(&out2).expect("readback");
        ctx.destroy_buffer(out2);
        ctx.destroy_buffer(buf2);
        let golden2 = Dump::read(dump_path(fixture, "mu2_s0"));
        let (d2, i2) = max_abs_diff(&got2, &golden2.f32_data);
        assert!(d2 <= bar, "{fixture} mu2: max abs {d2:e} at {i2}");
        println!("{fixture} mu2: max abs {d2:e}");
    }
}

fn pattern(kind: u32, w: usize, h: usize) -> Vec<f32> {
    let mut v = vec![0f32; 3 * w * h];
    let mut state: u32 = 0x9e3779b9 ^ (w as u32 * 7 + h as u32);
    let mut rnd = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        (state & 0xFFFFFF) as f32 / 0xFFFFFF as f32
    };
    for c in 0..3 {
        for y in 0..h {
            for x in 0..w {
                let i = c * w * h + y * w + x;
                v[i] = match kind {
                    0 => 0.5,
                    1 => x as f32 / w as f32,
                    2 => {
                        if x < w / 2 {
                            0.9
                        } else {
                            0.1
                        }
                    }
                    3 => {
                        if x == w / 2 && y == h / 2 {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    _ => rnd(),
                };
            }
        }
    }
    v
}

#[test]
fn blur_synthetic_battery() {
    let ctx = VkContext::new().expect("vulkan context");
    let bar = blur_bar(&ctx);
    let rg = create_recursive_gaussian(1.5);
    let sizes = [(8, 8), (9, 9), (12, 15), (16, 17), (64, 8), (67, 101), (8, 64)];
    let mut worst = (0f32, String::new());
    for (w, h) in sizes {
        for kind in 0..5u32 {
            let img = pattern(kind, w, h);
            let cpu = blur_planes_cpu(&img, w, h, &rg);
            let buf = ctx.create_buffer_f32(&img).expect("upload");
            let out = blur_planes(&ctx, &buf, w, h, &rg).expect("gpu blur");
            let got = ctx.readback_f32(&out).expect("readback");
            ctx.destroy_buffer(out);
            ctx.destroy_buffer(buf);
            let (d, i) = max_abs_diff(&got, &cpu);
            if d > worst.0 {
                worst = (d, format!("{w}x{h} kind{kind} idx{i}"));
            }
            assert!(d <= bar, "gpu-vs-naive {w}x{h} kind{kind}: {d:e} at {i}");
        }
    }
    println!("synthetic battery worst drift: {worst:?} (bar {bar:e})");
}
