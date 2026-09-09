// End-to-end GPU pipeline: mirrors ComputeSSIMULACRA2's scale loop
// (ssimulacra2.cc:449-485) exactly - break rule, per-scale linear downsample,
// XYB recompute, 5 blurs, maps; norms/weights/score on CPU f64.
use crate::blur::{blur_planes, create_recursive_gaussian, RgConst};
use crate::context::{GpuBuffer, VkContext};
use crate::maps::{edge_norms, ssim_norms};
use crate::score::{score, ScaleNorms};
use crate::xyb::xyb_convert;

const K_NUM_SCALES: usize = 6;

/// F8 (BUG_HUNT): every buffer created here is owned by the guard and freed
/// on all paths (success or `?`-error); nothing is returned to callers.
struct BufGuard<'a> {
    ctx: &'a VkContext,
    bufs: Vec<GpuBuffer>,
}

impl BufGuard<'_> {
    fn keep(&mut self, b: Result<GpuBuffer, String>) -> Result<GpuBuffer, String> {
        let b = b?;
        self.bufs.push(b);
        Ok(b)
    }
}
impl Drop for BufGuard<'_> {
    fn drop(&mut self) {
        while let Some(b) = self.bufs.pop() {
            self.ctx.destroy_buffer(b);
        }
    }
}

fn downsample(ctx: &VkContext, src: &GpuBuffer, iw: usize, ih: usize) -> Result<GpuBuffer, String> {
    let ow = (iw + 1) / 2;
    let oh = (ih + 1) / 2;
    let dst = ctx.create_empty(3 * ow * oh)?;
    let mut push = Vec::with_capacity(16);
    for u in [iw as u32, ih as u32, ow as u32, oh as u32] {
        push.extend_from_slice(&u.to_le_bytes());
    }
    let r = ctx.run_compute_push(
        include_bytes!("../shaders/downsample_box2.spv"),
        crate::c_main(),
        &[src, &dst],
        (3 * ow * oh).div_ceil(64) as u32,
        &push,
    );
    if let Err(e) = r {
        ctx.destroy_buffer(dst);
        return Err(e);
    }
    Ok(dst)
}

fn mul(ctx: &VkContext, a: &GpuBuffer, b: &GpuBuffer, n: usize) -> Result<GpuBuffer, String> {
    let o = ctx.create_empty(n)?;
    let r = ctx.run_compute_push(
        include_bytes!("../shaders/mul_planes.spv"),
        crate::c_main(),
        &[a, b, &o],
        (n.div_ceil(64)) as u32,
        &(n as u32).to_le_bytes(),
    );
    if let Err(e) = r {
        ctx.destroy_buffer(o);
        return Err(e);
    }
    Ok(o)
}

/// lin1/lin2: plane-major 3*w*h linear RGB (post-alpha-blend, post-sRGB).
pub fn compute_ssimulacra2_gpu(
    ctx: &VkContext,
    lin1: &[f32],
    lin2: &[f32],
    w: usize,
    h: usize,
) -> Result<Vec<ScaleNorms>, String> {
    compute_ssimulacra2_gpu_profiled(ctx, lin1, lin2, w, h, &mut crate::profile::Profile::default())
}

