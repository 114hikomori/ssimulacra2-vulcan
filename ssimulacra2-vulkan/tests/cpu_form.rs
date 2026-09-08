use ssimulacra2_vulkan::blur::{blur_planes_cpu, create_recursive_gaussian};
use ssimulacra2_vulkan::oracle_dump::{max_abs_diff, Dump};
fn dump_path(fixture: &str, name: &str) -> String {
    format!("{}/../dumps/{fixture}/run1/r0_{name}.bin", env!("CARGO_MANIFEST_DIR"))
}
#[test]
fn cpu_form_vs_dump() {
    let rg = create_recursive_gaussian(1.5);
    for fixture in ["photo", "step", "gray"] {
        let lin = Dump::read(dump_path(fixture, "xyb_orig_s0"));
        let (w, h) = (lin.xsize as usize, lin.ysize as usize);
        let got = blur_planes_cpu(&lin.f32_data, w, h, &rg);
        let golden = Dump::read(dump_path(fixture, "mu1_s0"));
        let (d, i) = max_abs_diff(&got, &golden.f32_data);
        println!("{fixture}: rust-cpu vs dump max abs {d:e} at {i}");
        assert_eq!(d, 0.0, "{fixture}: CPU blur form not bit-exact vs oracle dump");
    }
}
