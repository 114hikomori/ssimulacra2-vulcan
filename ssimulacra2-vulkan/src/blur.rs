// Recursive Gaussian blur (Charalampidis 2016 truncated-cosine IIR), transcribed
// from src/lib/jxl/gauss_blur.cc:499-590 (constants) and the scan forms:
// horizontal = 3-phase (border lane-0 / 4-output unrolled / remainder) exactly
// as FastGaussian1D (:40-201); vertical = naive recurrence as VerticalBlock
// (:254-373) including SingleInput border semantics (no +0 add).
use crate::context::{GpuBuffer, VkContext};

#[derive(Clone, Copy, Debug)]
pub struct RgConst {
    pub radius: u32,
    pub n2: [f32; 3],
    pub d1: [f32; 3],
    pub mul_prev: [f32; 12],
    pub mul_prev2: [f32; 12],
    pub mul_in: [f32; 12],
}

/// Derivation in f64 exactly as CreateRecursiveGaussian does, then cast to f32.
pub fn create_recursive_gaussian(sigma: f64) -> RgConst {
    const PI: f64 = 3.141592653589793238;
    let radius_f = (3.2795 * sigma + 0.2546).round(); // (57)
    let radius = radius_f as usize;
    let pd2r = PI / (2.0 * radius_f);
    let omega = [pd2r, 3.0 * pd2r, 5.0 * pd2r];
    let p = [
        1.0 / (0.5 * omega[0]).tan(),
        -1.0 / (0.5 * omega[1]).tan(),
        1.0 / (0.5 * omega[2]).tan(),
    ];
    let r = [
        p[0] * p[0] / omega[0].sin(),
        -p[1] * p[1] / omega[1].sin(),
        p[2] * p[2] / omega[2].sin(),
    ];
    let ns2 = -0.5 * sigma * sigma;
    let recip_r = 1.0 / radius_f;
    let rho = [
        (ns2 * omega[0] * omega[0]).exp() * recip_r,
        (ns2 * omega[1] * omega[1]).exp() * recip_r,
        (ns2 * omega[2] * omega[2]).exp() * recip_r,
    ];
    // (52): k pairs (1,3), (3,5), (5,1) over array indices (0,1), (1,2), (2,0)
    let d13 = p[0] * r[1] - r[0] * p[1];
    let d35 = p[1] * r[2] - r[1] * p[2];
    let d51 = p[2] * r[0] - r[2] * p[0];
    let recip_d13 = 1.0 / d13;
    let zeta15 = d35 * recip_d13;
    let zeta35 = d51 * recip_d13;
    // A rows: [p1 p3 p5; r1 r3 r5; zeta15 zeta35 1]  (56)
    let mut a = [p[0], p[1], p[2], r[0], r[1], r[2], zeta15, zeta35, 1.0];
    inv3x3(&mut a);
    let gamma = [
        1.0,
        radius_f * radius_f - sigma * sigma, // (55)
        zeta15 * rho[0] + zeta35 * rho[1] + rho[2],
    ];
    // beta = A * gamma, row-major dot in order (MatMul, linalg.h:100-114)
    let mut beta = [0.0; 3];
    for y in 0..3 {
        let mut e = 0.0;
        for z in 0..3 {
            e += a[y * 3 + z] * gamma[z];
        }
        beta[y] = e;
    }
    let mut n2 = [0f32; 3];
    let mut d1 = [0f32; 3];
    let mut mul_prev = [0f32; 12];
    let mut mul_prev2 = [0f32; 12];
    let mut mul_in = [0f32; 12];
    for i in 0..3 {
        let n2d = -(beta[i] * (omega[i] * (radius_f + 1.0)).cos()); // (33)
        let d1d = -2.0 * omega[i].cos(); // (33)
        n2[i] = n2d as f32;
        d1[i] = d1d as f32;
        // sympy-expanded 4-output coefficients (gauss_blur.cc:565-587), f64
        let d_2 = d1d * d1d;
        let mp = [-d1d, d_2 - 1.0, -d_2 * d1d + 2.0 * d1d, d_2 * d_2 - 3.0 * d_2 + 1.0];
        let mp2 = [-1.0, d1d, -d_2 + 1.0, d_2 * d1d - 2.0 * d1d];
        let mi = [
            n2d,
            -d1d * n2d,
            d_2 * n2d - n2d,
            -d_2 * d1d * n2d + 2.0 * d1d * n2d,
        ];
        for l in 0..4 {
            mul_prev[4 * i + l] = mp[l] as f32;
            mul_prev2[4 * i + l] = mp2[l] as f32;
            mul_in[4 * i + l] = mi[l] as f32;
        }
    }
    RgConst {
        radius: radius as u32,
        n2,
        d1,
        mul_prev,
        mul_prev2,
        mul_in,
    }
}

