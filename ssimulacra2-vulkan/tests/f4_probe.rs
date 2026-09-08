#![allow(clippy::manual_div_ceil)] // transcription of oracle rounding-up forms
// F4 probe (human-approved CI round-trip, 2026-09-08): localize the llvmpipe
// identity anomaly by driving the PRODUCTION maps_combine kernel with
// synthetic inputs that isolate each expression tree. The kernel's identity
// algebra: mu1==mu2, s11==s22==s12 => num_s and denom_s are the same value
// computed by DIFFERENT trees (delta+delta+kC2 vs (s11-mu11)+(s22-mu22)+kC2),
// so d must be exactly 0 on any spec-compliant device. Each case below
// exercises one branch of that structure; the CPU expectation mirrors the
// shader op-for-op in Rust f32 (correctly rounded, never contracted).
// On IEEE-fma devices every case is asserted bit-exact (harness validation +
// strict regression). On non-IEEE devices (llvmpipe on CI) results are
// PRINTED, not asserted - the printed per-case divergence pattern is the
// probe's answer: which tree diverges, at which value regime.
use ssimulacra2_vulkan::context::VkContext;

const K_C2: f32 = 0.0009;
const N: usize = 4096; // 64x64, 3 channels => 12288 elements

fn cpu_d(m1: f32, m2: f32, s11: f32, s22: f32, s12: f32) -> f32 {
    let mu11 = m1 * m1;
    let mu22 = m2 * m2;
    let mu12 = m1 * m2;
    let dm = m1 - m2;
    let dmsq = dm * dm;
    let num_m = 1.0 - dmsq;
    let delta = s12 - mu12;
    let num_s = delta + delta + K_C2;
    let denom_s = (s11 - mu11) + (s22 - mu22) + K_C2;
    let prod = num_m * num_s;
    let q = (prod as f64 / denom_s as f64) as f32;
    (1.0 - q).max(0.0)
}

struct Inputs {
    mu1: Vec<f32>,
    mu2: Vec<f32>,
    s11: Vec<f32>,
    s22: Vec<f32>,
    s12: Vec<f32>,
}

impl Inputs {
    fn zeros() -> Self {
        let v = vec![0.0f32; 3 * N];
        Self { mu1: v.clone(), mu2: v.clone(), s11: v.clone(), s22: v.clone(), s12: v }
    }
}

fn lcg(state: &mut u32) -> f32 {
    *state = state.wrapping_mul(1664525).wrapping_add(1013904223);
    ((*state >> 8) as f32) / ((1u32 << 24) as f32)
}

fn run_case(ctx: &VkContext, name: &str, inp: &Inputs, strict: bool) {
    let zero = vec![0.0f32; 3 * N];
    let mk = |v: &[f32]| ctx.create_buffer_f32(v).unwrap();
    let x1 = mk(&zero);
    let x2 = mk(&zero);
    let b_mu1 = mk(&inp.mu1);
    let b_mu2 = mk(&inp.mu2);
    let b_s11 = mk(&inp.s11);
    let b_s22 = mk(&inp.s22);
    let b_s12 = mk(&inp.s12);
    let sd = ctx.create_empty(3 * N).unwrap();
    let ed = ctx.create_empty(3 * N).unwrap();
    ctx.run_compute_push(
        include_bytes!("../shaders/maps_combine.spv"),
        c"main",
        &[&x1, &x2, &b_mu1, &b_mu2, &b_s11, &b_s22, &b_s12, &sd, &ed],
        (((3 * N) as u32) + 63) / 64,
        &(N as u32).to_le_bytes(),
    )
    .unwrap();
    let got = ctx.readback_f32(&sd).unwrap();
    let exp: Vec<f32> = inp
        .mu1
        .iter()
        .zip(&inp.mu2)
        .zip(&inp.s11)
        .zip(&inp.s22)
        .zip(&inp.s12)
        .map(|((((m1, m2), s11), s22), s12)| cpu_d(*m1, *m2, *s11, *s22, *s12))
        .collect();
    let mut n_diff = 0usize;
    let mut first = usize::MAX;
    let mut max_abs = 0.0f32;
    let mut max_d_gpu = 0.0f32;
    for (i, (g, e)) in got.iter().zip(&exp).enumerate() {
        if g.to_bits() != e.to_bits() {
            n_diff += 1;
            if first == usize::MAX {
                first = i;
            }
            let ad = (g - e).abs();
            if ad > max_abs {
                max_abs = ad;
            }
        }
        if *g > max_d_gpu {
            max_d_gpu = *g;
        }
    }
    println!(
        "F4 probe {name}: {n_diff}/{} differ, max|gpu-cpu|={max_abs:e}, max d_gpu={max_d_gpu:e}{}",
        3 * N,
        if first != usize::MAX { format!(", first at {first}") } else { String::new() }
    );
    if strict {
        assert_eq!(n_diff, 0, "F4 probe {name} must be bit-exact on IEEE-fma device");
    }
    for b in [x1, x2, b_mu1, b_mu2, b_s11, b_s22, b_s12, sd, ed] {
        ctx.destroy_buffer(b);
    }
}

