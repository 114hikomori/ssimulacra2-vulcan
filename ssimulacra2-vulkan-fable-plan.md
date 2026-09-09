# SSIMULACRA2 Vulkan Port — Fable Method Plan

**Status:** EXECUTED AND CLOSED through M10-batch (2026-09-09): M0-M9 done
(Phases A-G), Phase H adjudicated (option (a) for the single-pair CLI),
M10-batch shipped and closed (caching + 0.5 MP routing). Per-milestone exit
observations live in `CHECKPOINT.md`; CI green through run #28. This document
is the approved plan of record - read it with the Phase H annotations below.

**Scope:** Port the repository's SSIMULACRA2 v2.1 CPU implementation (`src/ssimulacra2.cc` +
vendored libjxl pieces) to a Vulkan compute backend, with the C++ implementation kept as the
correctness oracle and the CPU path unchanged by default.

**Primary authority:** the actual `src/ssimulacra2.{h,cc}` and `src/lib/jxl` source in this
repository (see `AGENTS.md` §2). Research material is secondary.

**Ground-truth companion:** `SSIMULACRA2_VULKAN_PORT_PLAN.md` (full source-line-cited
derivation trail) is the *next* Phase 0 deliverable, to be written after this plan is approved.
§2 below already inlines every load-bearing constant with file:line citations so this document
is self-sufficient for sequencing decisions — but consult the technical plan once it exists,
and never re-derive constants from memory.

**Environment facts carried from the sibling port** (`D:\.Github_Project\.repo\dssim-vulcan`,
`CHECKPOINT.md` M1 entry + Pass-9/10 lessons — observed on *this same host*):
- Real GPUs available locally: AMD Radeon RX 6600M (discrete) + AMD Radeon iGPU; both passed
  smoke tests. lavapipe is **not** available on this Windows host — the software-Vulkan leg
  runs in Linux CI only.
- The AMD Windows driver returns `VK_INCOMPLETE` when two threads create Vulkan instances
  concurrently → serialize context creation in-process.
- Shader toolchain: `glslc` from the pinned Vulkan SDK (1.4.357.0 there); compiled `.spv`
  blobs checked in; SPIR-V is **not byte-stable across glslc versions**, so no CI byte-compare
  gate without pinning the full SDK (that gate was tried and reverted — dssim lesson BH38).
- `shaderc` as a build.rs dependency was rejected there as too heavy; offline glslc +
  checked-in `.spv` is the proven pattern.

---

## 1. Outcome and Definition of Done

The finished work is a Vulkan compute path that produces the same SSIMULACRA2 scores as the
C++ reference in this repo, selectable via a CLI flag, with the CPU path as default and
fallback.

It is done when all of the following are observed:

- The C++ `ssimulacra2` binary builds and its behavior is unchanged by the port (the dump
  instrumentation is `#ifdef`-gated and off by default; default binary byte-identical).
- A Rust `ssimulacra2-vulkan` crate computes scores within **≤1e-5 absolute** (0..100 scale)
  of the C++ oracle on every fixture, with the primary numeric bar on the 108 pooled norms
  (≤1e-6 absolute) and the weighted sum (≤1e-6) — the final score is then CPU-f64-derived and
  matches by construction.
- An image compared with itself returns **exactly** `100.0` (asserted `==`), guaranteed by
  symmetric computation (§8), not by tolerance.
- Intermediate GPU results can be dumped and compared against CPU oracle dumps per stage, so
  a mismatch localizes to a stage, not just to the final score.
- Vulkan correctness tests run on lavapipe in Linux CI; real-GPU validation on the RX 6600M
  runs locally.
- The port does not require a new image-decoding stack for the oracle; the Rust CLI's own
  decode scope is defined in §4.
- Performance is measured separately and no speedup is claimed until end-to-end timings are
  observed on representative sizes.

**Explicit non-goals for the first working version:** GPU image decoding, ICC color-profile
handling (Rust CLI rejects ICC-profiled input with a pointer to the C++ binary; returns later
only as profiling-justified opt-in), float16 math, multi-GPU, GPU-side norm pooling / weighted
sum (stays CPU f64), JPEG decode parity in Rust (JPEG fixtures enter via oracle pixel dumps,
§4), bit-exactness with the C++ path (parity is within the §7 ladder, not bitwise — except
where noted).

### Load-bearing assumptions