/// linalg.h:170-200, verbatim (double intermediates).
fn inv3x3(m: &mut [f64; 9]) {
    let mut t = [0.0f64; 9];
    t[0] = m[4] * m[8] - m[5] * m[7];
    t[1] = m[2] * m[7] - m[1] * m[8];
    t[2] = m[1] * m[5] - m[2] * m[4];
    t[3] = m[5] * m[6] - m[3] * m[8];
    t[4] = m[0] * m[8] - m[2] * m[6];
    t[5] = m[2] * m[3] - m[0] * m[5];
    t[6] = m[3] * m[7] - m[4] * m[6];
    t[7] = m[1] * m[6] - m[0] * m[7];
    t[8] = m[0] * m[4] - m[1] * m[3];
    let det = m[0] * t[0] + m[1] * t[3] + m[2] * t[6];
    assert!(det.abs() >= 1e-10, "singular matrix");
    let idet = 1.0 / det;
    for i in 0..9 {
        m[i] = t[i] * idet;
    }
}

/// Upload layout (f32 words): [0..48) = n2,d1,mul_prev,mul_prev2,mul_in (each
/// 3 sections x 4 lanes, broadcast x4 like the oracle struct); [48] = radius
/// (as f32 bits via from_bits path below); rest padding to 64.
pub fn rg_upload(rg: &RgConst) -> Vec<f32> {
    let mut v = vec![0f32; 64];
    for k in 0..3 {
        for l in 0..4 {
            v[4 * k + l] = rg.n2[k];
            v[12 + 4 * k + l] = rg.d1[k];
            v[24 + 4 * k + l] = rg.mul_prev[4 * k + l];
            v[36 + 4 * k + l] = rg.mul_prev2[4 * k + l];
            v[48 + 4 * k + l] = rg.mul_in[4 * k + l];
        }
    }
    v
}

fn push_bytes(rg: &RgConst, w: u32, h: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity(12);
    v.extend_from_slice(&w.to_le_bytes());
    v.extend_from_slice(&h.to_le_bytes());
    v.extend_from_slice(&rg.radius.to_le_bytes());
    v
}

/// GPU blur of a 3-plane image (plane-major f32, w*h per plane): H pass then
/// V pass, matching FastGaussian order (gauss_blur.cc:615-620).
pub fn blur_planes(
    ctx: &VkContext,
    input: &GpuBuffer,
    w: usize,
    h: usize,
    rg: &RgConst,
) -> Result<GpuBuffer, String> {
    let n = 3 * w * h;
    let rgbuf = ctx.create_buffer_f32(&rg_upload(rg))?;
    let temp = ctx.create_empty(n)?;
    let out = ctx.create_empty(n)?;
    let push = push_bytes(rg, w as u32, h as u32);
    let entry = c"main";
    ctx.run_compute_push(
        include_bytes!("../shaders/blur_h.spv"),
        entry,
        &[input, &temp, &rgbuf],
        ((3 * h) as u32 + 63) / 64,
        &push,
    )?;
    ctx.run_compute_push(
        include_bytes!("../shaders/blur_v.spv"),
        entry,
        &[&temp, &out, &rgbuf],
        ((3 * w) as u32 + 63) / 64,
        &push,
    )?;
    ctx.destroy_buffer(temp);
    ctx.destroy_buffer(rgbuf);
    Ok(out)
}

