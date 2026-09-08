# CHECKPOINT

Durable, append-only status log for the SSIMULACRA2 Vulkan port. Read this first at the start
of every session (see `AGENTS.md` §1). Append new entries; never delete history.

Milestone ids refer to `AGENTS.md` §9 (provisional) until
`ssimulacra2-vulkan-fable-plan.md` is written in Phase 0; after that, the plan's milestone
list owns the ids.

---

## 2026-09-08 — M0 (not started)

- Done: Stage set. `AGENTS.md` (operating rules adapted from the dssim-vulkan port: authority
  order, checkpoint protocol, git discipline, resource safety, provisional M0–M10 milestones),
  this file, and `.gitignore` created. Ground truth confirmed in-repo: `src/ssimulacra2.cc`
  (446 lines, v2.1 per commit `fb5d0db`) + vendored libjxl under `src/lib/`. Reference port
  studied at `D:\.Github_Project\.repo\dssim-vulcan` (AGENTS.md/CHECKPOINT.md/plan-doc
  structure; its port reached Phase H / bug-hunt pass 10 with the same discipline).
- Deviated from plan: none (no plan docs exist yet — writing them is the first task).
- Blocked / open question: two decisions Phase 0 must settle, both flagged in AGENTS.md:
  (1) stack — `ash` + GLSL/SPIR-V (proven in dssim-vulkan) vs `cubecl` (research notes §4
  recommendation); (2) reuse boundary — dssim-vulkan is AGPL-3.0, so its host-side plumbing is
  study-only; anything wanted from it must be clean-room reimplemented (AGENTS.md §10).
  Note: `ssimulacra2-vulkan-port-research.md` is still untracked.
- Next: Phase 0 — read `src/ssimulacra2.{h,cc}` end to end, then write
  `SSIMULACRA2_VULKAN_PORT_PLAN.md` (every constant file:line-cited: alpha blend, sRGB→linear,
  XYB + positive-XYB rescale, 6-scale linear downsample, σ=1.5 Gaussian, the three error maps,
  1-norm/4-norm pooling, 108-weight sum + score polynomial) and
  `ssimulacra2-vulkan-fable-plan.md` (phases, exit observations, tolerance policy, reuse
  boundary, milestones replacing AGENTS.md §9). M0 exit observation after that: CPU golden
  dumps/scores generated twice, byte-for-byte reproducible.

## 2026-09-08 — M0 (in progress: execution plan drafted)