- The vendored `src/lib` subset compiles unmodified on Linux (README's Debian deps) — the
  oracle build happens in WSL2/CI, not on the Windows host. **Checkable in Phase A step 1;
  if it fails, that is a surprise that re-opens planning.**
- SSIMULACRA2's per-pixel arithmetic is f32 almost everywhere (only the norm accumulators and
  `EdgeDiffMap`'s ratio are double), so an f32 GPU pipeline plus CPU f64 pooling reproduces
  the oracle within the ladder.
- The recursive Gaussian's recurrence is causal and stable, so a straightforward per-row /
  per-column thread scan reproduces it to f32 reassociation drift (§2, Phase D).

---

## 2. Evidence Base and Important Corrections

The research notes (`ssimulacra2-vulkan-port-research.md`) were written before the source was
read line-by-line. Everything below was confirmed against this repo's source this session.

### Corrections to the research notes (binding)

| # | Research notes say | Source actually does | Cite |
|---|---|---|---|
| 1 | "box-downsample both images 2× in linear RGB" | Box average with **edge clamp-replication** (`std::min`), output size `ceil(in/f)`, and normalization by the **full** `fx·fy` even where edges were replicated — border pixels are double-counted by design. f32 accumulation, order: channel → out-y → out-x → box-y → box-x. | `ssimulacra2.cc:44-66` |
| 2 | "budget for a small fixed-radius separable Gaussian (FIR) instead of the recursive one" | The reference blur is the Charalampidis truncated-cosine **recursive IIR**: radius `roundf(3.2795σ+0.2546)` = **5** at σ=1.5; three 2nd-order sections k∈{1,3,5}; folded input `in[n−N−1] + in[n+N−1]`; **zero padding** (not mirror); forward-only scan from `n=−N+1` with zero ring state; horizontal pass first, then vertical. FIR is *not* parity-equivalent (AGENTS.md §8 forbids assuming it); the port targets the recurrence itself (§2 blur constants, Phase D). | `gauss_blur.cc:499-590, 615-620`; `ssimulacra2.cc:82-108` |
| 3 | "XYB (opsin-inspired), rescale constants 14, 0.42, 0.55, 0.01" | Confirmed, plus the parts the notes omit: absorb matrix is **pre-multiplied by IntensityTarget/255** (default 255 → scale 1.0), bias added *before* the clamp, cube root via a **6-ulp Newton-iteration polynomial** (`CubeRootAndAdd`, seed + 3 NR + final iteration) minus `cbrt(bias)` — not `pow(x,1/3)`; output mixing `X=½(L−M)`, `Y=½(L+M)`, `B=S`. | `enc_xyb.cc:215-223, 61-95, 70-73`; `opsin_params.h:20-66`; `fast_math-inl.h:173-209` |
| 4 | "positive XYB: B becomes (B−Y)+0.55" | Confirmed — and **order matters**: `B` is computed from the *pre-shift* `Y`, then `X`, then `Y += 0.01`. All f32. | `ssimulacra2.cc:209-220` |
| 5 | "6 scales (1:1 to 1:32)" | Loop **breaks when `xsize < 8 || ysize < 8`** → image-dependent scale count (1..6). The 108-weight dot product iterates `for c { for scale { for n } }` over `scales.size()`, so **missing scales shift weight indices** — a 3-scale image uses weights 0..53 in channel-major order. Must reproduce exactly. | `ssimulacra2.cc:449-452, 375-406` |
| 6 | "alpha-blend against a neutral gray background" | Library default `bg=0.5f` exists but the **CLI never uses it**: for alpha images it computes `min(score(bg=0.1), score(bg=0.9))`. Also: blending happens in the **source encoding** (e.g. sRGB gamma space) *before* linearization. | `ssimulacra2_main.cc:105-114`; `ssimulacra2.cc:222-234, 429-432, 488-491` |
| 7 | "1-norm and 4-norm" | 4-norm = `sqrt(sqrt(mean(d⁴)))`. SSIM-map `d` is computed in **f32** (products `mu11/mu22/mu12` are float-rounded), accumulated in f64; EdgeDiff `d1` is computed in **double** from f32 inputs. | `ssimulacra2.cc:126-158, 162-195, 110-114` |
| 8 | "reuse whatever host-side Vulkan plumbing from dssim-vulkan" | **License-blocked as code reuse**: `dssim-vulkan` is AGPL-3.0 (dssim-vulcan `CHECKPOINT.md` M1: "crate is AGPL-3.0"). Study-only; any plumbing is clean-room reimplemented in Rust (§15). | dssim-vulcan `CHECKPOINT.md:45-66` |
| 9 | "prototype kernels in cubecl" | Rejected for this port (§19): the parity ladder needs explicit control of contraction/reassociation in raw GLSL, and the ash+glslc+checked-in-`.spv` path is already proven on this exact hardware by the sibling port. | research §4-5; dssim-vulcan `CHECKPOINT.md` M1 |

### Exact constants (do not re-derive from memory)

**Pipeline scalars:** `kC2 = 0.0009f` (`ssimulacra2.cc:41`); `kNumScales = 6` (`:42`); blur
σ = 1.5 (`:85`); scale stop `xsize<8 || ysize<8` (`:450`); CLI min size 8×8 rejected
(`ssimulacra2_main.cc:90-93`); CLI alpha `min(bg 0.1, bg 0.9)` (`:111-113`); score printed
`%.8f` (`:107`).

**Opsin (`opsin_params.h:20-43, 62-66`):** matrix rows
`[0.30, 0.622, 0.078]`, `[0.23, 0.692, 0.078]`, `[0.24342268924547819, 0.20476744424496821,
1−kM20−kM21]` (row 3 col 3 = 0.5518098665095536, computed as the complement — reproduce the
computation, not a retyped decimal); bias `kB0=kB1=kB2=0.0037930732552754493f`.
`premul_absorb[i] = matrix[i] · (IntensityTarget/255.0f)`; `neg_bias_cbrt = −cbrtf(bias[i])`
(`enc_xyb.cc:215-223`). Mixed channels clamped `max(0, ·)` before cbrt (`:86-88`).

**Cube root (`fast_math-inl.h:173-209`):** bit-trick exponent seed
(`0x54800000`, `0x002AAAAA`, zero special-cased), 3 Newton iterations
`r = fma(−x/3, r⁵, 4r/3)`, final iteration `r = fma(1/3, x−x·r⁵… )` per source, then
`r²·x + add`. Transcribe the exact operation sequence; max error 6 ulp is *by design* — a more
accurate GPU `cbrt` **diverges** from the oracle.

**Positive XYB (`ssimulacra2.cc:215-217`):** `B = (B−Y) + 0.55f; X = X·14f + 0.42f;
Y = Y + 0.01f;` in that order, per pixel, f32.

**Recursive Gaussian (`gauss_blur.cc:499-590`):** all derived in **double** at runtime from
σ, stored f32: `N = roundf(3.2795σ + 0.2546)` (=5); `ω = {π,3π,5π}/2N`; `p_k = ±1/tan(ω_k/2)`
(signs +,−,+); `r_k = ±p_k²/sin(ω_k)` (+,−,+); `ρ_k = exp(−σ²ω_k²/2)/N`; `D_ij` cross terms,
`zeta_15 = D_35/D_13`, `zeta_35 = D_51/D_13`; solve `A·β = γ` (A = 3×3 of p,r,zeta-row,
γ = `[1, N²−σ², ζ·ρ+ρ]`); `n2_k = −β_k·cos(ω_k(N+1))`; `d1_k = −2cos(ω_k)`.
Scan (scalar-equivalent of the SIMD forms): per section k,
`y_k[n] = n2_k·(in[n−N−1] + in[n+N−1]) − d1_k·y_k[n−1] − y_k[n−2]`, out[n] = Σ_k y_k[n],
n from −N+1, zero ring state, out-of-range inputs = 0. Horizontal pass writes a full-size
temp, then vertical pass (`gauss_blur.cc:615-620`). **The CPU horizontal loop uses a
4-output unrolled expansion (`mul_prev/mul_prev2/mul_in`, `:576-587`); the GPU uses the naive
recurrence — algebraically identical, differing only in f32 association; Phase D measures the
drift against oracle dumps and must stay ≤2e-6 or the expansion gets transcribed too.**
The Rust host computes these constants in f64 and **asserts bit-equality against the oracle
dump** of the `RecursiveGaussian` struct (Phase A dumps it) — libm differences (tan/sin/exp)
are caught there, not in a shader.

**SSIM map (`ssimulacra2.cc:126-158`):** f32: `mu11=mu1·mu1; mu22=mu2·mu2; mu12=mu1·mu2;
num_m = 1 − (mu1−mu2)²; num_s = 2·(s12−mu12) + kC2; denom_s = (s11−mu11)+(s22−mu22)+kC2;
d = 1 − num_m·num_s/denom_s` (the product/division in f32, then double), `d = max(d, 0)`;
norms: `mean(d)`, `sqrt(sqrt(mean(d⁴)))` in f64.

**Edge diff map (`ssimulacra2.cc:172-193`):** `d1 = (1+|img2−mu2|)/(1+|img1−mu1|) − 1` in
**double** from f32 inputs; `artifact = max(d1,0)` → 1-norm + 4-norm;
`detail_lost = max(−d1,0)` → 1-norm + 4-norm. GPU computes `d1` in f32 (GLSL has no portable
double) — a **documented deviation**, budgeted at ≤2e-6 per-pixel and measured in Phase E;
the f64 accumulation of the norms stays CPU-side over the read-back map.

**Score (`ssimulacra2.cc:261-417`):** 108 weights — **never retyped**; extracted from source
by a codegen step and checksum-verified against the oracle dump (§5 `weights_gen`). Iteration
order c-major → scale → norm n∈{0,1}, three terms per (c,scale,n): `avg_ssim[c·2+n]`,
`avg_edgediff[c·4+n]`, `avg_edgediff[c·4+n+2]`, all through `std::abs`, f64 accumulate.
Then `s·=0.9562382616834844`; `s = 2.326765642916932·s − 0.020884521182843837·s² +
6.248496625763138e-05·s³`; `score = s>0 ? 100 − 10·pow(s, 0.6276336467831387) : 100`.

**Open question pinned by Phase A experiment, not by reading:** how gray-source inputs flow
through `ImageBundle` → `ToXYB` (1-channel vs replicated planes). The oracle's dumped planes
define the behavior; the Rust CPU path mirrors the dump.

---

## 3. Core Strategy: Oracle-Dump Parity, Hybrid Compute

```text
C++ ssimulacra2 (this repo, #ifdef-gated dump patch, default off)
        |  golden dumps: decoded linear RGB, rg constants, per-scale
        |  XYB, sigma1_sq/sigma2_sq/sigma12/mu1/mu2, d-maps, 108 norms,
        |  weighted sum, final score  (byte-reproducible = M0 exit)
        v
Rust ssimulacra2-vulkan crate
  ├── cpu.rs   faithful reimplementation (parity chain to C++ dumps, near-bit-exact)
  │            also the runtime fallback path
  └── gpu.rs   Vulkan compute:
        upload linear RGB (or XYB at first) → XYB+rescale kernel →
        Multiply fused into blur inputs → recursive-Gaussian H & V kernels →
        combine kernels emit 6 per-pixel d-maps per scale (3 ssim + 3 edge) →
        read back maps → CPU f64 norms + weights + score (identical code path
        to the oracle's arithmetic, same accumulation order)
        v
     score within ladder of oracle on every fixture
```

Two independently solvable problems, never debugged together: (1) can Vulkan reproduce the
spatial core (XYB, blur, maps)? (2) can the remaining CPU stages (decode, ICC, pooling) move
without changing semantics? Pooling deliberately **does not move** in this plan's scope.

Parity is defined at the **float-plane level via dumps**, not at the PNG-byte level: image
decoders (libpng vs Rust `png`) agree on decoded 8-bit samples for PNG, but JPEG IDCT
differences would masquerade as GPU bugs. JPEG fixtures therefore enter the Rust side through
oracle-dumped pixels.

---

## 4. Reuse Boundary

### Reuse directly
- C++ `ssimulacra2` + `src/lib` as the **build-and-run oracle** (unmodified except the
  `#ifdef SSIMULACRA2_DUMPS` patch; default binary unchanged).
- Its constants, weights, and semantics — via extraction/dumps, never retyped.
- dssim-vulkan's *design* lessons (context serialization, glslc pinning, dispatch-in-one-submit
  with barriers, dump format shape) — documented in §2/§10, reimplemented, not copied (AGPL).

