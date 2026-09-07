# SSIMULACRA2 → Vulkan GPU Port: Research Notes

*Compiled September 2026. Goal: identify what already exists — reference
algorithm, existing GPU ports, and Rust-to-Vulkan tooling — so a Vulkan port
of SSIMULACRA2 can reuse prior art instead of being built from zero.*

## Executive summary

No Vulkan-native implementation of SSIMULACRA2 exists yet (checked GitHub,
Codeberg, crates.io, lib.rs, and vapoursynth-plugin listings as of this
writing) — this part is genuinely new ground. But the two hard sub-problems
are each already solved separately, by different projects:

1. **Getting the numbers right** is solved: a canonical, ~550-line C++
   reference implementation exists, plus two working GPU ports (HIP/CUDA)
   whose algorithmic structure transfers directly, plus several CPU
   reimplementations whose bug-fix histories document exactly which parts of
   the algorithm are easy to get subtly wrong.
2. **Writing Vulkan compute kernels in Rust without hand-rolled GLSL/SPIR-V**
   is solved by existing tooling (`rust-gpu`, `krnl`, `cubecl`) — the most
   actively maintained of which (`cubecl`) also targets Vulkan's native
   SPIR-V path directly, not just through a generic WebGPU abstraction.

Recommendation: use Vship (GPU) and the `fast-ssim2` paper (optimization
technique) as the algorithmic reference, use the libjxl reference tool and
the `iqa` crate's validation approach as the correctness oracle, and build
the kernels in `cubecl` (or `krnl`/`rust-gpu` as a fallback) rather than
hand-written SPIR-V — reusing whatever host-side Vulkan plumbing from
dssim-vulkan isn't hard-coded to DSSIM's specific buffer shapes.

---

## 1. What SSIMULACRA2 actually computes

