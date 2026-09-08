// End-to-end GPU pipeline: mirrors ComputeSSIMULACRA2's scale loop
// (ssimulacra2.cc:449-485) exactly - break rule, per-scale linear downsample,
// XYB recompute, 5 blurs, maps; norms/weights/score on CPU f64.
use crate::blur::{blur_planes, create_recursive_gaussian, RgConst};
use crate::context::{GpuBuffer, VkContext};
use crate::maps::{edge_norms, ssim_norms};
use crate::score::ScaleNorms;
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
    assert_eq!(lin1.len(), 3 * w * h);
    assert_eq!(lin2.len(), 3 * w * h);
    let rg: RgConst = create_recursive_gaussian(1.5);
    let mut g = BufGuard { ctx, bufs: vec![] };
    let mut l1 = g.keep(ctx.create_buffer_f32(lin1))?;
    let mut l2 = g.keep(ctx.create_buffer_f32(lin2))?;
    let (mut cw, mut ch) = (w, h);
    let mut scales = Vec::new();
    for scale in 0..K_NUM_SCALES {
        if cw < 8 || ch < 8 {
            break;
        }
        if scale > 0 {
            let d1 = g.keep(downsample(ctx, &l1, cw, ch))?;
            let d2 = g.keep(downsample(ctx, &l2, cw, ch))?;
            cw = (cw + 1) / 2;
            ch = (ch + 1) / 2;
            l1 = d1;
            l2 = d2;
        }
        let n = cw * ch;
        let x1 = g.keep(xyb_convert(ctx, &l1, n, true))?;
        let x2 = g.keep(xyb_convert(ctx, &l2, n, true))?;
        let m11 = g.keep(mul(ctx, &x1, &x1, 3 * n))?;
        let m22 = g.keep(mul(ctx, &x2, &x2, 3 * n))?;
        let m12 = g.keep(mul(ctx, &x1, &x2, 3 * n))?;
        let s11 = g.keep(blur_planes(ctx, &m11, cw, ch, &rg))?;
        let s22 = g.keep(blur_planes(ctx, &m22, cw, ch, &rg))?;
        let s12 = g.keep(blur_planes(ctx, &m12, cw, ch, &rg))?;
        let mu1 = g.keep(blur_planes(ctx, &x1, cw, ch, &rg))?;
        let mu2 = g.keep(blur_planes(ctx, &x2, cw, ch, &rg))?;
        let sd = g.keep(ctx.create_empty(3 * n))?;
        let ed = g.keep(ctx.create_empty(3 * n))?;
        ctx.run_compute_push(
            include_bytes!("../shaders/maps_combine.spv"),
            crate::c_main(),
            &[&x1, &x2, &mu1, &mu2, &s11, &s22, &s12, &sd, &ed],
            ((3 * n) as u32).div_ceil(64),
            &(n as u32).to_le_bytes(),
        )?;
        let sd_r = ctx.readback_f32(&sd)?;
        let ed_r = ctx.readback_f32(&ed)?;
        let sdv: Vec<&[f32]> = (0..3).map(|c| &sd_r[c * n..(c + 1) * n]).collect();
        let edv: Vec<&[f32]> = (0..3).map(|c| &ed_r[c * n..(c + 1) * n]).collect();
        let mut sn = ScaleNorms::default();
        sn.avg_ssim.copy_from_slice(&ssim_norms(&sdv, cw, ch));
        sn.avg_edgediff.copy_from_slice(&edge_norms(&edv, cw, ch));
        scales.push(sn);
    }
    Ok(scales)
}