### Port/adapt (clean-room, in Rust)
- Vulkan context/device/queue/staging/allocator plumbing (ash + gpu-allocator).
- Dump binary format + comparison utility (max-abs, mean-abs, RMSE, worst pixel, CPU/GPU
  values) — same report fields the sibling port validated with.
- One-shot fence-wait submit pattern; multi-dispatch single submit with barriers.

### Reimplement from source (the algorithm proper)
- sRGB→linear (TF_SRGB piecewise EOTF, `enc_color_management.cc:140,1019` fast path),
  alpha blend, ToXYB + CubeRootAndAdd transcription, MakePositiveXYB, Downsample, Blur,
  Multiply, SSIMMap, EdgeDiffMap, norms, Score.

### Do not rebuild
- A second image-decode stack beyond PNG (+ oracle-fed pixels for JPEG); a general Vulkan
  framework; a weight-tuning pipeline; ICC transform math.

---

## 5. Project Shape

```text
ssimulacra2-vulcan/                  (repo root: existing C++ stays untouched)
├── Cargo.toml                       workspace → one member crate
├── ssimulacra2-vulkan/
│   ├── Cargo.toml                   ash, gpu-allocator, png, bytemuck (+dev: nothing exotic)
│   ├── shaders/                     *.comp + checked-in *.spv (pinned glslc)
│   │   ├── xyb_positive.comp        Phase C
│   │   ├── blur_h.comp              Phase D (naive recurrence, 1 thread = 1 row)
│   │   ├── blur_v.comp              Phase D (1 thread = 1 column)
│   │   ├── maps_combine.comp        Phase E (ssim d + edge d1 from mu/sigma planes)
│   │   └── downsample_box2.comp     Phase F (clamp-replicate, /4 normalize)
│   ├── src/
│   │   ├── lib.rs  context.rs  transfer.rs  pipeline.rs
│   │   ├── cpu.rs                  faithful CPU reimpl = fallback + parity bridge
│   │   ├── xyb.rs  blur.rs  maps.rs  scales.rs  score.rs (norms+weights+polynomial, f64)
│   │   ├── oracle_dump.rs          dump reader + comparator
│   │   └── weights_gen.rs          build-time extraction from ../src/ssimulacra2.cc + checksum vs dump
│   └── tests/                      parity suites per phase
├── oracle/                          build docs (WSL2/CI), dump patch (default-off), fixture generator
├── tests/fixtures/                  created Phase A: RGB pairs, odd sizes, 8..15 near-cutoff,
│                                    alpha pair, gray pair, gradient/noise/step, identical pair, 2048²
└── dumps/                           gitignored
```