/// H0-instrumented variant: every stage whose wall time we needed to see in the
/// 2026-09-08 Phase H profile runs inside `prof.time`; a disabled Profile makes
/// that a pass-through, so the two entry points compute identically.
pub fn compute_ssimulacra2_gpu_profiled(
    ctx: &VkContext,
    lin1: &[f32],
    lin2: &[f32],
    w: usize,
    h: usize,
    prof: &mut crate::profile::Profile,
) -> Result<Vec<ScaleNorms>, String> {
    assert_eq!(lin1.len(), 3 * w * h);
    assert_eq!(lin2.len(), 3 * w * h);
    let rg: RgConst = create_recursive_gaussian(1.5);
    let mut g = BufGuard { ctx, bufs: vec![] };
    let mut l1 = g.keep(prof.time("upload", || ctx.create_buffer_f32(lin1)))?;
    let mut l2 = g.keep(prof.time("upload", || ctx.create_buffer_f32(lin2)))?;
    let (mut cw, mut ch) = (w, h);
    let mut scales = Vec::new();
    for scale in 0..K_NUM_SCALES {
        if cw < 8 || ch < 8 {
            break;
        }
        // H2.2: one submit + one fence per scale. Every dispatch and upload
        // copy between begin/end records into a single command buffer with
        // explicit compute+transfer barriers between them; any `?` below
        // discards the batch via the guard instead of leaking an open one.
        ctx.begin_batch()?;
        let mut bg = BatchGuard { ctx, armed: true };
        let pass = (|| -> Result<(GpuBuffer, GpuBuffer), String> {
            if scale > 0 {
                let d1 = g.keep(prof.time("downsample", || downsample(ctx, &l1, cw, ch)))?;
                let d2 = g.keep(prof.time("downsample", || downsample(ctx, &l2, cw, ch)))?;
                cw = (cw + 1) / 2;
                ch = (ch + 1) / 2;
                l1 = d1;
                l2 = d2;
            }
            let n = cw * ch;
            let x1 = g.keep(prof.time("xyb", || xyb_convert(ctx, &l1, n, true)))?;
            let x2 = g.keep(prof.time("xyb", || xyb_convert(ctx, &l2, n, true)))?;
            let m11 = g.keep(prof.time("mul", || mul(ctx, &x1, &x1, 3 * n)))?;
            let m22 = g.keep(prof.time("mul", || mul(ctx, &x2, &x2, 3 * n)))?;
            let m12 = g.keep(prof.time("mul", || mul(ctx, &x1, &x2, 3 * n)))?;
            let s11 = g.keep(prof.time("blur", || blur_planes(ctx, &m11, cw, ch, &rg)))?;
            let s22 = g.keep(prof.time("blur", || blur_planes(ctx, &m22, cw, ch, &rg)))?;
            let s12 = g.keep(prof.time("blur", || blur_planes(ctx, &m12, cw, ch, &rg)))?;
            let mu1 = g.keep(prof.time("blur", || blur_planes(ctx, &x1, cw, ch, &rg)))?;
            let mu2 = g.keep(prof.time("blur", || blur_planes(ctx, &x2, cw, ch, &rg)))?;
            let sd = g.keep(ctx.create_empty(3 * n))?;
            let ed = g.keep(ctx.create_empty(3 * n))?;
            prof.time("maps", || {
                ctx.run_compute_push(
                    include_bytes!("../shaders/maps_combine.spv"),
                    crate::c_main(),
                    &[&x1, &x2, &mu1, &mu2, &s11, &s22, &s12, &sd, &ed],
                    ((3 * n) as u32).div_ceil(64),
                    &(n as u32).to_le_bytes(),
                )
            })?;
            Ok((sd, ed))
        })();
        let (sd, ed) = pass?; // on Err, `?` returns and bg.drop() aborts the batch
        // H0 100%-coverage rule: the batched submits+wait land here, not in any
        // per-dispatch stage; time it so RESIDUAL stays below the 5% gate.
        prof.time("submit-wait", || ctx.end_batch())?;
        bg.armed = false;
        let n = cw * ch; // post-downsample dims (cw/ch mutated inside the batch)
        let mut both = prof.time("readback", || ctx.readback_f32_all(&[&sd, &ed]))?;
        let ed_r = both.pop().unwrap_or_default();
        let sd_r = both.pop().unwrap_or_default();
        let sn = prof.time("norms", || {
            let sdv: Vec<&[f32]> = (0..3).map(|c| &sd_r[c * n..(c + 1) * n]).collect();
            let edv: Vec<&[f32]> = (0..3).map(|c| &ed_r[c * n..(c + 1) * n]).collect();
            let mut sn = ScaleNorms::default();
            sn.avg_ssim.copy_from_slice(&ssim_norms(&sdv, cw, ch));
            sn.avg_edgediff.copy_from_slice(&edge_norms(&edv, cw, ch));
            sn
        });
        scales.push(sn);
    }
    // Residual hunt: the guard drop (~50 vkDestroyBuffer/FreeMemory + their
    // bookkeeping) ran after every stage and before the fn returned - real
    // in-wall time attributed to nothing. Measure it explicitly; the drop
    // order is unchanged (same work, same point, just timed).
    prof.time("teardown", || drop(g));
    Ok(scales)
}

