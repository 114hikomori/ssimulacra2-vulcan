// CLI: ssimulacra2-vulkan [--cpu] [--profile] original.png distorted.png
// Mirrors the C++ CLI contract: score %.8f on stdout; alpha inputs take the
// worst of bg=0.1/bg=0.9; <8x8 rejected; GPU default with CPU fallback.
use ssimulacra2_vulkan::cpu::{alpha_blend, compute_ssimulacra2_cpu, decode_png, to_linear};
use ssimulacra2_vulkan::gpu_pipeline::compute_ssimulacra2_gpu_profiled;
use ssimulacra2_vulkan::profile::Profile;
use ssimulacra2_vulkan::score::{score, ScaleNorms};

fn pipeline(
    ctx: Option<&ssimulacra2_vulkan::context::VkContext>,
    lin1: &[f32],
    lin2: &[f32],
    w: usize,
    h: usize,
    prof: &mut Profile,
) -> Vec<ScaleNorms> {
    if let Some(ctx) = ctx {
        match compute_ssimulacra2_gpu_profiled(ctx, lin1, lin2, w, h, prof) {
            Ok(s) => return s,
            Err(e) => eprintln!("note: GPU path failed ({e}); falling back to CPU"),
        }
    }
    prof.time("cpu-path", || compute_ssimulacra2_cpu(lin1, lin2, w, h))
}

fn score_pair(
    ctx: Option<&ssimulacra2_vulkan::context::VkContext>,
    a: &ssimulacra2_vulkan::cpu::Decoded,
    b: &ssimulacra2_vulkan::cpu::Decoded,
    bg: f32,
    prof: &mut Profile,
) -> f64 {
    let n = a.w * a.h;
    let (l1, l2) = prof.time("prep", || {
        // H3: 8-bit alpha-free pixels are on the k/255 grid -> 256-entry LUT
        // (bit-identical, see cpu.rs), and reading srgb directly skips the
        // old 50 MB clone. Alpha-blended pixels are arbitrary floats -> keep
        // the exact per-element path.
        let lut = ssimulacra2_vulkan::cpu::linearize_lut();
        let lin = |dec: &ssimulacra2_vulkan::cpu::Decoded| -> Vec<f32> {
            match &dec.alpha {
                Some(al) => to_linear(&alpha_blend(&dec.srgb, al, bg, n)),
                None => ssimulacra2_vulkan::cpu::to_linear_8bit(&dec.srgb, &lut),
            }
        };
        (lin(a), lin(b))
    });
    let scales = pipeline(ctx, &l1, &l2, a.w, a.h, prof);
    prof.time("score", || score(&scales))
}

fn main() {
    let t_start = std::time::Instant::now();
    let args: Vec<String> = std::env::args().collect();
    let mut force_cpu = false;
    let mut profile_on = false;
    let mut pos = Vec::new();
    for a in &args[1..] {
        match a.as_str() {
            "--cpu" => force_cpu = true,
            "--profile" => profile_on = true,
            _ => pos.push(a.clone()),
        }
    }
    let mut prof = Profile::enabled(profile_on);
    if pos.len() != 2 {
        eprintln!("SSIMULACRA 2.1 (Vulkan port)");
        eprintln!("Usage: ssimulacra2-vulkan [--cpu] [--profile] original.png distorted.png");
        std::process::exit(1);
    }
    let a = match prof.time("decode", || decode_png(&pos[0])) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Could not load original image: {e}");
            std::process::exit(1);
        }
    };
    if a.w < 8 || a.h < 8 {
        eprintln!("Minimum image size is 8x8 pixels");
        std::process::exit(1);
    }
    let b = match prof.time("decode", || decode_png(&pos[1])) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Could not load distorted image: {e}");
            std::process::exit(1);
        }
    };
    if a.w != b.w || a.h != b.h {
        eprintln!("Image size mismatch");
        std::process::exit(1);
    }
    let ctx = if force_cpu {
        None
    } else {
        match prof.time("context-init", ssimulacra2_vulkan::context::VkContext::new) {
            Ok(c) => Some(c),
            Err(e) => {
                eprintln!("note: no Vulkan device ({e}); using CPU path");
                None
            }
        }
    };
    // Oracle contract (ssimulacra2_main.cc:105-114): the worst-of-bg{0.1,0.9}
    // dual pass fires ONLY when the ORIGINAL image has alpha; if only the
    // distorted image has alpha it is a single pass with bg=0.5 (the library
    // default), which blends each alpha-bearing image.
    let has_alpha = a.alpha.is_some();
    let s = if has_alpha {
        let s0 = score_pair(ctx.as_ref(), &a, &b, 0.1, &mut prof);
        let s1 = score_pair(ctx.as_ref(), &a, &b, 0.9, &mut prof);
        s0.min(s1)
    } else {
        score_pair(ctx.as_ref(), &a, &b, 0.5, &mut prof)
    };
    println!("{:.8}", s);
    prof.report(t_start.elapsed());
}