SSIMULACRA2 ("Structural SIMilarity Unveiling Local And Compression Related
Artifacts", v2) was designed by Jon Sneyers at Cloudinary, released July
2022, and **updated in April 2023** with rescaled XYB values and retuned
weights. That April-2023 revision is what "SSIMULACRA2" means today — older
libjxl builds (≤ v0.8.2) still emit the July-2022 (v2.0) numbers, which look
plausible but aren't comparable. Confirm which version any reference you use
actually implements before trusting it as a validation oracle.

The canonical source is `libjxl/tools/ssimulacra2.{h,cc}` (BSD-licensed,
part of the JPEG XL reference codebase); `cloudinary/ssimulacra2` is a
thin wrapper repo with a build script that fetches only the pieces of
libjxl needed to compile the standalone tool. Treat any other
implementation — including this report's summary — as a re-derivation to be
checked against that source directly.

Staged the way a compute-shader port would naturally split it, the pipeline
is:

1. If either image has an alpha channel, alpha-blend it against a neutral
   gray background.
2. Convert both images to linear RGB.
3. Convert linear RGB to XYB (JPEG XL's opsin-inspired color space).
4. Rescale XYB into a roughly 0..1 "positive XYB": the B channel becomes
   (B − Y) + 0.55, X becomes X·14 + 0.42, and Y becomes Y + 0.01. This exact
   set of constants has been a real source of bugs in reimplementations
   (§3) — copy them from source rather than re-deriving them.
5. Repeat for 6 scales (1:1 down to 1:32): from scale 1 onward, first
   box-downsample both images 2× in linear RGB, then repeat steps 3–4.
6. At each scale, form five working images by elementwise
   squaring/multiplying: ref², dist², ref·dist, ref, and dist.
7. Blur all five with a Gaussian, σ = 1.5. The reference implementation uses
   a **recursive (IIR) Gaussian** for CPU speed — this is sequential by
   construction and does not parallelize well, making it the single most
   important thing to change for a GPU port (§4).
8. From the five blurred images, compute two per-pixel error maps per
   channel: a corrected 1−SSIM map (the reference drops the usual SSIM
   denominator to avoid double gamma-correcting), and an edge map split
   into "ringing/blockiness" (distorted has an edge the original lacks) and
   "blur/smoothing" (original has an edge the distorted lacks).
9. Reduce each of the resulting 3 error maps × 3 channels × 6 scales = 54
   maps with both a 1-norm (mean) and a 4-norm (mean of the 4th power, then
   fourth-rooted) → 108 numbers.
10. Take a fixed, pre-tuned weighted sum of those 108 numbers, then apply a
    fixed cubic/power rescaling to get the final ~0–100 score.

Every step except the last (a 108-term dot product plus a scalar
polynomial — trivial on CPU, not worth a GPU kernel) is either purely
elementwise or a small-radius convolution/reduction. The whole pipeline is a
good fit for compute shaders, and steps 2–9 are structurally the same
operations dssim-vulkan already implements for a single-scale, single-blur
metric — just repeated across 6 scales, 3 channels, and 5 blur targets
instead of 1.

## 2. Prior art — what not to rebuild

### GPU implementations

- **Vship** (Line-fr, MIT license, HIP + CUDA). The only mature GPU port of
  SSIMULACRA2 found. Ships SSIMULACRA2 alongside Butteraugli and CVVDP, via
  a vapoursynth plugin, a standalone `FFVship` CLI, and a C API. It credits
  dnjulek's Zig CPU implementation (below) as its correctness reference.
  **The GitHub repo was archived in February 2026**; the project moved to
  Codeberg (`codeberg.org/Line-fr/Vship`), which is the current home —
  GitHub still hosts the old tree read-only. Benchmarked on a Ryzen 7940HS +
  RTX 4050 Mobile, 1080p, 1339 frames: libjxl CPU reference 115 s, VSZIP
  (fast CPU) 68 s, Vship-as-vapoursynth-plugin 25 s, FFVship (native) 9 s —
  roughly 13× the CPU reference. VRAM use is documented as
  `9 * 4 * width * height * 4/3` bytes per concurrent stream. Its HIP
  backend already runs on RDNA2-class AMD hardware in the same family as an
  RX 6600M, so it's usable as a numeric oracle on the same GPU, even though
  it isn't Vulkan.
- **turbo-metrics / `ssimulacra2-cuda(-kernel)`** — a solo-developer Rust
  project with a from-scratch CUDA implementation, notable for compiling
  actual Rust to CUDA PTX (the `nvptx64-nvidia-cuda` target) instead of
  hand-written CUDA C++. The author's own project notes call the current
  kernel unoptimized ("I'm terrible at writing GPU code that runs fast") and
  explicitly list a Vulkan/`krnl`/`cubecl` port as an unclaimed TODO — so
  this is useful for the plumbing (colorspace-conversion kernels,
  plane-separated buffers) rather than for performance technique. The same
  project's README surveys hardware video/compute API support across
  vendors and concludes Vulkan (plus Vulkan Video for decode) is "the way
  forward" for cross-vendor GPU media processing — independent validation
  of Vulkan as the right target rather than a single vendor's API.

### CPU references (use as correctness oracles, not to port from directly)

- `ssimulacra2` (a `docs.rs`-hosted Rust crate) — clean struct-per-stage
  layout (`Blur`, `Xyb`, `LinearRgb`, `Frame`, `Plane`) that maps cleanly
  onto how you'd split compute-shader passes.
- `fast-ssim2` — a paper-backed implementation (~44× faster than libjxl on
  CPU) that gets its speedup from three changes: replacing the recursive
  blur with a separable FIR filter, splitting reference/distortion
  computation so the (often-reused) reference can be precomputed, and
  skipping near-zero-weight terms. The separable-filter change is the single
  most transferable insight for a GPU port (§4).
- `vszip` / dnjulek's Zig implementation — the fastest CPU implementation in
  the benchmarks above, now folded into `vapoursynth-zip` (the standalone
  repo is archived). Its release history documents two real correctness
  bugs worth avoiding: an outdated `make_positive_xyb` formula in an early
  release, and a recursive-blur artifact on video that forced a later
  revert to "true Gauss."
- `iqa` — a Rust wrapper crate whose test suite vendors the exact
  Cloudinary/libjxl source as a git submodule and cross-validates its
  output pixel-for-pixel against it, while also documenting the v2.0-vs-2.1
  version drift mentioned in §1. A directly reusable template for a
  validation harness.
- `hlindset/ssimulacra2` (an Elixir NIF wrapping `fast-ssim2`) explicitly
  notes its scores have not been bit-exactly validated against the
  canonical reference — a reminder that "compiles and runs" is not the same
  as "numerically correct," for GPU or CPU ports alike.

## 3. Known correctness pitfalls (from three independent reimplementations)

- **Version drift** — only the April-2023 (v2.1) weights/XYB scaling match
  today's SSIMULACRA2; older libjxl builds emit v2.0 numbers that look
  plausible but aren't comparable. Pin to a specific, checked libjxl
  commit as ground truth, not to a remembered formula.
- **The positive-XYB rescale** — vszip shipped an outdated version of this
  formula and had to bugfix it later. Copy the constants (14, 0.42, 0.55,
  0.01, and the B−Y subtraction) from source.
- **Recursive vs. true Gaussian blur** — the CPU reference's recursive
  Gaussian caused a real, video-visible bug in vszip, forcing a revert to a
  direct ("true Gauss") implementation. For a GPU port this is moot either
  way: recursive filters are inherently sequential, so budget for a small
  fixed-radius separable Gaussian (radius ≈ 4–5 px covers σ = 1.5) from the
  start, and treat "matches the reference within tolerance" — not "uses the
  same filter" — as the correctness bar.
- **Not bit-exact by default** — at least one independent wrapper
  explicitly flags its own output as unvalidated against the canonical
  reference. Build the validation harness (§2, the `iqa` approach) *before*
  trusting any new implementation's numbers.

## 4. Rust → Vulkan tooling, so raw SPIR-V/GLSL isn't necessary

| Option | Maintenance status (Sept 2026) | Notes |
|---|---|---|
| **rust-gpu** | Active, but moved. The original EmbarkStudios repo was archived Oct 31, 2025; development continues under the community-owned `Rust-GPU` GitHub org (its `cargo-gpu` companion tool was merged into the main repo in April 2026). | Compiles ordinary Rust to SPIR-V for Vulkan shaders. Point any new dependency at the new org, not the archived one. |
| **krnl** | Appears stale — latest release (v0.1.1) is pinned to the old EmbarkStudios `spirv-builder` location and doesn't show updates since rust-gpu's move. | A thin, Vulkan-1.2-only convenience layer on top of rust-gpu: kernels are written inline as plain Rust and compiled ahead-of-time into a cache file so the crate still builds on stable Rust. Closest existing match to "write the blur/XYB/SSIM kernels as Rust functions, get Vulkan compute shaders out" — worth using as a design reference or a fork-and-repin starting point even if not adopted as-is. |
| **cubecl** (tracel-ai, the Burn ML framework's compute layer) | Most actively maintained — weekly releases through August 2026, 1.1M+ crates.io downloads. | A single `#[cube]`-annotated Rust function compiles to CUDA, ROCm/HIP, Metal, CPU, or — listed as its own row, distinct from generic WebGPU — native **Vulkan via a SPIR-V compiler path**. Because it also has a HIP backend, the same kernel source could produce both the Vulkan port and a HIP build that runs on the RX 6600M's existing ROCm stack for cross-checking, without maintaining two kernel implementations. |

**Recommendation:** prototype the SSIMULACRA2 kernels in `cubecl` first,
given its maintenance activity and the built-in cross-backend check; keep
`krnl`/raw `rust-gpu` as a fallback if `cubecl`'s Vulkan/SPIR-V backend is
missing something a kernel needs (e.g. a specific subgroup operation).

## 5. What this means for reusing dssim-vulkan

dssim-vulkan already solves, for a single-scale/single-blur metric, the
parts of this pipeline that are hardest to get right the first time:
Vulkan device/queue/pipeline setup, buffer upload/readback, and a working
Gaussian-blur compute shader. SSIMULACRA2 is structurally the same shape
repeated — 6 scales instead of 1, three color channels instead of one, five
blurred images per scale instead of one or two. If dssim-vulkan's host-side
plumbing isn't hard-coded to DSSIM's specific buffer shapes, most of it —
instance/device setup, upload/readback, the blur dispatch pattern — should
carry over, with mainly the kernel bodies and the per-scale/per-channel
bookkeeping changing.

The Phase H finding on dssim-vulkan (host-side data prep dominates at
~70–90 ms at 2048×2048, with GPU compute only ~5% of wall time) very likely
recurs here, probably more severely: SSIMULACRA2 moves 3 channels × 5 blur
targets × 6 scales of data per image pair, versus DSSIM's single blur. Worth
profiling the sRGB→linear→XYB conversion and upload path early, rather than
assuming the blur/SSIM kernels themselves are the bottleneck.

**Caveat:** dssim-vulkan's and compare-image's actual source weren't
reachable for this research (no public repo found under a search of GitHub
for either project name) — the reuse claim above is an inference from the
project's documented scope, not something checked against the code
directly. Worth a quick look at how tightly the existing Vulkan setup is
coupled to DSSIM's specific buffer layout before assuming full reuse.

## 6. Suggested reference order, if starting the port

1. Read `libjxl/tools/ssimulacra2.{h,cc}` end to end once — it's the ground
   truth for every number the port needs to reproduce.
2. Build a CPU-side validation harness modeled on the `iqa` crate's
   approach: pin an exact libjxl commit, score a small fixed set of test
   images with it once, and check every future implementation against those
   saved numbers rather than "does this look right."
3. Build the kernels in `cubecl` (or `krnl`/`rust-gpu` if `cubecl`'s Vulkan
   path is missing something), reusing dssim-vulkan's host-side setup
   wherever it isn't DSSIM-specific.
4. Cross-check against Vship's `FFVship` binary on the same images and GPU
   family — it's already running HIP on RDNA2-class AMD hardware and is the
   closest thing to a second, independent GPU oracle.
5. Optimize only after correctness is nailed down: the `fast-ssim2` paper's
   techniques (separable blur, precomputed reference, weight-skipping)
   apply to the GPU port too, but as a second pass, once a slow-but-correct
   Vulkan version exists.

## Sources

- [libjxl/tools/ssimulacra2.cc](https://github.com/libjxl/libjxl/blob/main/tools/ssimulacra2.cc) — canonical reference implementation
- [cloudinary/ssimulacra2](https://github.com/cloudinary/ssimulacra2) — build wrapper and spec summary
- [Fast Computation of SSIMULACRA2 on GPUs: A Performance Evaluation — Codec Wiki](https://wiki.x266.mov/blog/turbo-metrics-performance) — algorithm summary and CPU/GPU benchmark comparison
- [Line-fr/Vship (GitHub, archived)](https://github.com/Line-fr/Vship) — GPU (HIP/CUDA) implementation, performance table
- [Vship SSIMULACRA2 doc](https://github.com/Line-fr/Vship/blob/main/doc/SSIMULACRA2.md) — VRAM formula and API
- [turbo-metrics (lib.rs)](https://lib.rs/crates/turbo-metrics) — project scope, TODOs, platform-support survey
- [ssimulacra2-cuda-kernel (lib.rs)](https://lib.rs/crates/ssimulacra2-cuda-kernel) — Rust-to-PTX CUDA kernel build process
- [ssimulacra2 crate (docs.rs)](https://docs.rs/ssimulacra2) — CPU reference crate structure
- [fast-ssim2 crate (docs.rs)](https://docs.rs/fast-ssim2) — optimized CPU implementation
- ["Fast implementation of SSIMULACRA2 for image quality assessment" (ResearchGate)](https://www.researchgate.net/publication/401347718_Fast_implementation_of_SSIMULACRA2_for_image_quality_assessment) — separable-filter/precompute/weight-skipping optimization paper
- [dnjulek/vapoursynth-ssimulacra2 releases](https://github.com/dnjulek/vapoursynth-ssimulacra2/releases) — bugfix history (positive-XYB formula, recursive-vs-true Gauss)
- [iqa crate test source (docs.rs)](https://docs.rs/crate/iqa/latest/source/tests/ssimulacra2_reference.rs) — validation-harness approach, v2.0 vs v2.1 note
- [hlindset/ssimulacra2 releases](https://github.com/hlindset/ssimulacra2/releases) — non-bit-exact validation caveat
- [charles-r-earp/krnl](https://github.com/charles-r-earp/krnl) — Vulkan-only Rust kernel crate
- [tracel-ai/cubecl](https://github.com/tracel-ai/cubecl) — multi-backend (incl. native Vulkan/SPIR-V) Rust compute crate
- [EmbarkStudios/rust-gpu (archived notice)](https://github.com/embarkstudios/rust-gpu) — move to community ownership
- [Rust-GPU/rust-gpu PR #553](https://github.com/Rust-GPU/rust-gpu/pull/553) — cargo-gpu merge, evidence of active 2026 development