Dependencies justified per AGENTS.md §8: `ash` (the API), `gpu-allocator` (buffer memory,
proven in sibling), `png` (fixtures are PNG), `bytemuck` (f32/bytes for dumps). Pin versions
at Phase B start against what the sibling proved as a floor — not from memory.

---

## 6. Implementation Sequence

### Phase A — CPU oracle + golden dumps  → M0
**Target:** 3–5 days
1. Build the C++ oracle on Linux (WSL2 here; ubuntu CI later) with README deps.
2. Add `#ifdef SSIMULACRA2_DUMPS`-gated dumping to `ssimulacra2.cc` (test-only; default
   binary byte-identical): decoded linear RGB planes, `RecursiveGaussian` struct, per-scale
   XYB (pre/post rescale), sigma1_sq/sigma2_sq/sigma12/mu1/mu2, per-pixel d-maps, 108 norms,
   weighted sum, final score, plus scale count and the weight-index shift behavior on a
   small-height fixture.
3. Create the fixture set (§12) and generate goldens; record CLI scores (`%.8f`) per fixture,
   including the alpha worst-of-two-bg path and a gray pair (resolving the gray open question).
4. Dump format: fixed 48-byte header + LE f32/f64 payload, self-describing dims/stride
   (sibling-proven), `MANIFEST.txt` per run.

**Exit observation:** two separate oracle runs produce byte-identical dumps (SHA-256); CLI
scores stable across runs; default-off binary unchanged (hash compare vs pre-patch build).

### Phase B — Minimal Vulkan runtime  → M1
**Target:** ~1 week
Clean-room instance/device selection (discrete > integrated), validation in dev builds,
compute queue, one-shot fence submits, staging upload/readback, `noop`/×2 smoke shader,
**serialized context creation** (AMD driver fact), debug names. lavapipe leg = CI only.

**Exit observation:** smoke shader element-exact on RX 6600M locally and lavapipe in CI,
validation clean.

### Phase C — XYB + positive rescale kernel  → M2
**Target:** ~1 week
Transcribe `CubeRootAndAdd` operation-for-operation (seed + 3 NR + final + `r²x+add`),
opsin premul (assert matrix premul + `neg_bias_cbrt` bit-equal to oracle dump), clamp order,
X/Y/B mixing, MakePositiveXYB in source order. Test standalone against per-scale XYB dumps on
real fixture data + synthetic extremes (0, negative-mixed clamps, wide-gamut values).

**Exit observation:** GPU XYB planes ≤1e-6 max-abs vs oracle dumps on all fixtures; constants
bit-equal.

### Phase D — Recursive Gaussian blur  → M3
**Target:** 1–2 weeks. The most correctness-critical kernel.
Naive recurrence per section k, folded-input indexing (`n−N−1`, `n+N−1`), zero padding, warmup
rows before first output, H→V order, per-row/per-column thread scans; constants from the
dumped struct via push/uniform buffer. Equivalence battery ported to this repo: constants,
gradients, steps, impulses, random noise, odd sizes, near-cutoff sizes (8..16), strided
sub-images. Measure naive-vs-CPU-unrolled drift; if >2e-6, transcribe the 4-output expansion.

**Exit observation:** blur outputs ≤2e-6 max-abs vs dumps across the battery; drift report
recorded in CHECKPOINT.

### Phase E — Single-scale error maps  → M4
**Target:** 1–2 weeks
Upload oracle XYB for scale 0; GPU: fused Multiply+blur (3 products) + 2 means = 5 blurred
images; `maps_combine` emits ssim-d (f32 semantics per §2) and edge-d1 (f32, documented
double→float deviation); read back 6 maps; CPU f64 norms in source accumulation order.

**Exit observation:** all 18 norms for scale 0 ≤1e-6 vs dumps; per-pixel d-maps ≤2e-6; edge-d1
deviation measured and reported.

### Phase F — Full 6-scale pipeline  → M5, M6
**Target:** ~1.5 weeks
GPU Downsample (clamp-replicate + /4, exact accumulation order) + per-scale XYB recompute;
scale loop with the exact stop rule and weight-index behavior; Rust `cpu.rs` path validated
against dumps first (it is the fallback and the parity bridge); GPU path end-to-end.

**Exit observation:** M5: pipeline runs on all fixtures with maps matching dumps per stage.
M6: final scores ≤1e-5 on all fixtures, norms/weighted sum ≤1e-6, identity exactly 100.0.

### Phase G — Matrix, CLI, CI, fallback  → M7, M8
**Target:** ~1 week
Validation matrix (§12) green; `--gpu` flag on the Rust CLI (PNG inputs; ICC → explicit
reject message pointing at the C++ binary; JPEG via oracle-fed pixels or documented
unsupported); CPU fallback on device-creation failure; CI: ubuntu + lavapipe + oracle build
cached, `.spv` freshness via pinned-SDK glslc step (manual, per BH38 lesson); benchmark
harness skeleton.

