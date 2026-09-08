// Faithful CPU reimplementation: decode (PNG 8-bit) -> sRGB->linear (TF_SRGB
// rational polynomial) -> alpha blend -> XYB -> 6-scale pipeline -> score.
// Doubles as the runtime fallback and the parity bridge to the C++ oracle.
// Every arithmetic step mirrors the oracle's exact operation order; the
// cpu_parity test asserts bit-exactness against oracle dumps.
use crate::blur::{blur_planes_cpu, create_recursive_gaussian};
use crate::maps::tothe4th;
use crate::score::ScaleNorms;

pub struct Decoded {
    pub w: usize,
    pub h: usize,
    /// plane-major sRGB-encoded floats 0..1 (3 planes; gray replicated)
    pub srgb: Vec<f32>,
    /// alpha plane (0..1) if present
    pub alpha: Option<Vec<f32>>,
}

/// Minimal chunk scan for profile-bearing PNGs (iCCP/gAMA/cHRM/sRGB chunks).
/// Returns Some(chunk_name) if a color-management chunk is present.
pub fn png_profile_chunk(bytes: &[u8]) -> Option<&'static str> {
    for name in [b"iCCP".as_slice(), b"gAMA".as_slice(), b"cHRM".as_slice()] {
        // chunks appear after the 8-byte signature as [len u32be][type][data][crc]
        let mut pos = 8usize;
        while pos + 8 <= bytes.len() {
            let len = u32::from_be_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
            if pos + 8 + len > bytes.len() {
                break;
            }
            if &bytes[pos + 4..pos + 8] == name {
                return Some(match name {
                    b"iCCP" => "iCCP",
                    b"gAMA" => "gAMA",
                    _ => "cHRM",
                });
            }
            if &bytes[pos + 4..pos + 8] == b"IEND".as_slice() {
                break;
            }
            pos += 12 + len;
        }
    }
    None
}

pub fn decode_png(path: &str) -> Result<Decoded, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {path}: {e}"))?;
    if let Some(chunk) = png_profile_chunk(&bytes) {
        return Err(format!(
            "PNG carries a {chunk} chunk (color-managed input); this CLI supports \
             plain sRGB/gray/RGB PNGs - use the C++ oracle binary for profiled images"
        ));
    }
    let decoder = png::Decoder::new(std::io::Cursor::new(&bytes));
    let mut reader = decoder.read_info().map_err(|e| format!("png info: {e}"))?;
    let (w, h) = (reader.info().width as usize, reader.info().height as usize);
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let of = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("png frame: {e}"))?;
    if of.width as usize != w || of.height as usize != h {
        return Err("frame size mismatch".into());
    }
    if of.bit_depth != png::BitDepth::Eight {
        return Err(format!("only 8-bit PNGs supported (got {:?})", of.bit_depth));
    }
    let ct = of.color_type;
    let n = w * h;
    let f = |v: u8| (v as f32) * (1.0f32 / 255.0f32); // ConvertToFloat factor (image_ops.h:463)
    let mut srgb = vec![0f32; 3 * n];
    let alpha = match ct {
        png::ColorType::Grayscale => {
            for i in 0..n {
                let v = f(buf[i]);
                srgb[i] = v;
                srgb[n + i] = v;
                srgb[2 * n + i] = v;
            }
            None
        }
        png::ColorType::Rgb => {
            for i in 0..n {
                srgb[i] = f(buf[3 * i]);
                srgb[n + i] = f(buf[3 * i + 1]);
                srgb[2 * n + i] = f(buf[3 * i + 2]);
            }
            None
        }
        png::ColorType::Rgba => {
            let mut a = vec![0f32; n];
            for i in 0..n {
                srgb[i] = f(buf[4 * i]);
                srgb[n + i] = f(buf[4 * i + 1]);
                srgb[2 * n + i] = f(buf[4 * i + 2]);
                a[i] = f(buf[4 * i + 3]);
            }
            Some(a)
        }
        other => return Err(format!("unsupported color type {other:?}")),
    };
    Ok(Decoded { w, h, srgb, alpha })
}

/// AlphaBlend (ssimulacra2.cc:222-234) in source (sRGB) encoding, f32.
pub fn alpha_blend(srgb: &[f32], alpha: &[f32], bg: f32, n: usize) -> Vec<f32> {
    let mut out = srgb.to_vec();
    for (i, &a) in alpha.iter().enumerate() {
        for c in 0..3 {
            let p = c * n + i;
            out[p] = a * out[p] + (1.0 - a) * bg;
        }
    }
    out
}

