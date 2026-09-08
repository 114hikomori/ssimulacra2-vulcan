// CPU-side f64 norms over read-back per-pixel maps, in the oracle's exact
// accumulation order (ssimulacra2.cc:110-195): row-major y-then-x sums in f64,
// onePerPixels = 1.0/(w*h), 4-norm = sqrt(sqrt(mean(d^4))) with tothe4th(x)
// = (x*x)*(x*x) in f64.
pub fn tothe4th(x: f64) -> f64 {
    let mut x = x;
    x *= x;
    x *= x;
    x
}

/// SSIMMap plane_averages: [c*2] = mean(d), [c*2+1] = 4-norm(d). d maps are
/// f32 (the oracle's d is double but computed from f32 operands; the maps
/// module documents the deviation).
pub fn ssim_norms(d_maps: &[&[f32]], w: usize, h: usize) -> [f64; 6] {
    assert_eq!(d_maps.len(), 3);
    let one_per = 1.0f64 / (w * h) as f64;
    let mut out = [0f64; 6];
    for (c, dm) in d_maps.iter().enumerate() {
        let mut s1 = 0f64;
        let mut s4 = 0f64;
        for &x in dm.iter() {
            let d = x as f64;
            s1 += d;
            s4 += tothe4th(d);
        }
        out[2 * c] = one_per * s1;
        out[2 * c + 1] = (one_per * s4).sqrt().sqrt();
    }
    out
}

/// EdgeDiffMap plane_averages: per channel [0]=mean(artifact), [1]=4norm,
/// [2]=mean(detail_lost), [3]=4norm. d1 maps are f32 (oracle: double).
pub fn edge_norms(d1_maps: &[&[f32]], w: usize, h: usize) -> [f64; 12] {
    assert_eq!(d1_maps.len(), 3);
    let one_per = 1.0f64 / (w * h) as f64;
    let mut out = [0f64; 12];
    for (c, dm) in d1_maps.iter().enumerate() {
        let mut s = [0f64; 4];
        for &x in dm.iter() {
            let d1 = x as f64;
            let artifact = d1.max(0.0);
            let detail_lost = (-d1).max(0.0);
            s[0] += artifact;
            s[1] += tothe4th(artifact);
            s[2] += detail_lost;
            s[3] += tothe4th(detail_lost);
        }
        out[4 * c] = one_per * s[0];
        out[4 * c + 1] = (one_per * s[1]).sqrt().sqrt();
        out[4 * c + 2] = one_per * s[2];
        out[4 * c + 3] = (one_per * s[3]).sqrt().sqrt();
    }
    out
}