**Exit observation:** CI green on lavapipe; local real-GPU run green; fallback exercised by
device-forcing test.

### Phase H — Performance (plan revision approved by human 2026-09-08)  → M9 done; M10 adjudicated (a); M10-batch CLOSED 2026-09-09
M9 delivered (per-process timings, big GPU ~2.3x slower than oracle) and two post-M9
findings shape this revision: (a) removing the 4 f64 muls/pixel from maps_combine changed
wall time NOT AT ALL (launch/sync-bound, CI #20 confirmed correctness) — arithmetic is
not the bottleneck at current overhead; (b) research §5's dssim-vulkan claim ("host-side
data prep dominates ~70-90 ms at 2048^2, GPU ~5%") is a HYPOTHESIS about another codebase
(AGENTS §0) and our pre-pipeline path (VkContext init with validation, png decode x2,
clone/blend, to_linear x2 ~100MB, upload ~100MB) was NOT in the original stage list —
H0 must therefore cover 100% of process wall time, with a residual line; a large residual
is itself the finding.

- **H0 DONE (2026-09-08): measured profile of big 2048^2 (quiet batch, wall 1870 ms,
  residual 4.5%):** readback 742 (39.7%) > prep 408 (21.8%) > blur 235 (12.6%) >
  context-init 170 (9.1%) > decode 57 > upload 50 > norms 39 > mul 37 > xyb 24 >
  downsample 18 > maps 7. GPU arithmetic itself ~320 ms; ~45% of wall is HOST/SYNC
  (the research §5 "host prep" story confirmed as ours, and bigger than assumed).
  Causes located: readback = fresh HOST_COHERENT ~100 MB staging + Vec<u8> copy +
  zero-init Vec<f32> + byte-loop per call (12 calls), 3-4x inflated; prep =
  srgb_to_linear degree-4 rational (~8 mul_add + true div) per element over 25.2 M
  elements - yet 8-bit PNG inputs are only k/255, so a 256-entry table computed by
  the SAME function is bit-identical by construction. Measurement-protocol finding:
  laptop wall times swing up to 1.6x between batches (thermal/all-core oracle
  interleaving) - DEV-ITERATION protocol = warm same-session per-impl MIN of >=5
  alternating runs (fast A/B); FINAL M10 adjudication = COLD-machine batch (see
  the M10 definition below; a laptop swings ~150 ms batch-to-batch, larger than
  the remaining levers, so only a cold batch can decide the bar), plus
  M9's 0.82 s oracle number NOT reproducible this session (measured 1.13-1.46 s);
  M10 compares against the FRESH same-session oracle measurement, not the old one.
- **Lever ranking per data (reorders H1-H5; M10 re-checked after each):**
  H1 = readback fix: persistent grow-only staging buffer, one memcpy (no Vec<u8>
  +no zero-init+no byte loop), sd+ed fused into one buffer/scale (12 syncs -> 6);
  purely host-side, bit-exact by construction.
  H2 = pipeline/descriptor/shader-module cache + real VkPipelineCache (persisted)
  + one submit+fence per scale with explicit compute->compute barriers; gated per
  point 6 (>=5 suite repeats, validate_sync on, CI green, revert-on-red).
  H3 = prep: 8-bit LUT path for alpha-free images (table values computed by
  srgb_to_linear itself -> bit-identical; blended-float alpha path keeps the
  per-element function), plus avoid the redundant srgb.clone() copies if free.
  H4/H5 = blur tiles, mul fusion, f64-div emulation - ONLY if post-H1-H3 re-profile
  still ranks them; GPU-side floor (~320 ms) may make them moot for M10.
- **H1 detail (readback, target ~600 ms):** grow-only persistent HOST staging buffer
  (one alloc reused, never destroyed per call); replace the Vec<u8> copy + zero-init
  Vec<f32> + elementwise from_le_bytes loop with ONE copy_to_vec of aligned f32 words;
  maps_combine writes sd+ed into one 6*n buffer -> 6 readbacks/syncs instead of 12.
  Values are byte-identical copies of the same device buffers - bit-exactness is
  structural; gate = suite + timing. GPU-side norms stay a non-goal (data would have
  to scream first; readback fix does not require them).
- **H2 detail (dispatch overhead, target ~350 ms):** per-(spv,bindings,push) cache of
  shader-module/dsl/layout/pipeline + persistent descriptor pool + REAL VkPipelineCache
  (not NULL; persisted to disk - human point 7) and one submit + one fence per scale.
  H2.1 (caching) is pure host plumbing, values cannot change; suite + timing gate.
  H2.2 (fused submit) removes the implicit per-dispatch fence ordering: explicit
  compute->compute vkCmdPipelineBarrier between every dependent dispatch; barrier bugs
  can be RACES, not deterministic wrong values (human point 6), so the H2.2 commit is
  gated by (i) correctness suite run >=5 times locally, (ii) Vulkan synchronization
  validation enabled for those runs (VK_LAYER_SETTING=khronos_validation.validate_sync),
  (iii) CI green, (iv) timing. Revert-on-red; highest-risk commit of the phase.
- **H3 detail (prep LUT, target ~350 ms):** 8-bit PNG alpha-free fast path:
  decode records the bit-depth/normalization fact; to_linear indexes a
  table[k] = srgb_to_linear(k/255.0) built once with the SAME function -> table output
  bits are identical to the per-element path by construction (assert table vs
  per-element over all 256 values in a unit test); alpha/blend path (arbitrary floats)
  keeps the current function untouched. Remove srgb.clone() by fusing blend where free.
- **H4 detail (blur tiles):** shared-memory/segmented IIR - ONLY if post-H1..H3
  re-profile still ranks blur material. Recursive IIR order is reassociation-sensitive:
  H4 must preserve BIT-EXACT output or it FAILS and is recorded as such (no
  drift-budget exception — the top rule stands unconditional — human point 2).
- **H5 detail (rest):** mul-fusion into neighbours, f64-division emulation in
  maps_combine — ONLY if the post-H1..H3 profile still ranks them material
  (currently expected: no; same "measure, don't assume" that killed the f64 theory).
- **After EVERY H-step: re-check M10** (big vs oracle, same-session per-impl batches,
  MIN of >=5 runs each — the H0 protocol finding, not median-of-interleaved).
  If already met, remaining H-steps are skipped, not "polished"
  (human point 3). Non-goal list (f16, multi-GPU, GPU decode/ICC/GPU norms) unchanged.
- **M10 definition (unchanged bar; protocol pinned by human 2026-09-09):** PASS =
  big GPU beats big C++ oracle (the reference is NOT to be re-based onto the
  slower Rust CPU path - that would be moving the goalposts; Rust-vs-GPU numbers
  may be reported as supplementary facts only). FINAL adjudication protocol =
  COLD-machine, same-session, per-impl MIN-of-5 alternating batch; warm
  interleaved runs are dev-iteration A/B only (batch-to-batch swings of
  ~150 ms on this laptop exceed the remaining lever sizes, so warm numbers
  cannot decide M10). Photo clause: photo GPU is not worse than photo oracle
  by >10% - structurally unreachable in the current 1-process-per-image CLI
  (vkCreateInstance+Device alone ~190 ms on this driver vs 31 ms total oracle;
  a future batch/daemon mode reusing one VkDevice could change this - the
  claim is scoped to the architecture, not eternal). Any photo loss is an
  unresolved gap reported for human decision, never silently accepted. All
  correctness gates (RDNA2 bit-exact dumps, all-device identity asserts,
  llvmpipe sanity bars, committed validate_sync) must stay green; no
  tolerance moves.
- **Measurement-condition record (so numbers don't look like regressions):**
  prep reads ~96 ms warm (H3 batch) vs ~116-128 ms cold - same code, cache/I/O
  conditions differ; stage numbers are only comparable within a batch type.
- **H6 residual, closed:** instrumented "teardown" (BufGuard drop ~62 ms) +
  pre/post-main ~37 ms -> in-main RESIDUAL now 3.8% (< 5% gate, H0 coverage
  rule satisfied). The exit-skip A/B earlier tested the wrong scope (guard
  drops inside the pipeline fn, before any late process::exit) - lesson
  recorded in CHECKPOINT. Remaining identified levers if the human wants to
  keep pushing: buffer-churn reduction (create/destroy ~72 device buffers +
  upload staging churn; est. ~100-150 ms) and H4 blur tiles (the only lever
  with GPU-side headroom, highest risk, bit-exact-or-fail). Decode-LUT fusion
  alone is NOT worth its invasiveness (would still land ~0.88 vs ~0.81 oracle).
Original levers still apply: RGBA-packed planes and precomputed-reference/fast-ssim2
caching remain candidates after profiling, behind the same gates. No optimization
before M6 parity — already satisfied.

- **PHASE H + BATCH CLOSED (2026-09-09; full record in CHECKPOINT):** human
  accepted option (a) for the single-pair CLI (big 1.18-1.33x, photo 9-13x —
  context-init is structural at one process per pair; verdict recorded, not
  re-based). Batch round 1 shipped (6533a3c, CI #27): original-pyramid
  caching, single-shot == fused == split bit-exact over the corpus, 4K A/B
  836 vs 1038 ms/img = 19.5% — accepted as the measured ceiling after the
  27% figure was traced to a pipeline-share-vs-wall-share unit error.
  Routing wired at GPU_ROUTE_MIN_PIXELS=500_000 (d54ad86, CI #28) against
  the correct comparator (in-binary CPU engine; measured crossover
  0.40-0.45 MP via bench/routing_calib.py). The 8.3 MP figure is kept as a
  DISTINCT number: GPU-vs-C++-oracle competitiveness, not routing — the
  two-crossovers separation is pinned in README, main.rs const doc, and
  AGENTS 4.5 (numbers-carry-comparator rule). Pipelining/daemon parked at
  H4 status (opt-in on product request only). No open levers.

---

## 7. Numerical Correctness Policy

| Stage | Bar | Notes |
|---|---:|---|
| rg constants, opsin premul, neg_bias_cbrt | bit-exact vs dump | host-side assert, not shader math |
| weights (108) | bit-exact | codegen + checksum vs dump |
| XYB + rescale | ≤1e-6 max-abs | target ~0 with faithful transcription |
| Downsample | ≤1e-7 max-abs | same order → expect bit-exact |
| blur (H, V) | ≤2e-6 max-abs | reassociation drift; expansion fallback |
| ssim d-map | ≤2e-6 max-abs | f32 semantics identical to CPU |
| edge d1 map | ≤2e-6 max-abs | **documented double→float deviation** |
| 108 norms (CPU f64) | ≤1e-6 abs | primary numeric bar |
| weighted sum (CPU f64) | ≤1e-6 abs | score is derived from it |
| final score | ≤1e-5 abs (score ≤ 99 band) | near-100 region judged on weighted sum — `pow(s, 0.627…)` amplifies tiny sums drift into visible score drift |
| identity | exactly `100.0` (`==`) | §8 |

When a bar fails: locate the first diverging stage via dumps, inspect the exact pixel/op,
classify (algorithmic / layout / FP ordering), fix the cause. Tolerances are never widened
without a CHECKPOINT entry proving the cause is oracle-side nondeterminism (there is none —
the oracle is single-threaded, `null_pool`).

FP rules: no fast-math; deterministic reductions only; f32 everywhere on GPU except where the
oracle uses f64 (which stays CPU); inspect generated SPIR-V for contraction where association
matters (sibling lesson: `OpFma` audit + `NoContraction` when transcribing exactly); the
`CubeRootAndAdd` sequence is transcribed op-for-op — its 6-ulp sloppiness is load-bearing.

---

## 8. Identity Semantics

`orig == dist` (bit-identical inputs) must yield exactly `100.0`: all five blurred images are
then produced by identical dispatches on identical buffers, so `s12 == s11 == s22` and
`mu12 == mu11 == mu22` bitwise, `num_s == denom_s`, `d == 0` exactly, norms 0, weighted 0,
score 100. This holds only if both images go through the **same** kernel/order — do not
"optimize" one side. A deterministic input-equality fast path is allowed as a contract
safeguard; the GPU identity regression test stays regardless.

---

## 9. GPU Data Representation

Correctness first: planar f32 SSBOs, one buffer per plane, stride = width (matches the CPU's
`Image3F` plane layout and the dump format 1:1 — a dump file *is* an upload buffer). No
packing, no storage images, no RGBA interleaving until Phase H measures a benefit. Row-major
linear layout; no padded row pitch (staging copies handle alignment).

---

## 10. Synchronization and Dispatch Strategy

One submit per scale with explicit pipeline barriers between dependent dispatches (sibling's
`dispatch_sequence` pattern); fence-wait readback at scale boundaries where maps are needed
CPU-side. Never remove a dependency because a small test passed. Barrier reduction and
cross-scale overlap are Phase H, behind measured parity re-validation.

---

## 11. Performance Strategy

Profile independently: decode, CPU prep (linearize/alpha), XYB, upload, blur H/V, combine,
readback, norms/score. Report kernel-only throughput **and** end-to-end pair latency.
Expected-dominant stages (inference, to be measured): 5-blur-per-scale traffic and scale-0
map readback (~24 B/px); host prep. Optimization order: resource reuse → descriptor/pipeline
reuse → reduce transfers (keep XYB GPU-resident across scales, fuse Multiply into blur loads)
→ kernel fusion → tiling → GPU reductions (deterministic only). Crossover thresholds only
when measured. Vship's published numbers (13× CPU reference at 1080p) are a feasibility
signal, not a target to claim.

---

## 12. Validation Matrix

Created in Phase A (this repo has **no test fixtures yet** — a real gap):
- RGB pair (photographic), gradient pair, random-noise pair, step-edge pair
- odd dimensions (e.g. 101×67), near-cutoff sizes (8×8 … 15×15 → exercises the break rule and
  weight-index shift), one sub-cutoff pair (CLI rejection behavior)
- alpha pair (exercises worst-of-bg 0.1/0.9 path)
- gray pair (resolves the gray open question against the oracle)
- identical pair (exact-100 contract)
- large pair 2048×2048 (perf + memory)
- JPEG pair (oracle-dump mode only)

Primary oracle = this repo's C++ binary. Vship (HIP/CUDA, MIT) may serve as a *secondary*
cross-check on the same GPU family later; disagreement between two non-oracles is never
resolved by majority vote — it goes back to the C++ source.

---

## 13. CI / Hardware Matrix

- **PRs (ubuntu-latest):** C++ oracle build + golden regeneration (cached), Rust workspace
  `cargo test` on lavapipe (mesa vulkan drivers), CPU-path tests, validation layer ON.
- **Local (this host):** RX 6600M + iGPU real-GPU runs; context creation serialized.
- **Nightly/scheduled (later):** none until Phase G lands; then consider pinned-SDK glslc
  freshness step (manual per BH38 — SPIR-V bytes are toolchain-version-sensitive).
- No single-vendor success is treated as portability proof; format/feature queries with
  explicit CPU fallback.

---

## 14. Research/Reuse Sources Worth Keeping Open

- `src/ssimulacra2.{h,cc}`, `src/lib/jxl/{gauss_blur,enc_xyb,opsin_params,fast_math-inl,
  enc_color_management}.{h,cc}` — the algorithm ground truth (this repo).
- Charalampidis, "Recursive Implementation of the Gaussian Filter Using Truncated Cosine
  Functions" (2016) — the blur's math (cited by formula number in `gauss_blur.cc:497-589`).
- `D:\.Github_Project\.repo\dssim-vulcan` — design/pattern/lesson study **only** (AGPL-3.0;
  no code may flow from it — §15).
- Vship (`codeberg.org/Line-fr/Vship`, MIT) — GPU port feasibility + secondary numeric
  cross-check; not a base to build on.
- fast-ssim2 paper/notes — Phase H optimization ideas only (separable/split/precompute).
- Vulkan SDK (pinned glslc), ash, gpu-allocator docs — infrastructure.

---

## 15. License / Provenance Rule

This repo is BSD-3-Clause (Cloudinary) + `PATENTS` grant. The port crate inherits the repo's
posture: keep copyright notices on anything adapted from `src/`; `CubeRootAndAdd` derives from
vectormath (Apache-2, per its comment — attribution travels). Every non-trivial reuse gets a
provenance entry: `source / file:function / license / copied|adapted|reimplemented / reason`.

**AGPL firewall:** `dssim-vulkan` (sibling) is AGPL-3.0. No code, no translated-to-Rust
transcriptions, no adapted buffer layouts-as-code from it — only *documented design decisions*
(patterns, tolerances, toolchain lessons) enter this plan and the Rust code, each traceable to
this repo's own source or public Vulkan/algorithm references. The `ash` plumbing is small
enough that clean-room is cheap; if a future audit finds dssim-vulkan-shaped code here, it
fails the firewall and gets rewritten.

The oracle dump patch to `ssimulacra2.cc` is a local, clearly-marked, default-off modification
of BSD code — provenance: same repo, same license, logged in `oracle/README.md`.

---

## 16. Milestones (this list now owns the ids; `AGENTS.md` §9 mirrors it)

```text
M0  Plan docs approved + CPU golden dumps/scores byte-reproducible      (Phase A)
M1  Vulkan smoke test on lavapipe (CI) + one real GPU (local)           (Phase B)
M2  XYB conversion + positive-XYB rescale matches oracle dumps          (Phase C)
M3  GPU blur passes the equivalence battery within ladder               (Phase D)
M4  Single-scale GPU error maps + norms match oracle                    (Phase E)
M5  Full 6-scale pipeline runs, per-stage maps match oracle             (Phase F)  \ critical
M6  Norms + weighted sum + final-score parity (≤1e-5; identity ==100)   (Phase F)  / functional
M7  Alpha/gray/odd-size/small-image/near-cutoff matrix green            (Phase G)
M8  CLI + CI + CPU fallback integrated                                  (Phase G)
M9  Performance profile completed                                       (Phase H)
M10 Optimized path beats the C++ ORACLE on chosen workloads       (Phase H;
    comparator fixed by human review 2026-09-09 - NOT the slower Rust CPU
    path). Adjudicated: option (a) accepted - single-pair CLI NOT met,
    structural (context-init ~190 ms/process; big 1.18-1.33x, photo 9-13x).
    Recorded, no goalpost move.
M10-batch Shared-original batch beats oracle per-image at production
    resolution (addendum ssimulacra2-vulkan-batch-addendum.md). CLOSED
    2026-09-09: original-pyramid caching three-way bit-exact (CI #27),
    4K A/B 19.5% accepted (27% target superseded - unit error); single-pair
    CLI routing wired at 0.5 MP vs the in-binary CPU engine (CI #28,
    bench/routing_calib.py); --gpu override; pipelining parked H4-style.
```

M5+M6 are the critical functional pair: at M6 there is a useful Vulkan SSIMULACRA2 even with
pooling on CPU. Don't sequence ahead of an observed exit (§AGENTS 9).

---

## 17. Time Estimate

| Milestone group | Expected effort |
|---|---:|
| Oracle build + dumps + fixtures (A) | 3–5 days |
| Vulkan runtime (B) | ~1 week |
| XYB kernel (C) | ~1 week |
| Blur kernel + battery (D) | 1–2 weeks |
| Single-scale maps (E) | 1–2 weeks |
| Full pipeline (F) | ~1.5 weeks |
| Matrix/CLI/CI (G) | ~1 week |
| Initial optimization (H) | 2–4 weeks |

Functional GPU SSIMULACRA2: roughly 5–9 weeks; hardened + optimized: 7–13 weeks. First hard
checkpoint is M3 (blur): if the IIR reassociation drift misbehaves on real drivers, revise
from evidence, not from this table.

---

## 18. First Actions

*(Executed 2026-09-08 — oracle builds NATIVELY on MSYS2 ucrt64 on this host,
not WSL2; every step below is done, results in CHECKPOINT M0+. Text kept verbatim.)*

```text
1. Build the C++ oracle in WSL2 (or CI container); confirm README deps suffice.
2. Write the #ifdef SSIMULACRA2_DUMPS patch; dump the §6-Phase-A stage list for one fixture.
3. Run twice; SHA-256 the dumps; verify default-off binary hash unchanged.
4. Create the fixture set; record CLI scores incl. alpha worst-of-bg and gray behavior.
5. Scaffold the Rust workspace + clean-room context.rs; ×2 smoke shader on RX 6600M + CI lavapipe.
6. Upload one oracle XYB plane; run the Phase C kernel; compare against the dump.
```

Do **not** start with: GPU decode, ICC, cubecl, RGBA packing, fused multi-stage kernels, GPU
reductions, multi-GPU, or any Phase H idea.

---

## 19. Decision Summary

Recommended: **Rust + ash + raw GLSL (glslc-pinned, checked-in `.spv`), hybrid compute
(GPU spatial core, CPU f64 pooling), oracle-dump-defined parity.**

Alternatives considered, one line each:
- **cubecl** (research §4 pick): lost on FP-control grounds — the ladder needs op-exact
  transcription (cbrt polynomial, contraction audit) and cubecl's SPIR-V backend doesn't
  offer that control cheaply; plus it's unproven on this host while ash is proven by the
  sibling on this exact GPU pair.
- **rust-gpu / krnl**: archived-upstream churn and stale pinning per research §4; lost to
  toolchain risk.
- **All-GPU incl. pooling**: rejected — CPU f64 pooling is free (maps dominate anyway) and
  removes an entire reduction-order bug class from the critical path.
- **C++/Vulkan-Hpp sidecar inside the existing build**: rejected — splits the port from the
  proven Rust ecosystem patterns of the sibling and doubles the CI matrix.
- **Port from Vship's kernels**: rejected as a base (different structure, HIP/CUDA); kept as
  secondary cross-check only.

The shortest credible path: **oracle dumps → XYB → blur → single-scale maps → 6-scale →
integration → optimization.** No implementation work is authorized by this plan; approval of
this plan (plus writing `SSIMULACRA2_VULKAN_PORT_PLAN.md`) is the gate to Phase A.

---

## 20. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| IIR blur reassociation drift >2e-6 on real drivers | Medium | High (M3 slip) | Naive-vs-expansion fallback defined in Phase D; battery includes the exact CPU-unrolled comparison; drift report is an exit artifact |
| `CubeRootAndAdd` transcription diverges (compiler contracts NR steps) | Medium | High (M2) | Op-for-op transcription + SPIR-V `OpFma` audit + `NoContraction`; per-plane dump compare localizes instantly |
| C++ oracle won't build on Windows host | High (expected) | Low | Oracle lives in WSL2/CI by design (§1 assumption); Windows runs only the Rust side |
| No fixtures exist yet; gray/alpha behavior unknown | Certain | Medium | Phase A creates the corpus and *pins* gray/alpha behavior from the oracle, not from reading libjxl |
| Weight transcription error (108 f64 literals) | Medium | Fatal silently | Codegen extraction + checksum vs oracle dump; never retype |
| Missing-scales weight-index shift mis-ported | Medium | High on small images | Near-cutoff fixtures (8..15) in matrix; dump includes scale count |
| AMD driver concurrency (`VK_INCOMPLETE`) | Known | Medium | Serialized context creation (sibling-proven) |
| glslc/SPIR-V toolchain drift | Known | Medium | Pinned SDK, checked-in `.spv`, manual freshness check only (BH38 lesson) |
| EdgeDiff double→float drift exceeds ladder | Low | Medium | Measured at Phase E with a dedicated report; fallback = f64 edge-d1 via `VK_KHR_shader_float64` only if the device supports it *and* parity demands it (opt-in, logged) |
| Readback volume dominates (24 B/px scale 0) | High (perf only) | Low for correctness | Phase H: fuse norms into GPU with deterministic order *after* parity is banked |
| AGPL contamination via sibling code | Low | Legal | §15 firewall; provenance entries; audit target |
| v2.0 vs v2.1 oracle confusion | Low | Fatal silently | AGENTS.md §2 version pin; goldens recorded with the oracle's own version banner (`SSIMULACRA 2.1`, `ssimulacra2_main.cc:30`) |
