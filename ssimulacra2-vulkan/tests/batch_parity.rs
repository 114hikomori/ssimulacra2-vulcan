// M10-batch: the prep/compare split + original-caching path must be
// bit-identical to the untouched single-shot golden path (the whole corpus's
// 26-CI-run history). This is the round-1 correctness gate from the GO.
use ssimulacra2_vulkan::context::VkContext;
use ssimulacra2_vulkan::gpu_pipeline::{compare_gpu, compute_ssimulacra2_gpu, prep_gpu, score_batch_gpu};
use ssimulacra2_vulkan::oracle_dump::Dump;
use ssimulacra2_vulkan::score::{score, ScaleNorms};

fn dump(fixture: &str, name: &str) -> Dump {
    Dump::read(format!(
        "{}/../dumps/{fixture}/run1/r0_{name}.bin",
        env!("CARGO_MANIFEST_DIR")
    ))
}

fn eq_norms(a: &[ScaleNorms], b: &[ScaleNorms]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| {
            x.avg_ssim.iter().zip(&y.avg_ssim).all(|(p, q)| p == q)
                && x.avg_edgediff.iter().zip(&y.avg_edgediff).all(|(p, q)| p == q)
        })
}

#[test]
fn batch_cached_equals_single_shot_bit_exact() {
    let ctx = VkContext::new().expect("vulkan context");
    for fixture in ["photo", "grad", "noise", "step", "odd", "s8", "s9", "s12", "s15", "alpha", "gray"] {
        let l1 = dump(fixture, "linear_orig_s0");
        let l2 = dump(fixture, "linear_dist_s0");
        let (w, h) = (l1.xsize as usize, l1.ysize as usize);
        let single = compute_ssimulacra2_gpu(&ctx, &l1.f32_data, &l2.f32_data, w, h).expect("single");
        let mut prof = ssimulacra2_vulkan::profile::Profile::default();
        let batch = score_batch_gpu(&ctx, &l1.f32_data, w, h, &[&l2.f32_data[..]], &mut prof)
            .expect("batch");
        // Third path: the unfused split (prep each side, then compare) - all
        // three independent code paths must agree bit-for-bit.
        let mut prof2 = ssimulacra2_vulkan::profile::Profile::default();
        let pa = prep_gpu(&ctx, &l1.f32_data, w, h, &mut prof2).expect("prep a");
        let pb = prep_gpu(&ctx, &l2.f32_data, w, h, &mut prof2).expect("prep b");
        let split = compare_gpu(&ctx, &pa, &pb, &mut prof2).expect("split compare");
        assert_eq!(batch.len(), 1, "{fixture}: batch produced {} results", batch.len());
        assert!(eq_norms(&single, &batch[0]), "{fixture}: fused batch norms != single-shot");
        assert!(eq_norms(&single, &split[..]), "{fixture}: split compare norms != single-shot");
        assert_eq!(score(&single), score(&batch[0]), "{fixture}: fused batch score != single-shot");
        assert_eq!(score(&single), score(&split), "{fixture}: split compare score != single-shot");
        println!("{fixture}: single == fused == split bit-identical ({})", score(&single));
    }
}

#[test]
fn batch_shares_one_original_prep_without_bleed() {
    let ctx = VkContext::new().expect("vulkan context");
    let l1 = dump("photo", "linear_orig_s0");
    let l2 = dump("photo", "linear_dist_s0");
    let (w, h) = (l1.xsize as usize, l1.ysize as usize);
    // Three variants against ONE cached original: the dist (real), the dist
    // again (repeat determinism), and the original itself (identity -> 100.0).
    let variants: Vec<&[f32]> = vec![&l2.f32_data[..], &l2.f32_data[..], &l1.f32_data[..]];
    let mut prof = ssimulacra2_vulkan::profile::Profile::default();
    let batch = score_batch_gpu(&ctx, &l1.f32_data, w, h, &variants, &mut prof).expect("batch");
    let single_dist = compute_ssimulacra2_gpu(&ctx, &l1.f32_data, &l2.f32_data, w, h).unwrap();
    let single_id = compute_ssimulacra2_gpu(&ctx, &l1.f32_data, &l1.f32_data, w, h).unwrap();
    assert_eq!(batch.len(), 3);
    assert!(eq_norms(&single_dist, &batch[0]), "variant0 (dist) differs");
    assert!(eq_norms(&single_dist, &batch[1]), "variant1 (dist repeat) differs");
    assert!(eq_norms(&single_id, &batch[2]), "variant2 (identity) differs");
    assert_eq!(score(&batch[2]), 100.0, "identity via cached batch must be exactly 100");
    println!("shared-original batch: 3 variants, no bleed, identity=100.0 exact");
}
