// Host-side constants + runner for the XYB / positive-XYB kernel.
// Constants transcribed from opsin_params.h:20-66 with the source's own
// float arithmetic (complements computed as subtractions, never retyped
// decimals); premul/negcbrt per enc_xyb.cc:215-223.
use crate::context::{GpuBuffer, VkContext};

const KM00: f32 = 0.30;
const KM02: f32 = 0.078;
const KM01: f32 = 1.0 - KM02 - KM00;
const KM10: f32 = 0.23;
const KM12: f32 = 0.078;
const KM11: f32 = 1.0 - KM12 - KM10;
const KM20: f32 = 0.24342268924547819;
const KM21: f32 = 0.20476744424496821;
const KM22: f32 = 1.0 - KM20 - KM21;
const KB0: f32 = 0.0037930732552754493;

/// IntensityTarget default from the oracle metadata path (255 -> scale 1.0).
pub fn premul_absorb(intensity_target: f32) -> [f32; 9] {
    let s = intensity_target / 255.0f32;
    [
        KM00 * s, KM01 * s, KM02 * s, KM10 * s, KM11 * s, KM12 * s, KM20 * s, KM21 * s, KM22 * s,
    ]
}

pub fn opsin_bias() -> [f32; 3] {
    [KB0, KB0, KB0]
}

/// -cbrtf(bias) as computed by the oracle (enc_xyb.cc:221). NOTE: Rust's
/// f32::cbrt routes to UCRT libm which is 1 ulp off the oracle's mingw cbrtf
/// for this value (observed 2026-09-08: 0xbe1fb276 vs 0xbe1fb275, shifting 43%
/// of XYB pixels). Compute via f64 (correctly rounded) instead; the
/// xyb_parity test asserts bit-exactness against the oracle dump.
pub fn neg_bias_cbrt() -> [f32; 3] {
    let v = -((KB0 as f64).cbrt() as f32);
    [v, v, v]
}

#[repr(C)]
struct XybPush {
    m: [f32; 9],
    bias: [f32; 3],
    negcbrt: [f32; 3],
    n: u32,
    positive: u32,
}

fn bytes_of(p: &XybPush) -> Vec<u8> {
    let mut v = Vec::with_capacity(68);
    for f in p.m.iter().chain(p.bias.iter()).chain(p.negcbrt.iter()) {
        v.extend_from_slice(&f.to_le_bytes());
    }
    v.extend_from_slice(&p.n.to_le_bytes());
    v.extend_from_slice(&p.positive.to_le_bytes());
    v
}

/// Convert a plane-major linear-RGB buffer (3*n f32) to XYB on the GPU.
/// `positive` selects the MakePositiveXYB rescale. Returns the output buffer.
pub fn xyb_convert(
    ctx: &VkContext,
    linear: &GpuBuffer,
    n: usize,
    positive: bool,
) -> Result<GpuBuffer, String> {
    let out = ctx.create_empty(3 * n)?;
    let push = XybPush {
        m: premul_absorb(255.0),
        bias: opsin_bias(),
        negcbrt: neg_bias_cbrt(),
        n: n as u32,
        positive: positive as u32,
    };
    if let Err(e) = ctx.run_compute_push(
        include_bytes!("../shaders/xyb_positive.spv"),
        crate::c_main(),
        &[linear, &out],
        ((n as u32) + 63) / 64,
        &bytes_of(&push),
    ) {
        ctx.destroy_buffer(out);
        return Err(e);
    }
    Ok(out)
}
