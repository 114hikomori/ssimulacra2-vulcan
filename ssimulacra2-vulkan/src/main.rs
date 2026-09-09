// CLI: ssimulacra2-vulkan [--cpu|--gpu] [--profile] original.png distorted.png
// CLI (batch): ssimulacra2-vulkan score-many --orig <original.png> --vars <dir>
//              [--no-cache] [--profile]
// Mirrors the C++ CLI contract: score %.8f on stdout; alpha inputs take the
// worst of bg=0.1/bg=0.9; <8x8 rejected; GPU default with CPU fallback.
// Single-pair default is size-ROUTED (GPU_ROUTE_MIN_PIXELS below); score-many
// is GPU-always (context-init is amortized over the batch - different math).
use ssimulacra2_vulkan::cpu::{alpha_blend, compute_ssimulacra2_cpu, decode_png, to_linear};
use ssimulacra2_vulkan::gpu_pipeline::{
    compute_ssimulacra2_gpu_profiled, list_variants, score_batch_paths, score_nocache_paths,
};
use ssimulacra2_vulkan::profile::Profile;
use ssimulacra2_vulkan::score::{score, ScaleNorms};

/// Below this pixel count the single-pair CLI runs the in-binary CPU engine
/// instead of the GPU. COMPARATOR: this binary's `--cpu` engine (the one
/// routing actually switches to) - NOT the C++ oracle. PROTOCOL: cold
/// MIN-of-3 single-pair CLI, warmed file cache, per-process incl. context
/// init; AMD RX 6600M, 2026-09-09, reproducible via `bench/routing_calib.py`
/// (--gpu-pinned re-run, MIN-of-5, agrees): GPU/CPU = 1.44 @0.31MP,
/// 0.81 @0.61MP, 0.68 @1.02MP, 0.23 @4.19MP -> crossover 0.40-0.45MP.
/// The distinct C++-oracle crossover (0.84-0.93x @8.3MP,
/// cold MIN-of-5 single-process) answers M10 competitiveness, not routing -
/// see README "two crossovers". `--gpu` overrides this default; `--cpu`
/// forces CPU either way.
const GPU_ROUTE_MIN_PIXELS: usize = 500_000;

/// M10-batch: N variants vs one shared original, original-side prep cached
/// across the batch (round 1: caching only, no pipelining; batch CLI, not a
/// daemon). `--no-cache` runs the same variants through the untouched
/// single-shot path in one process - the fair baseline the caching gate is
/// measured against. One line per variant: "<name> <score>".
fn run_score_many(args: &[String], t_start: std::time::Instant) {
    let mut vars_dir: Option<String> = None;
    let mut orig_path: Option<String> = None;
    let mut no_cache = false;
    let mut profile_on = false;
    let mut it = args[2..].iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--orig" => orig_path = it.next().cloned(),
            "--vars" => vars_dir = it.next().cloned(),
            "--no-cache" => no_cache = true,
            "--profile" => profile_on = true,
            other => {
                eprintln!("score-many: unexpected argument {other}");
                eprintln!("{USAGE}");
                std::process::exit(1);
            }
        }
    }
    let (Some(orig_path), Some(vars_dir)) = (orig_path, vars_dir) else {
        eprintln!("score-many requires --orig <file> and --vars <dir>");
        std::process::exit(1);
    };
    let variants = match list_variants(&vars_dir) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("score-many: {e}");
            std::process::exit(1);
        }
    };
    let mut prof = Profile::enabled(profile_on);
    let n = variants.len();
    let results = match ssimulacra2_vulkan::context::VkContext::new() {
        Ok(ctx) => {
            let r = if no_cache {
                score_nocache_paths(&ctx, &orig_path, &variants, &mut prof)
            } else {
                score_batch_paths(&ctx, &orig_path, &variants, &mut prof)
            };
            r.unwrap_or_else(|e| {
                eprintln!("score-many: {e}");
                std::process::exit(1);
            })
        }
        Err(e) => {
            eprintln!("note: no Vulkan device ({e}); CPU fallback, no caching benefit");
            let od = decode_png(&orig_path).unwrap_or_else(|x| {
                eprintln!("score-many: {x}");
                std::process::exit(1);
            });
            let front = |dec: &ssimulacra2_vulkan::cpu::Decoded| -> Vec<f32> {
                match &dec.alpha {
                    Some(al) => to_linear(&alpha_blend(&dec.srgb, al, 0.5, dec.w * dec.h)),
                    None => to_linear(&dec.srgb),
                }
            };
            let ol = front(&od);
            variants
                .iter()
                .map(|vp| {
                    let vd = decode_png(vp).unwrap_or_else(|x| {
                        eprintln!("score-many: {x}");
                        std::process::exit(1);
                    });
                    let vl = front(&vd);
                    let s = compute_ssimulacra2_cpu(&ol, &vl, od.w, od.h);
                    (vp.clone(), score(&s))
                })
                .collect()
        }
    };
    for (path, s) in &results {
        let name = std::path::Path::new(path)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone());
        println!("{name} {s:.8}");
    }
    if profile_on {
        prof.report(t_start.elapsed());
        let secs = t_start.elapsed().as_secs_f64();
        eprintln!("per-image over {n} variants: {:.3} ms", secs / n as f64 * 1e3);
    }
    use std::io::Write;
    let _ = std::io::stdout().flush();
    std::process::exit(0);
}

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

/// Single source of truth for CLI discoverability: a sibling project
/// concluded "batch isn't in this binary's CLI" because the old usage line
/// only showed the single-pair form. Keep every mode listed here.
const USAGE: &str = "\
SSIMULACRA 2.1 (Vulkan port)
Usage (single pair, engine auto-routed at 0.5 MP):
  ssimulacra2-vulkan [--cpu|--gpu] [--profile] original.png distorted.png
Usage (batch: one original vs a directory of same-size variants,
original-side preprocessing cached across the batch):
  ssimulacra2-vulkan score-many --orig <original.png> --vars <dir> [--no-cache] [--profile]
Flags:
  --cpu / --gpu   force an engine for the single-pair path (mutually exclusive)
  --no-cache      (score-many) same variants through the uncached path - the
                  fair baseline the caching win is measured against
  --profile       per-stage wall-time breakdown on stderr
  --help, -h      this text";

fn main() {
    let t_start = std::time::Instant::now();
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s.as_str()) == Some("score-many") {
        run_score_many(&args, t_start);
        return;
    }
    if args.iter().skip(1).any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return;
    }
    let mut force_cpu = false;
    let mut force_gpu = false;
    let mut profile_on = false;
    let mut pos = Vec::new();
    for a in &args[1..] {
        match a.as_str() {
            "--cpu" => force_cpu = true,
            "--gpu" => force_gpu = true,
            "--profile" => profile_on = true,
            _ => pos.push(a.clone()),
        }
    }
    if force_cpu && force_gpu {
        eprintln!("--cpu and --gpu are mutually exclusive");
        std::process::exit(1);
    }
    let mut prof = Profile::enabled(profile_on);
    if pos.len() != 2 {
        eprintln!("{USAGE}");
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
    // Engine routing (see GPU_ROUTE_MIN_PIXELS): single-pair only; score-many
    // owns its context and stays GPU-always (amortized init).
    let routed_cpu = !force_gpu && (a.w * a.h) < GPU_ROUTE_MIN_PIXELS;
    let ctx = if force_cpu || routed_cpu {
        if routed_cpu && !force_cpu {
            eprintln!(
                "note: {:.2} MP below routing threshold; CPU engine (--gpu to override)",
                (a.w * a.h) as f64 / 1e6
            );
        }
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
