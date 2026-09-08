// Final score: 108-weight sum + polynomial, transcribed from
// ssimulacra2.cc:261-417 (f64, plain ops - x86-64 SSE2 has no f64 fma, so
// contraction cannot differ from the oracle).
include!(concat!(env!("OUT_DIR"), "/weights.rs"));

#[derive(Clone, Copy, Default, Debug)]
pub struct ScaleNorms {
    pub avg_ssim: [f64; 6],
    pub avg_edgediff: [f64; 12],
}

/// Msssim::Score() - exact iteration order (c-major, then scale, then norm)
/// including the weight-index shift when scales are missing.
pub fn score(scales: &[ScaleNorms]) -> f64 {
    weighted_then_poly(weighted_sum(scales))
}

/// The raw 108-term weighted sum, before the polynomial (matches the oracle's
/// value at ssimulacra2.cc:406, pre-scaling).
pub fn weighted_sum(scales: &[ScaleNorms]) -> f64 {
    let mut ssim = 0.0f64;
    let mut i = 0usize;
    for c in 0..3 {
        for sc in scales {
            for n in 0..2 {
                ssim += WEIGHTS[i] * sc.avg_ssim[c * 2 + n].abs();
                i += 1;
                ssim += WEIGHTS[i] * sc.avg_edgediff[c * 4 + n].abs();
                i += 1;
                ssim += WEIGHTS[i] * sc.avg_edgediff[c * 4 + n + 2].abs();
                i += 1;
            }
        }
    }
    ssim
}

/// The post-sum transform, split out so tests can compare the raw weighted
/// sum against the oracle dump before the pow() amplification region.
pub fn weighted_then_poly(ssim: f64) -> f64 {
    let mut ssim = ssim * 0.9562382616834844;
    ssim = 2.326765642916932 * ssim - 0.020884521182843837 * ssim * ssim
        + 6.248496625763138e-05 * ssim * ssim * ssim;
    if ssim > 0.0 {
        100.0 - 10.0 * ssim.powf(0.6276336467831387)
    } else {
        100.0
    }
}