/// H2.2: aborts an open batch on scope exit unless disarmed after a
/// successful end_batch. The abort error is swallowed: whatever already
/// triggered the unwinding is the primary error to report.
struct BatchGuard<'a> {
    ctx: &'a VkContext,
    armed: bool,
}

impl Drop for BatchGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.ctx.abort_batch();
        }
    }
}

// ---------------------------------------------------------------------------
// M10-batch: prep/compare split + reference-image caching (round 1: caching
// only, no pipelining; batch CLI, not a daemon).
//
// prep_gpu(side) runs EXACTLY that side's operations from the single-shot
// scale loop (downsample -> xyb -> m=x*x -> s=blur(m) -> mu=blur(x)) and
// returns the resident GPU pyramid; compare_gpu runs only the pair terms
// (m12 = x1*x2, s12 = blur(m12), maps). Every dispatch is a pure function of
// its input buffers with fresh output buffers, so any interleaving of the two
// sides' ops yields bit-identical values to the single-shot path - and the
// batch_vs_single identity test asserts exactly that across the corpus.
// The single-shot entry points above are deliberately left untouched: the
// golden path's 26-CI-run history must not be disturbed by this addition.
// ---------------------------------------------------------------------------

pub struct ScalePrep {
    pub x: GpuBuffer,
    pub s: GpuBuffer,
    pub mu: GpuBuffer,
    pub w: usize,
    pub h: usize,
}

/// One prepped side: resident across a whole batch. All buffers registered in
/// `bufs` the moment they are created, so any `?` inside prep frees exactly
/// what exists (F8 discipline); Drop frees the rest.
pub struct PrepImage<'a> {
    ctx: &'a VkContext,
    bufs: Vec<GpuBuffer>,
    scales: Vec<ScalePrep>,
}

impl Drop for PrepImage<'_> {
    fn drop(&mut self) {
        while let Some(b) = self.bufs.pop() {
            self.ctx.destroy_buffer(b);
        }
    }
}

/// Register a freshly created buffer in the PrepImage's free list. On Err the
/// producing helper already cleaned up its own partial state (they all do),
/// so there is nothing to register - same invariant BufGuard.keep relies on.
fn reg(img: &mut PrepImage, r: Result<GpuBuffer, String>) -> Result<GpuBuffer, String> {
    let b = r?;
    img.bufs.push(b);
    Ok(b)
}

/// lin: plane-major 3*w*h linear RGB (post-alpha-blend, post-sRGB).
pub fn prep_gpu<'a>(
    ctx: &'a VkContext,
    lin: &[f32],
    w: usize,
    h: usize,
    prof: &mut crate::profile::Profile,
) -> Result<PrepImage<'a>, String> {
    assert_eq!(lin.len(), 3 * w * h);
    let rg: RgConst = create_recursive_gaussian(1.5);
    let mut img = PrepImage {
        ctx,
        bufs: Vec::new(),
        scales: Vec::new(),
    };
    let mut l = reg(&mut img, prof.time("upload", || ctx.create_buffer_f32(lin)))?;
    let (mut cw, mut ch) = (w, h);
    for scale in 0..K_NUM_SCALES {
        if cw < 8 || ch < 8 {
            break;
        }
        if scale > 0 {
            let d = reg(&mut img, prof.time("downsample", || downsample(ctx, &l, cw, ch)))?;
            cw = (cw + 1) / 2;
            ch = (ch + 1) / 2;
            l = d;
        }
        let n = cw * ch;
        ctx.begin_batch()?;
        let mut bg = BatchGuard { ctx, armed: true };
        let one = (|| -> Result<ScalePrep, String> {
            let x = reg(&mut img, prof.time("xyb", || xyb_convert(ctx, &l, n, true)))?;
            let m = reg(&mut img, prof.time("mul", || mul(ctx, &x, &x, 3 * n)))?;
            let s = reg(&mut img, prof.time("blur", || blur_planes(ctx, &m, cw, ch, &rg)))?;
            let mu = reg(&mut img, prof.time("blur", || blur_planes(ctx, &x, cw, ch, &rg)))?;
            Ok(ScalePrep { x, s, mu, w: cw, h: ch })
        })();
        let prep = match one {
            Ok(v) => v,
            Err(e) => {
                let _ = ctx.abort_batch();
                return Err(e);
            }
        };
        prof.time("submit-wait", || ctx.end_batch())?;
        bg.armed = false;
        img.scales.push(prep);
    }
    Ok(img)
}