/// TF_SRGB DisplayFromEncoded (transfer_functions-inl.h:106-146): piecewise
/// x/12.92 (as x * fl(1/12.92)) vs degree-4 rational polynomial with fma
/// Horner and true division (rational_polynomial-inl.h:57-90, FastDivision
/// float path is `Div` under `#if 1`).
pub fn srgb_to_linear(x: f32) -> f32 {
    const P: [f32; 5] = [
        2.200248328e-04,
        1.043637593e-02,
        1.624820318e-01,
        7.961564959e-01,
        8.210152774e-01,
    ];
    const Q: [f32; 5] = [
        2.631846970e-01,
        1.076976492e+00,
        4.987528350e-01,
        -5.512498495e-02,
        6.521209011e-03,
    ];
    if x > 0.04045f32 {
        let mut yp = P[4];
        let mut yq = Q[4];
        for i in (0..4).rev() {
            yp = yp.mul_add(x, P[i]);
            yq = yq.mul_add(x, Q[i]);
        }
        yp / yq
    } else {
        x * (1.0f32 / 12.92f32)
    }
}

pub fn to_linear(srgb: &[f32]) -> Vec<f32> {
    srgb.iter().map(|&v| srgb_to_linear(v)).collect()
}

/// H3: 256-entry lookup table for 8-bit PNG sRGB values. Entry k is built by
/// applying the SAME `srgb_to_linear` to the SAME expression the decoder uses
/// (`(k as f32) * (1.0/255.0)`), so for any decoder-produced value v=fl(k/255)
/// the table returns exactly the bit pattern per-element to_linear(v) returns.
/// decode_png only accepts BitDepth::Eight, so every alpha-free pixel is on
/// this grid; alpha-blended values are arbitrary floats and must NOT use it
/// (caller gates on provenance). Bit-identity is asserted over all 256 values
/// against to_linear AND against the oracle linear dumps in cpu_parity.
pub fn linearize_lut() -> [f32; 256] {
    let mut t = [0f32; 256];
    let r = 1.0f32 / 255.0f32;
    for (k, slot) in t.iter_mut().enumerate() {
        *slot = srgb_to_linear((k as f32) * r);
    }
    t
}

/// to_linear for values known to be on the k/255 grid (8-bit alpha-free PNGs).
/// Grid recovery: |fl(fl(k/255)*255) - k| < 5e-5 << 0.5 for all k in 0..255,
/// so round() recovers k exactly (asserted for all 256 in cpu_parity).
pub fn to_linear_8bit(srgb: &[f32], lut: &[f32; 256]) -> Vec<f32> {
    srgb
        .iter()
        .map(|&v| {
            let k = (v * 255.0f32).round();
            debug_assert!((0.0..=255.0).contains(&k), "off-grid value in 8-bit path");
            lut[k.clamp(0.0, 255.0) as usize]
        })
        .collect()
}

/// CubeRootAndAdd (fast_math-inl.h:177-210) - same transcription as the shader.
fn cbrt_and_add(x: f32, add: f32) -> f32 {
    const K1_3: f32 = 1.0 / 3.0;
    const K4_3: f32 = 4.0 / 3.0;
    let m1 = x.to_bits() as i32;
    let r = if m1 == 0 {
        0.0f32
    } else {
        let m2 = 0x5480_0000i32.wrapping_sub((m1 >> 23).wrapping_mul(0x002A_AAAA));
        f32::from_bits(m2 as u32)
    };
    let mut r = r;
    let xa3 = K1_3 * x;
    for _ in 0..3 {
        let r2 = r * r;
        let r4 = r2 * r2;
        r = (-xa3).mul_add(r4, K4_3 * r);
    }
    let r2 = r * r;
    let r4 = r2 * r2;
    r = K1_3.mul_add((-x).mul_add(r4, r), r);
    let r2 = r * r;
    r2.mul_add(x, add)
}

/// ToXYB (linear input path) + MakePositiveXYB, per pixel.
pub fn xyb_planes_cpu(lin: &[f32], w: usize, h: usize, positive: bool) -> Vec<f32> {
    use crate::xyb::{neg_bias_cbrt, opsin_bias, premul_absorb};
    let m = premul_absorb(255.0);
    let bias = opsin_bias();
    let nc = neg_bias_cbrt();
    let n = w * h;
    let mut out = vec![0f32; 3 * n];
    for i in 0..n {
        let r = lin[i];
        let g = lin[n + i];
        let b = lin[2 * n + i];
        let mut mixed = [0f32; 3];
        for c in 0..3 {
            let mut v = m[3 * c].mul_add(r, m[3 * c + 1].mul_add(g, m[3 * c + 2].mul_add(b, bias[c])));
            // F6 (BUG_HUNT): max(0.0) matches the oracle's ZeroIfNegative
            // (sign-bit test) and the shader's max() - maps -0.0 to +0.0.
            v = v.max(0.0);
            mixed[c] = cbrt_and_add(v, nc[c]);
        }
        let mut x = 0.5 * (mixed[0] - mixed[1]);
        let mut y = 0.5 * (mixed[0] + mixed[1]);
        let mut bb = mixed[2];
        if positive {
            bb = (bb - y) + 0.55;
            x = x * 14.0 + 0.42;
            y += 0.01;
        }
        out[i] = x;
        out[n + i] = y;
        out[2 * n + i] = bb;
    }
    out
}