- Done: `ssimulacra2-vulkan-fable-plan.md` drafted (20 sections, mirroring the dssim-vulkan
  plan's shape). Built on a full read of `src/ssimulacra2.{h,cc}` (491 lines) plus
  `src/lib/jxl/{gauss_blur,enc_xyb,opsin_params,fast_math-inl}.h/cc` — every constant in its
  §2 is file:line-cited from this session's reads. It settles the two open questions from the
  previous entry: (1) stack = Rust + ash + raw GLSL (glslc-pinned, checked-in .spv), cubecl
  rejected on FP-transcription-control grounds + unproven on this host; (2) reuse boundary =
  dssim-vulkan is AGPL-3.0 → design-lessons-only, clean-room Rust, firewall written into plan
  §15. Parity strategy = oracle-dump-defined (C++ binary with default-off `#ifdef
  SSIMULACRA2_DUMPS` patch; GPU spatial core; CPU-f64 norms/weights/score stay authoritative;
  identity contract = exactly 100.0). Milestones §16 match AGENTS.md §9.
- Deviated from plan: none yet (plan is a draft, not approved; no code touched). Notable
  findings pre-empted in the draft: research notes' FIR-blur suggestion contradicts the
  reference's recursive IIR (plan targets the recurrence instead); Downsample edge
  clamp-replicates and normalizes by full 4 (not dssim's floor-drop); CLI alpha path is
  min(score@bg0.1, score@bg0.9), never the 0.5 default; missing scales shift the 108-weight
  indices; `CubeRootAndAdd` is a deliberate 6-ulp Newton polynomial that must be transcribed,
  not "improved".
- Blocked / open question: gray-input flow through `ImageBundle`→`ToXYB` unresolved by reading
  — pinned as a Phase A oracle experiment (plan §2). `SSIMULACRA2_VULKAN_PORT_PLAN.md` (full
  citation trail) still unwritten.
- Next: human review/approval of the fable-plan → write `SSIMULACRA2_VULKAN_PORT_PLAN.md` →
  Phase A (oracle build in WSL2/CI + dump patch + fixture corpus) for M0 exit.

## 2026-09-08 — M0 ✅ (Phase A complete)

- Done: M0 exit observation MET, observed. (1) C++ oracle builds NATIVELY on this Windows
  host (MSYS2 ucrt64: gcc 15.2 + pacman highway/lcms2/libpng/libjpeg-turbo) — plan's WSL2/CI
  assumption superseded (deviation below). (2) `#ifdef SSIMULACRA2_DUMPS` patch in
  src/ssimulacra2.{h,cc}: per-stage dumps (linear input, rg constants, xyb_pre/xyb, 5 blurred
  planes, per-pixel ssim-d + edge-d1 f64 maps, per-scale norms, weighted+final score,
  meta.txt). (3) Fixture corpus created + committed (`tests/fixtures/`, 26 PNGs, 3.1 MB,
  deterministic generator `oracle/gen_fixtures.py`): photo/grad/noise/step/odd, near-cutoff
  s8/s9/s12/s15, sub-cutoff s7 (CLI-reject), alpha, gray, big 2048², identical. (4) Goldens:
  1008 dump files × 2 runs, SHA-256 all equal (byte-reproducible); CLI scores span
  1.24..100.00 incl. exact-100 identity + s7 rejection. (5) Default-path invariance proven:
  14/14 scores identical pre/post patch AND asm diff = 5287 lines with exactly 2 differing
  instructions (JXL_CHECK `__LINE__` immediates 436→609, 438→611) — PE byte-identity is
  unattainable for any in-file patch (asserts embed line numbers); asm-identity is the
  achieved bar. Dump-build scores == default scores.
- Deviated from plan: (a) oracle builds native ucrt64, not WSL2/CI (better: no VM); (b) M0
  "byte-identical default binary" restated as asm-identical-except-__LINE__ (reason above,
  evidence in oracle/README.md); (c) `big` fixture is score-only (dumps would be >1.5 GB).
- Blocked / open question: none. Gray question CLOSED by dump evidence: gray inputs reach XYB
  as 3 replicated planes (ch=3). `SSIMULACRA2_VULKAN_PORT_PLAN.md` still unwritten — its
  content is now largely inlined in fable-plan §2; will write it as the citation trail during
  Phase B rather than as a gate.
- Next: Phase B — Rust workspace + clean-room ash context (serialize instance creation — AMD
  driver fact), staging, ×2 smoke shader (glslc 1.4.357.0, checked-in .spv), green on RX
  6600M locally; lavapipe leg deferred to CI (not available on this host, sibling precedent).

## 2026-09-08 — M1 ✅ (real-GPU leg; lavapipe deferred to CI)

- Done: Phase B complete. Rust workspace + `ssimulacra2-vulkan` crate (ash 0.38.0, clean-room
  context.rs/pipeline.rs — instance/device/queue, discrete-first selection, staging
  upload/readback, one-shot fence submits, per-call compute dispatch; serialized context
  creation per AMD driver fact). Smoke ×2 shader (glslc → checked-in .spv) element-exact on
  AMD Radeon RX 6600M incl. ±0/±inf/MAX÷4 bit-compare; 3 consecutive debug runs + 1 release
  run green; validation layer ACTIVE in debug and clean after fix. Validation caught a real
  bug pre-commit: device buffers from create_buffer_f32 lacked TRANSFER_SRC → readback copy
  was UB (VUID-vkCmdCopyBuffer-srcBuffer-00118); fixed by adding the usage flag.
- Deviated from plan: (1) gpu-allocator deferred — manual vkAllocateMemory is sufficient for
  planar SSBOs; revisit at Phase H if fragmentation measured (dependency-minimal per AGENTS 8).
  (2) ash 0.38 API rework (no builder(); ::default()+setters, SPIR-V as &[u32]) — learned from
  installed source, not memory. (3) NEW DRIVER FACT (recorded): AMD RDNA2 shaders FLUSH
  DENORMALS TO ZERO (1.4e-45×2→0); GLSL has no FTZ control. Data-range proof that denormals
  never occur (XYB ~0..1, kC2=0.0009 floor) must be re-asserted in Phase E range audit.
  (4) lavapipe leg deferred to CI (host lacks it; sibling precedent M1).
- Blocked / open question: none.
- Next: Phase C — xyb_positive.comp: opsin premul (assert vs dumped rg/premul constants),
  CubeRootAndAdd op-for-op transcription (bit-trick seed, 3 NR, final iter, r²x+add),
  clamp order, X/Y/B mix, MakePositiveXYB order; parity ≤1e-6 vs dumps/photo r0 xyb planes.

## 2026-09-08 — M2 ✅ + M3 ✅ (Phases C+D complete)

- Done: M2: xyb_positive.comp (opsin fma chain, ZeroIfNegative clamps, CubeRootAndAdd
  op-for-op, StoreXYB mix, MakePositiveXYB order) ≤4.8e-7 vs dumps on 4 fixtures × orig/dist ×
  pre/positive (bar 1e-6). M3: blur_h/blur_v. rg constants (all 60 f32 + radius) derived in
  Rust f64 = BIT-EXACT vs oracle dump. Rust transcription of FastGaussian1D's 3-phase form
  (lane-0 border / 4-output unrolled / remainder) + VerticalBlock naive form = BIT-EXACT vs
  dumps (cpu_form.rs, drift 0.0). GPU blur vs dumps ≤1.07e-6 (bar 2e-6); s8 bit-exact.
  Synthetic battery (7 sizes × 5 patterns incl. odd/near-cutoff) GPU vs Rust ≤1.5e-6.
  Full workspace suite green (6 tests).
- Deviated from plan: naive-form-only blur was insufficient (3.2e-6 > 2e-6 bar) → executed the
  plan's escalation: transcribed the CPU 4-output unrolled expansion (mul_prev/mul_prev2/mul_in
  via a 64-float rg SSBO). Residual: group-loop lanes drift ≤1 ulp on ~1% of pixels on RDNA2
  (border/remainder/vertical bit-exact; SPIR-V verified faithful mul+fma sequence — driver
  scheduling). Bounded, within ladder; revisit only if score parity demands.
- Blocked / open question: none.
- Next: Phase E — maps_combine.comp (ssim d f32-semantics + edge d1 f32) + fused multiply
  inputs; single-scale norms (CPU f64, source accumulation order) vs dumps ≤1e-6; measure the
  documented double→float edge deviation; range audit (no denormals in real data, FTZ-safe).

## 2026-09-08 — M4 ✅ (Phase E complete; XYB+blur now BIT-EXACT)

- Done: maps_combine.comp + maps.rs (f64 norms, source accumulation order). Single-scale
  norms vs dumps: ssim ≤9.1e-9, edge ≤1.9e-8 (bar 1e-6). All 5 blurred planes bit-exact vs
  dumps. Range audit: min positive 8.2e-2 — FTZ-safe confirmed on real data. Full suite
  green (7 tests). Two root-cause fixes found by chasing d-map drift (7.1e-4 → 0):
  (1) GLSL/driver mul+sub contraction (ACO folds fma(a,b,0)→mul then re-fuses): fixed with
  `precise` on mu11/mu22/mu12/dm/dmsq/num_m (maps) and X*14+0.42 (xyb) — CPU rounds products
  to f32 separately. (2) Rust f32::cbrt (UCRT libm) is 1 ulp off oracle's mingw cbrtf for
  kB0 (0xbe1fb276 vs 0xbe1fb275) — shifted 43% of XYB pixels; fixed via f64 cbrt→f32 cast.
  xyb_parity TIGHTENED to bit-exact assert (16/16 pass). blur group-loop zero-coefficient
  fma steps skipped (driver drift source; value-identical on strictly-positive data, guarded
  by range audit) → GPU blur bit-exact vs dumps.
- Deviated from plan: d-map per-pixel bar (2e-6) retained but actual drift now ~0 for ssim;
  edge retains documented double→float (~1e-8 at norms). The 1e-6 XYB bar superseded by
  bit-exactness (stronger).
- Blocked / open question: none. WATCH for Phase F: Rust f64 pow (score polynomial) vs
  mingw pow — same libm-divergence class; score dumps will catch it.
- Next: Phase F — downsample_box2.comp (clamp-replicate, /4, exact order) + per-scale loop
  (GPU XYB recompute, break rule, weight-index shift) + score.rs (108 weights codegen from
  source + checksum vs dumps) + end-to-end: all fixtures ≤1e-5 score, identity ==100.0.

## 2026-09-08 — M5 ✅ + M6 ✅ (Phase F complete)

- Done: Full 6-scale GPU pipeline (downsample_box2.comp bit-exact vs CPU order; mul_planes;
  per-scale XYB recompute; break rule; weight-index shift via c-major loop over scales.size()).
  score.rs: 108 weights codegen'd from src/ssimulacra2.cc by build.rs as exact bit patterns.
  e2e vs oracle dumps, 13 runs (10 fixtures + identical + alpha r0/r1): weighted drift
  ≤3.9e-9 (bar 1e-6), score drift ≤4.5e-8 (bar 1e-5), identity weighted drift EXACTLY 0 and
  score == 100.0 exactly. Near-cutoff s8/s9/s15 exercise the missing-scale weight shift.
  Key fix en route: GPU f32 fdiv measured 1 ulp off oracle on RDNA2 → amplified x225 by the
  large tuned weights (noise fixture score drift 6.2e-5 > bar) → quotient + edge ratio now
  computed in f64 in-shader (shaderFloat64 feature enabled at device creation after
  validation caught the missing enable, VUID-pCode-08740); double-rounding risk ~2^-29
  documented in shader header. Validation clean across full suite; 8 tests green.
- Deviated from plan: edge_d1 double->float deviation ELIMINATED by f64 in-shader ratio
  (plan §7 documented it as accepted; now bit-matched to CPU double then stored f32).
  f64 use is a correctness choice; Phase H may replace with correctly-rounded f32 division
  emulation if RDNA2 f64 rate (1/16) shows in profiles.
- Blocked / open question: none.
- Next: Phase G — cpu.rs fallback path (faithful Rust reimpl, validated vs dumps), PNG-decode
  CLI with --gpu, ICC reject message, .github/workflows/ci.yml (ubuntu+lavapipe+oracle),
  validation matrix green incl. gray/alpha/odd/small (M7) + CLI/CI/fallback (M8).