/// Pair terms for two prepped sides -> the same per-scale norms the
/// single-shot pipeline produces. Both sides must have identical scale chains
/// (same dimensions), which prep guarantees for same-size inputs.
pub fn compare_gpu(
    ctx: &VkContext,
    a: &PrepImage,
    b: &PrepImage,
    prof: &mut crate::profile::Profile,
) -> Result<Vec<ScaleNorms>, String> {
    let rg: RgConst = create_recursive_gaussian(1.5);
    let mut g = BufGuard { ctx, bufs: vec![] };
    let mut scales = Vec::new();
    for (sa, sb) in a.scales.iter().zip(b.scales.iter()) {
        let (cw, ch) = (sa.w, sa.h);
        let n = cw * ch;
        ctx.begin_batch()?;
        let mut bg = BatchGuard { ctx, armed: true };
        let pass = (|| -> Result<(GpuBuffer, GpuBuffer), String> {
            let m12 = g.keep(prof.time("mul", || mul(ctx, &sa.x, &sb.x, 3 * n)))?;
            let s12 = g.keep(prof.time("blur", || blur_planes(ctx, &m12, cw, ch, &rg)))?;
            let sd = g.keep(ctx.create_empty(3 * n))?;
            let ed = g.keep(ctx.create_empty(3 * n))?;
            prof.time("maps", || {
                ctx.run_compute_push(
                    include_bytes!("../shaders/maps_combine.spv"),
                    crate::c_main(),
                    &[&sa.x, &sb.x, &sa.mu, &sb.mu, &sa.s, &sb.s, &s12, &sd, &ed],
                    ((3 * n) as u32).div_ceil(64),
                    &(n as u32).to_le_bytes(),
                )
            })?;
            Ok((sd, ed))
        })();
        let (sd, ed) = match pass {
            Ok(v) => v,
            Err(e) => {
                let _ = ctx.abort_batch();
                return Err(e);
            }
        };
        prof.time("submit-wait", || ctx.end_batch())?;
        bg.armed = false;
        let mut both = prof.time("readback", || ctx.readback_f32_all(&[&sd, &ed]))?;
        let ed_r = both.pop().unwrap_or_default();
        let sd_r = both.pop().unwrap_or_default();
        let sn = prof.time("norms", || {
            let sdv: Vec<&[f32]> = (0..3).map(|c| &sd_r[c * n..(c + 1) * n]).collect();
            let edv: Vec<&[f32]> = (0..3).map(|c| &ed_r[c * n..(c + 1) * n]).collect();
            let mut sn = ScaleNorms::default();
            sn.avg_ssim.copy_from_slice(&ssim_norms(&sdv, cw, ch));
            sn.avg_edgediff.copy_from_slice(&edge_norms(&edv, cw, ch));
            sn
        });
        scales.push(sn);
    }
    Ok(scales)
}

