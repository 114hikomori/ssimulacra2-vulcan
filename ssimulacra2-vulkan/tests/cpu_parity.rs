// CPU fallback path must be bit-exact vs oracle dumps (it mirrors the oracle's
// exact operation order, unlike the GPU path's documented deviations).
use ssimulacra2_vulkan::cpu::{compute_ssimulacra2_cpu, decode_png, to_linear};
use ssimulacra2_vulkan::oracle_dump::Dump;
use ssimulacra2_vulkan::score::score;

fn dump(fixture: &str, name: &str) -> Dump {
    Dump::read(format!("{}/../dumps/{fixture}/run1/r0_{name}.bin", env!("CARGO_MANIFEST_DIR")))
}

#[test]
fn decode_chain_bit_exact_vs_oracle() {
    for fixture in ["photo", "gray", "alpha"] {
        let d = decode_png(&format!("{}/../tests/fixtures/{}_orig.png", env!("CARGO_MANIFEST_DIR"), fixture)).unwrap();
        let n = d.w * d.h;
        let s1 = match &d.alpha {
            Some(a) => ssimulacra2_vulkan::cpu::alpha_blend(&d.srgb, a, 0.1, n),
            None => d.srgb.clone(),
        };
        let lin = to_linear(&s1);
        let g = dump(fixture, "linear_orig_s0");
        assert_eq!(lin.len(), g.f32_data.len());
        let bad: Vec<usize> = (0..lin.len()).filter(|&i| lin[i].to_bits() != g.f32_data[i].to_bits()).collect();
        assert!(bad.is_empty(), "{fixture}: decode chain differs at {} pixels (first {:?})", bad.len(), bad.first());
        println!("{fixture}: decode+linearize bit-exact");
    }
}

// H3: the 256-entry LUT (used for alpha-free 8-bit PNGs) must be bit-identical
// to the per-element path, and the k-recovery (round(v*255)==k) exact, or the
// CLI's prep would silently drift from the oracle.
#[test]
fn to_linear_8bit_lut_bit_exact() {
    use ssimulacra2_vulkan::cpu::{linearize_lut, to_linear, to_linear_8bit};
    let lut = linearize_lut();
    let r = 1.0f32 / 255.0f32;
    for k in 0..=255usize {
        let v = (k as f32) * r; // exactly what decode_png stores
        assert_eq!(
            (v * 255.0f32).round() as usize,
            k,
            "grid recovery failed at k={k}"
        );
        assert_eq!(
            to_linear_8bit(&[v], &lut)[0].to_bits(),
            to_linear(&[v])[0].to_bits(),
            "LUT != per-element at k={k}"
        );
    }
    // And the LUT path must equal the oracle linear dumps for the alpha-free
    // fixtures the CLI runs through it (photo, gray).
    for fixture in ["photo", "gray"] {
        let d = decode_png(&format!(
            "{}/../tests/fixtures/{}_orig.png",
            env!("CARGO_MANIFEST_DIR"),
            fixture
        ))
        .unwrap();
        assert!(d.alpha.is_none(), "{fixture} expected alpha-free");
        let lin = to_linear_8bit(&d.srgb, &lut);
        let g = dump(fixture, "linear_orig_s0");
        let bad: Vec<usize> = (0..lin.len())
            .filter(|&i| lin[i].to_bits() != g.f32_data[i].to_bits())
            .collect();
        assert!(
            bad.is_empty(),
            "{fixture}: LUT differs from oracle at {} (first {:?})",
            bad.len(),
            bad.first()
        );
        println!("{fixture}: LUT bit-exact vs oracle over {} values", lin.len());
    }
}

#[test]
fn cpu_pipeline_bit_exact_vs_oracle() {
    for fixture in ["photo", "step", "gray", "s8", "s15"] {
        let l1 = dump(fixture, "linear_orig_s0");
        let l2 = dump(fixture, "linear_dist_s0");
        let (w, h) = (l1.xsize as usize, l1.ysize as usize);
        let scales = compute_ssimulacra2_cpu(&l1.f32_data, &l2.f32_data, w, h);
        for (s, sn) in scales.iter().enumerate() {
            let g = dump(fixture, &format!("ssim_norms_s{s}"));
            let e = dump(fixture, &format!("edge_norms_s{s}"));
            for c in 0..6 {
                assert_eq!(sn.avg_ssim[c].to_bits(), g.f64_data[c].to_bits(), "{fixture} s{s} ssim[{c}]");
            }
            for c in 0..12 {
                assert_eq!(sn.avg_edgediff[c].to_bits(), e.f64_data[c].to_bits(), "{fixture} s{s} edge[{c}]");
            }
        }
        let got = score(&scales);
        let want = dump(fixture, "score_final_s0").f64_data[0];
        println!("{fixture}: cpu score {got:.8} vs {want:.8}");
        assert!((got - want).abs() <= 1e-9, "{fixture} cpu score drift");
    }
}