#[test]
fn maps_combine_synthetic_bisection() {
    let ctx = VkContext::new().expect("vulkan context");
    let strict = ctx.fma_ieee();

    // A: mu=0, s11==s22==s12=s -> isolates num_s vs denom_s trees (delta=s).
    let mut a = Inputs::zeros();
    for i in 0..3 * N {
        let s = 1e-4 + (i % 997) as f32 * 1e-3;
        a.s11[i] = s;
        a.s22[i] = s;
        a.s12[i] = s;
    }
    run_case(&ctx, "A_mu0_sigvar", &a, strict);

    // B: s=0, mu1==mu2=m -> delta = -m^2, both trees carry the same -2m^2+kC2.
    let mut b = Inputs::zeros();
    for i in 0..3 * N {
        let m = 0.01 + (i % 89) as f32 * 0.01;
        b.mu1[i] = m;
        b.mu2[i] = m;
    }
    run_case(&ctx, "B_sig0_muvar", &b, strict);

    // C: delta=0 exactly (s12=mu12, s11=mu11, s22=mu22) -> num_s=kC2 literal
    // vs denom_s=(0)+(0)+kC2: tests whether the trees agree at the constant.
    let mut c = Inputs::zeros();
    for i in 0..3 * N {
        let m = 0.01 + (i % 89) as f32 * 0.01;
        let mm = m * m;
        c.mu1[i] = m;
        c.mu2[i] = m;
        c.s11[i] = mm;
        c.s22[i] = mm;
        c.s12[i] = mm;
    }
    run_case(&ctx, "C_delta0", &c, strict);

    // D: realistic identity regime (m from XYB-like range, s = m^2 + var) -
    // closest synthetic analogue of the failing photo/identical fixture.
    let mut d = Inputs::zeros();
    let mut st = 12345u32;
    for i in 0..3 * N {
        let m = 0.05 + lcg(&mut st) * 0.5;
        let var = 1e-5 + lcg(&mut st) * 1e-2;
        let s = m * m + var;
        d.mu1[i] = m;
        d.mu2[i] = m;
        d.s11[i] = s;
        d.s22[i] = s;
        d.s12[i] = s;
    }
    run_case(&ctx, "D_realistic_identity", &d, strict);

    // E: non-identity (dm != 0, independent sigmas) - general path GPU vs CPU.
    let mut e = Inputs::zeros();
    let mut st = 987654321u32;
    for i in 0..3 * N {
        let m1 = 0.05 + lcg(&mut st) * 0.5;
        let m2 = m1 * (0.9 + lcg(&mut st) * 0.2);
        e.mu1[i] = m1;
        e.mu2[i] = m2;
        e.s11[i] = m1 * m1 + 1e-5 + lcg(&mut st) * 1e-2;
        e.s22[i] = m2 * m2 + 1e-5 + lcg(&mut st) * 1e-2;
        e.s12[i] = m1 * m2 + 1e-5 + lcg(&mut st) * 1e-2;
    }
    run_case(&ctx, "E_nonidentity", &e, strict);
}