/// M10-batch round 1: score N variants against one shared original, caching
/// the original's prep (GPU pyramid) across the whole batch. Each variant
/// pays only its own prep + the pair's compare. Values are identical to the
/// single-shot path by construction (see prep_gpu/compare_gpu notes); the
/// batch_vs_single identity test asserts it. `prof` aggregates across the
/// batch, so per-image cost = totals / N.
///
/// Fused variant (this fn): one begin/end_batch per scale doing variant-side
/// prep + cross terms against the resident original pyramid - same fence
/// count per image as the single-shot, ~half the compute (original side
/// skipped). The split prep_gpu+compare_gpu pair is kept as an independent
/// reference implementation the tests cross-check.
pub fn compare_prep_gpu(
    ctx: &VkContext,
    orig: &PrepImage,
    vlin: &[f32],
    w: usize,
    h: usize,
    prof: &mut crate::profile::Profile,
) -> Result<Vec<ScaleNorms>, String> {
    assert_eq!(vlin.len(), 3 * w * h);
    let rg: RgConst = create_recursive_gaussian(1.5);
    let mut img = PrepImage {
        ctx,
        bufs: Vec::new(),
        scales: Vec::new(),
    };
    let mut l = reg(&mut img, prof.time("upload", || ctx.create_buffer_f32(vlin)))?;
    let (mut cw, mut ch) = (w, h);
    let mut out = Vec::with_capacity(orig.scales.len());
    for (si, os) in orig.scales.iter().enumerate() {
        ctx.begin_batch()?;
        let mut bg = BatchGuard { ctx, armed: true };
        let pass = (|| -> Result<(GpuBuffer, GpuBuffer), String> {
            if si > 0 {
                let d = reg(&mut img, prof.time("downsample", || downsample(ctx, &l, cw, ch)))?;
                cw = (cw + 1) / 2;
                ch = (ch + 1) / 2;
                l = d;
            }
            assert_eq!((os.w, os.h), (cw, ch), "orig/variant scale chains diverged");
            let n = cw * ch;
            let vx = reg(&mut img, prof.time("xyb", || xyb_convert(ctx, &l, n, true)))?;
            let vm = reg(&mut img, prof.time("mul", || mul(ctx, &vx, &vx, 3 * n)))?;
            let vs = reg(&mut img, prof.time("blur", || blur_planes(ctx, &vm, cw, ch, &rg)))?;
            let vmu = reg(&mut img, prof.time("blur", || blur_planes(ctx, &vx, cw, ch, &rg)))?;
            // cross terms, same operand order as the single-shot's mul(x1,x2)
            let m12 = reg(&mut img, prof.time("mul", || mul(ctx, &os.x, &vx, 3 * n)))?;
            let s12 = reg(&mut img, prof.time("blur", || blur_planes(ctx, &m12, cw, ch, &rg)))?;
            let sd = reg(&mut img, ctx.create_empty(3 * n))?;
            let ed = reg(&mut img, ctx.create_empty(3 * n))?;
            prof.time("maps", || {
                ctx.run_compute_push(
                    include_bytes!("../shaders/maps_combine.spv"),
                    crate::c_main(),
                    &[&os.x, &vx, &os.mu, &vmu, &os.s, &vs, &s12, &sd, &ed],
                    ((3 * n) as u32).div_ceil(64),
                    &(n as u32).to_le_bytes(),
                )
            })?;
            Ok((sd, ed))
        })();
        let (sd, ed) = match pass {
            Ok(v) => v,
            Err(e) => {
                let _ = ctx.abort_batch();
                return Err(e);
            }
        };
        prof.time("submit-wait", || ctx.end_batch())?;
        bg.armed = false;
        let n = cw * ch;
        let mut both = prof.time("readback", || ctx.readback_f32_all(&[&sd, &ed]))?;
        let ed_r = both.pop().unwrap_or_default();
        let sd_r = both.pop().unwrap_or_default();
        let sn = prof.time("norms", || {
            let sdv: Vec<&[f32]> = (0..3).map(|c| &sd_r[c * n..(c + 1) * n]).collect();
            let edv: Vec<&[f32]> = (0..3).map(|c| &ed_r[c * n..(c + 1) * n]).collect();
            let mut sn = ScaleNorms::default();
            sn.avg_ssim.copy_from_slice(&ssim_norms(&sdv, cw, ch));
            sn.avg_edgediff.copy_from_slice(&edge_norms(&edv, cw, ch));
            sn
        });
        out.push(sn);
    }
    Ok(out)
}

pub fn score_batch_gpu(
    ctx: &VkContext,
    orig_lin: &[f32],
    w: usize,
    h: usize,
    variants: &[&[f32]],
    prof: &mut crate::profile::Profile,
) -> Result<Vec<Vec<ScaleNorms>>, String> {
    let orig = prep_gpu(ctx, orig_lin, w, h, prof)?;
    let mut out = Vec::with_capacity(variants.len());
    for v in variants {
        out.push(compare_prep_gpu(ctx, &orig, v, w, h, prof)?);
    }
    Ok(out)
}

/// Linear-plane builder mirroring the CLI prep exactly (8-bit alpha-free ->
/// LUT; alpha-bearing -> blend + per-element), so batch and single-shot paths
/// share one source of truth for the front end.
fn linearize_front_end(
    dec: &crate::cpu::Decoded,
    bg: f32,
    prof: &mut crate::profile::Profile,
) -> Vec<f32> {
    let n = dec.w * dec.h;
    prof.time("prep", || {
        let lut = crate::cpu::linearize_lut();
        match &dec.alpha {
            Some(al) => crate::cpu::to_linear(&crate::cpu::alpha_blend(&dec.srgb, al, bg, n)),
            None => crate::cpu::to_linear_8bit(&dec.srgb, &lut),
        }
    })
}