/// CPU-side reference, same forms as the shaders (test oracle for the battery).
pub fn blur_planes_cpu(img: &[f32], w: usize, h: usize, rg: &RgConst) -> Vec<f32> {
    let n = 3 * w * h;
    assert_eq!(img.len(), n);
    let nw = rg.radius as i64;
    let mut temp = vec![0f32; n];
    let mut out = vec![0f32; n];
    for c in 0..3 {
        let base = c * w * h;
        // ---- horizontal (3-phase, FastGaussian1D transcription) ----
        for y in 0..h {
            let row_in = &img[base + y * w..base + (y + 1) * w];
            let row_out = &mut temp[base + y * w..base + (y + 1) * w];
            blur_1d(row_in, row_out, rg);
        }
        // ---- vertical (naive form, VerticalBlock transcription) ----
        for x in 0..w {
            let mut y1 = [0f32; 3];
            let mut y2 = [0f32; 3];
            for ni in (-(nw - 1))..h as i64 {
                let top = ni - nw - 1;
                let bottom = ni + nw - 1;
                // SingleInput (top<0): no add. Bottom border: TwoInputs with
                // zero row (add happens, -0 -> +0). Interior: plain add.
                let sum = if top < 0 {
                    if bottom < h as i64 { temp[base + bottom as usize * w + x] } else { 0.0 }
                } else if bottom >= h as i64 {
                    temp[base + top as usize * w + x] + 0.0
                } else {
                    temp[base + top as usize * w + x] + temp[base + bottom as usize * w + x]
                };
                let mut o = [0f32; 3];
                for k in 0..3 {
                    let inner = (-rg.d1[k]).mul_add(y1[k], -y2[k]);
                    o[k] = rg.n2[k].mul_add(sum, inner);
                    y2[k] = y1[k];
                    y1[k] = o[k];
                }
                if ni >= 0 {
                    out[base + ni as usize * w + x] = o[0] + (o[1] + o[2]);
                }
            }
        }
    }
    out
}

/// One row/column of the horizontal 3-phase scan (lane-0 border, 4-output
/// unrolled interior, lane-0 remainder) - exact FastGaussian1D semantics.
pub fn blur_1d(src: &[f32], dst: &mut [f32], rg: &RgConst) {
    let nw = rg.radius as i64;
    let len = src.len() as i64;
    let first_aligned = ((nw + 1 + 3) / 4) * 4; // RoundUpTo(N+1, 4)
    let mut y1 = [0f32; 3];
    let mut y2 = [0f32; 3];
    let mut n = -nw + 1;
    // border (lane-0 form)
    while n < first_aligned.min(len) {
        let left = n - nw - 1;
        let right = n + nw - 1;
        let lv = if left >= 0 { src[left as usize] } else { 0.0 };
        let rv = if right < len { src[right as usize] } else { 0.0 };
        let sum = lv + rv;
        let mut o = [0f32; 3];
        for k in 0..3 {
            o[k] = rg.mul_prev[4 * k].mul_add(
                y1[k],
                rg.mul_prev2[4 * k].mul_add(y2[k], sum * rg.mul_in[4 * k]),
            );
            y2[k] = y1[k];
            y1[k] = o[k];
        }
        if n >= 0 {
            dst[n as usize] = o[0] + (o[1] + o[2]);
        }
        n += 1;
    }
    // unrolled groups of 4
    while n < len - nw + 1 - 3 {
        let mut sums = [0f32; 4];
        for j in 0..4i64 {
            sums[j as usize] = src[(n + j - nw - 1) as usize] + src[(n + j + nw - 1) as usize];
        }
        let mut outs = [[0f32; 4]; 3];
        for k in 0..3 {
            for l in 0..4i64 {
                let mut o = sums[0] * rg.mul_in[4 * k + l as usize];
                for j in 1..4i64 {
                    let c = if l >= j { rg.mul_in[4 * k + (l - j) as usize] } else { 0.0 };
                    o = c.mul_add(sums[j as usize], o);
                }
                o = rg.mul_prev2[4 * k + l as usize].mul_add(y2[k], o);
                o = rg.mul_prev[4 * k + l as usize].mul_add(y1[k], o);
                outs[k][l as usize] = o;
            }
            y2[k] = outs[k][2];
            y1[k] = outs[k][3];
        }
        for l in 0..4i64 {
            dst[(n + l) as usize] = outs[0][l as usize] + (outs[1][l as usize] + outs[2][l as usize]);
        }
        n += 4;
    }
    // remainder (lane-0 form)
    while n < len {
        let left = n - nw - 1;
        let right = n + nw - 1;
        let lv = if left >= 0 { src[left as usize] } else { 0.0 };
        let rv = if right < len { src[right as usize] } else { 0.0 };
        let sum = lv + rv;
        let mut o = [0f32; 3];
        for k in 0..3 {
            o[k] = rg.mul_prev[4 * k].mul_add(
                y1[k],
                rg.mul_prev2[4 * k].mul_add(y2[k], sum * rg.mul_in[4 * k]),
            );
            y2[k] = y1[k];
            y1[k] = o[k];
        }
        dst[n as usize] = o[0] + (o[1] + o[2]);
        n += 1;
    }
}
