// M2: GPU XYB + positive rescale must match oracle dumps within 1e-6.
use ssimulacra2_vulkan::context::VkContext;
use ssimulacra2_vulkan::oracle_dump::{max_abs_diff, Dump};
use ssimulacra2_vulkan::xyb::xyb_convert;

fn dump_path(fixture: &str, name: &str) -> String {
    format!(
        "{}/../dumps/{fixture}/run1/r0_{name}.bin",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn check_pair(ctx: &VkContext, fixture: &str, which: &str) {
    let lin = Dump::read(dump_path(fixture, &format!("linear_{which}_s0")));
    let n = (lin.xsize * lin.ysize) as usize;
    assert_eq!(lin.channels, 3, "linear input must be 3-plane");
    let buf = ctx.create_buffer_f32(&lin.f32_data).expect("upload");

    for positive in [false, true] {
        let golden_tag = if positive { "xyb" } else { "xyb_pre" };
        let golden = Dump::read(dump_path(fixture, &format!("{golden_tag}_{which}_s0")));
        let out = xyb_convert(ctx, &buf, n, positive).expect("xyb kernel");
        let got = ctx.readback_f32(&out).expect("readback");
        ctx.destroy_buffer(out);
        let (d, i) = max_abs_diff(&got, &golden.f32_data);
        assert!(
            d <= 1e-6,
            "{fixture}/{which} positive={positive}: max abs {d:e} at {i} (got {} want {})",
            got[i],
            golden.f32_data[i]
        );
        println!("{fixture}/{which} positive={positive}: max abs {d:e}");
    }
    ctx.destroy_buffer(buf);
}

#[test]
fn xyb_matches_oracle_dumps() {
    let ctx = VkContext::new().expect("vulkan context");
    for fixture in ["photo", "step", "gray", "s8"] {
        for which in ["orig", "dist"] {
            check_pair(&ctx, fixture, which);
        }
    }
}
