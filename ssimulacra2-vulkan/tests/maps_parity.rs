#![allow(clippy::manual_div_ceil)] // transcription of oracle rounding-up forms
// M4: single-scale error maps + f64 norms vs oracle dumps.
use ssimulacra2_vulkan::blur::{blur_planes, create_recursive_gaussian};
use ssimulacra2_vulkan::context::{GpuBuffer, VkContext};
use ssimulacra2_vulkan::maps::{edge_norms, ssim_norms};
use ssimulacra2_vulkan::oracle_dump::{max_abs_diff, Dump};

fn dump_path(fixture: &str, name: &str) -> String {
    format!("{}/../dumps/{fixture}/run1/r0_{name}.bin", env!("CARGO_MANIFEST_DIR"))
}

fn max_abs_diff_f64(a: &[f64], b: &[f64]) -> (f64, usize) {
    let mut worst = (0f64, 0usize);
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        let d = (x - y).abs();
        if d > worst.0 { worst = (d, i); }
    }
    worst
}

/// min positive |v| and max |v| across slices (FTZ-safety range audit).
fn range_audit(name: &str, slices: &[&[f32]]) -> (f32, f32) {
    let mut minpos = f32::INFINITY;
    let mut maxabs = 0f32;
    for s in slices {
        for &v in *s {
            let a = v.abs();
            if a > 0.0 && a < minpos { minpos = a; }
            if a > maxabs { maxabs = a; }
        }
    }
    println!("range[{name}]: min positive {minpos:e}, max {maxabs:e}");
    assert!(minpos > 1e-30 || minpos == f32::INFINITY, "denormal-range values in {name} - FTZ hazard");
    (minpos, maxabs)
}

#[test]
fn single_scale_maps_and_norms_match_oracle() {
    let ctx = VkContext::new().expect("vulkan context");
    let rg = create_recursive_gaussian(1.5);
    let entry = c"main";
    for fixture in ["photo", "step", "gray", "s8"] {
        let lin1 = Dump::read(dump_path(fixture, "xyb_orig_s0"));
        let lin2 = Dump::read(dump_path(fixture, "xyb_dist_s0"));
        let (w, h) = (lin1.xsize as usize, lin1.ysize as usize);
        let n = w * h;
        // host multiplies (exact f32 products, same op as CPU Multiply)
        let mul11: Vec<f32> = lin1.f32_data.iter().map(|x| x * x).collect();
        let mul22: Vec<f32> = lin2.f32_data.iter().map(|x| x * x).collect();
        let mul12: Vec<f32> = lin1.f32_data.iter().zip(&lin2.f32_data).map(|(a, b)| a * b).collect();
        range_audit(fixture, &[&lin1.f32_data, &lin2.f32_data, &mul11, &mul22, &mul12]);

        let up = |v: &Vec<f32>| ctx.create_buffer_f32(v).expect("upload");
        let b11 = up(&mul11); let b22 = up(&mul22); let b12 = up(&mul12);
        let bi1 = up(&lin1.f32_data); let bi2 = up(&lin2.f32_data);
        let blur = |src: &GpuBuffer| blur_planes(&ctx, src, w, h, &rg).expect("blur");
        let s11 = blur(&b11);
        let s22 = blur(&b22);
        let s12 = blur(&b12);
        let mu1 = blur(&bi1);
        let mu2 = blur(&bi2);
        // localize: GPU blurred planes vs dumps (expect bit-exact)
        for (tag, gb) in [("sigma1_sq", &s11), ("sigma2_sq", &s22), ("sigma12", &s12), ("mu1", &mu1), ("mu2", &mu2)] {
            let g = Dump::read(dump_path(fixture, &format!("{tag}_s0")));
            let r = ctx.readback_f32(gb).expect("rb");
            let (d, i) = max_abs_diff(&r, &g.f32_data);
            println!("{fixture} {tag}: drift {d:e} at {i}");
            assert!(d == 0.0, "{fixture} {tag} not bit-exact: {d:e}");
        }

        let sd = ctx.create_empty(3 * n).expect("alloc");
        let ed = ctx.create_empty(3 * n).expect("alloc");
        let push = (n as u32).to_le_bytes().to_vec();
        ctx.run_compute_push(
            include_bytes!("../shaders/maps_combine.spv"),
            entry,
            &[&bi1, &bi2, &mu1, &mu2, &s11, &s22, &s12, &sd, &ed],
            ((3 * n) as u32 + 63) / 64,
            &push,
        ).expect("combine");
        let sd_r = ctx.readback_f32(&sd).expect("rb");
        let ed_r = ctx.readback_f32(&ed).expect("rb");

        // per-pixel maps vs dumps (dumps are f64; compare in f64)
        for c in 0..3 {
            let g = Dump::read(dump_path(fixture, &format!("ssim_d_c{c}_s0")));
            let gpu: Vec<f64> = sd_r[c * n..(c + 1) * n].iter().map(|&x| x as f64).collect();
            let (d, i) = max_abs_diff_f64(&gpu, &g.f64_data);
            assert!(d <= 2e-6, "{fixture} ssim_d c{c}: {d:e} at {i}");
            let g = Dump::read(dump_path(fixture, &format!("edge_d1_c{c}_s0")));
            let gpu: Vec<f64> = ed_r[c * n..(c + 1) * n].iter().map(|&x| x as f64).collect();
            let (d, i) = max_abs_diff_f64(&gpu, &g.f64_data);
            assert!(d <= 2e-6, "{fixture} edge_d1 c{c}: {d:e} at {i}");
        }
        range_audit(fixture, &[&sd_r, &ed_r]);

        // norms (CPU f64 over GPU maps) vs dumps
        let sdv: Vec<&[f32]> = (0..3).map(|c| &sd_r[c * n..(c + 1) * n]).collect();
        let edv: Vec<&[f32]> = (0..3).map(|c| &ed_r[c * n..(c + 1) * n]).collect();
        let sn = ssim_norms(&sdv, w, h);
        let en = edge_norms(&edv, w, h);
        let gsn = Dump::read(dump_path(fixture, "ssim_norms_s0"));
        let gen = Dump::read(dump_path(fixture, "edge_norms_s0"));
        let (d1, i1) = max_abs_diff_f64(&sn, &gsn.f64_data);
        let (d2, i2) = max_abs_diff_f64(&en, &gen.f64_data);
        println!("{fixture}: ssim_norms drift {d1:e} (idx {i1}), edge_norms drift {d2:e} (idx {i2})");
        assert!(d1 <= 1e-6, "{fixture} ssim_norms: {d1:e} at {i1}");
        assert!(d2 <= 1e-6, "{fixture} edge_norms: {d2:e} at {i2}");

        for b in [b11, b22, b12, bi1, bi2, s11, s22, s12, mu1, mu2, sd, ed] {
            ctx.destroy_buffer(b);
        }
    }
}