/// Path-driven batch used by the `score-many` CLI: decodes the original once,
/// preps it once (cached), then streams each variant (decode -> front-end ->
/// prep -> compare) so peak host/GPU memory is one variant + the resident
/// original pyramid even at 4K. Returns (variant path, score) in input order.
///
/// The confirmed production pattern is an alpha-free 4K original vs alpha-free
/// compressed variants; an alpha-bearing original would need the worst-of-bg
/// dual pass (two different preps) and is rejected here rather than silently
/// changing what "the cached original" means.
pub fn score_batch_paths(
    ctx: &VkContext,
    orig_path: &str,
    variant_paths: &[String],
    prof: &mut crate::profile::Profile,
) -> Result<Vec<(String, f64)>, String> {
    let orig = prof.time("decode", || crate::cpu::decode_png(orig_path))?;
    if orig.w < 8 || orig.h < 8 {
        return Err("original below 8x8".into());
    }
    if orig.alpha.is_some() {
        return Err(
            "score-many requires an alpha-free original (the confirmed production \
             pattern); alpha originals need the worst-of-bg dual pass and are not cached"
                .into(),
        );
    }
    let w = orig.w;
    let h = orig.h;
    let orig_lin = linearize_front_end(&orig, 0.5, prof);
    let orig_prep = prep_gpu(ctx, &orig_lin, w, h, prof)?;
    let mut out = Vec::with_capacity(variant_paths.len());
    for vp in variant_paths {
        let vd = prof.time("decode", || crate::cpu::decode_png(vp))?;
        if vd.w != w || vd.h != h {
            return Err(format!("variant {vp} size {}x{} != original {w}x{h}", vd.w, vd.h));
        }
        if vd.alpha.is_some() {
            return Err(format!("variant {vp} has alpha; the batch path is alpha-free only"));
        }
        let vlin = linearize_front_end(&vd, 0.5, prof);
        let scales = compare_prep_gpu(ctx, &orig_prep, &vlin, w, h, prof)?;
        out.push((vp.clone(), score(&scales)));
        // vlin drops here; orig_prep stays cached.
    }
    Ok(out)
}

/// No-cache baseline in the same process: full single-shot per variant (the
/// original's side is recomputed every time). This is the comparator M10-batch
/// measures the cached batch against, holding the session warm-state fixed.
pub fn score_nocache_paths(
    ctx: &VkContext,
    orig_path: &str,
    variant_paths: &[String],
    prof: &mut crate::profile::Profile,
) -> Result<Vec<(String, f64)>, String> {
    let orig = prof.time("decode", || crate::cpu::decode_png(orig_path))?;
    let w = orig.w;
    let h = orig.h;
    let orig_lin = linearize_front_end(&orig, 0.5, prof);
    let mut out = Vec::with_capacity(variant_paths.len());
    for vp in variant_paths {
        let vd = prof.time("decode", || crate::cpu::decode_png(vp))?;
        if vd.w != w || vd.h != h {
            return Err(format!("variant {vp} size {}x{} != original {w}x{h}", vd.w, vd.h));
        }
        let vlin = linearize_front_end(&vd, 0.5, prof);
        let scales = compute_ssimulacra2_gpu_profiled(ctx, &orig_lin, &vlin, w, h, prof)?;
        out.push((vp.clone(), score(&scales)));
    }
    Ok(out)
}

/// Shared `score-many` variant discovery: *.png in a dir, sorted for
/// deterministic order (the batch benchmark depends on a stable sequence).
pub fn list_variants(dir: &str) -> Result<Vec<String>, String> {
    let mut v: Vec<String> = Vec::new();
    for e in std::fs::read_dir(dir).map_err(|e| format!("read dir {dir}: {e}"))? {
        let e = e.map_err(|e| format!("dir entry: {e}"))?;
        let p = e.path();
        if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("png") {
            v.push(p.to_string_lossy().into_owned());
        }
    }
    v.sort();
    if v.is_empty() {
        return Err(format!("no .png variants found in {dir}"));
    }
    Ok(v)
}