/// Downsample (ssimulacra2.cc:44-66) exact order.
pub fn downsample_cpu(src: &[f32], iw: usize, ih: usize) -> (Vec<f32>, usize, usize) {
    let ow = (iw + 1) / 2;
    let oh = (ih + 1) / 2;
    let mut dst = vec![0f32; 3 * ow * oh];
    for c in 0..3 {
        for oy in 0..oh {
            for ox in 0..ow {
                let mut sum = 0f32;
                for iy in 0..2 {
                    let y = (oy * 2 + iy).min(ih - 1);
                    for ix in 0..2 {
                        let x = (ox * 2 + ix).min(iw - 1);
                        sum += src[c * iw * ih + y * iw + x];
                    }
                }
                dst[c * ow * oh + oy * ow + ox] = sum * 0.25;
            }
        }
    }
    (dst, ow, oh)
}

/// Full pipeline on CPU, f64 accumulation exactly as the oracle (d maps stay
/// double here - no f32 quantization like the GPU readback path).
pub fn compute_ssimulacra2_cpu(lin1: &[f32], lin2: &[f32], w: usize, h: usize) -> Vec<ScaleNorms> {
    let rg = create_recursive_gaussian(1.5);
    let mut a = lin1.to_vec();
    let mut b = lin2.to_vec();
    let (mut cw, mut ch) = (w, h);
    let mut scales = Vec::new();
    for scale in 0..6 {
        if cw < 8 || ch < 8 {
            break;
        }
        if scale > 0 {
            let (da, ow, oh) = downsample_cpu(&a, cw, ch);
            let db = downsample_cpu(&b, cw, ch).0;
            a = da;
            b = db;
            cw = ow;
            ch = oh;
        }
        let n = cw * ch;
        let x1 = xyb_planes_cpu(&a, cw, ch, true);
        let x2 = xyb_planes_cpu(&b, cw, ch, true);
        let mul = |p: &[f32], q: &[f32]| -> Vec<f32> { p.iter().zip(q).map(|(x, y)| x * y).collect() };
        let s11 = blur_planes_cpu(&mul(&x1, &x1), cw, ch, &rg);
        let s22 = blur_planes_cpu(&mul(&x2, &x2), cw, ch, &rg);
        let s12 = blur_planes_cpu(&mul(&x1, &x2), cw, ch, &rg);
        let mu1 = blur_planes_cpu(&x1, cw, ch, &rg);
        let mu2 = blur_planes_cpu(&x2, cw, ch, &rg);
        let mut sn = ScaleNorms::default();
        let one_per = 1.0f64 / (n as f64);
        const KC2: f32 = 0.0009f32;
        for c in 0..3 {
            let mut ss = [0f64; 2];
            let mut es = [0f64; 4];
            for y in 0..ch {
                for x in 0..cw {
                    let i = c * n + y * cw + x;
                    let m1 = mu1[i];
                    let m2 = mu2[i];
                    let mu11 = m1 * m1;
                    let mu22 = m2 * m2;
                    let mu12 = m1 * m2;
                    let dm = m1 - m2;
                    let num_m = 1.0 - dm * dm;
                    let num_s = 2.0 * (s12[i] - mu12) + KC2;
                    let denom_s = (s11[i] - mu11) + (s22[i] - mu22) + KC2;
                    let d = 1.0f64 - (num_m * num_s / denom_s) as f64;
                    let d = d.max(0.0);
                    ss[0] += d;
                    ss[1] += tothe4th(d);
                    let t1 = (x1[i] - m1).abs();
                    let t2 = (x2[i] - m2).abs();
                    let d1 = (1.0f64 + t2 as f64) / (1.0f64 + t1 as f64) - 1.0f64;
                    let artifact = d1.max(0.0);
                    let detail_lost = (-d1).max(0.0);
                    es[0] += artifact;
                    es[1] += tothe4th(artifact);
                    es[2] += detail_lost;
                    es[3] += tothe4th(detail_lost);
                }
            }
            sn.avg_ssim[2 * c] = one_per * ss[0];
            sn.avg_ssim[2 * c + 1] = (one_per * ss[1]).sqrt().sqrt();
            sn.avg_edgediff[4 * c] = one_per * es[0];
            sn.avg_edgediff[4 * c + 1] = (one_per * es[1]).sqrt().sqrt();
            sn.avg_edgediff[4 * c + 2] = one_per * es[2];
            sn.avg_edgediff[4 * c + 3] = (one_per * es[3]).sqrt().sqrt();
        }
        scales.push(sn);
    }
    scales
}
