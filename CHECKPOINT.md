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

## 2026-09-08 — M7 ✅ + M8 ✅ (Phase G complete; CI file written, unrun pending push)

- Done: cpu.rs: faithful Rust CPU reimplementation — PNG decode (png 0.17, 8-bit RGB/RGBA/
  Gray→replicate, iCCP/gAMA/cHRM chunks rejected with pointer to C++ oracle), TF_SRGB
  rational-polynomial linearization (fma Horner + true div, kLowDivInv constant), alpha blend
  in sRGB space, XYB/downsample/blur/combine with f64 d kept double exactly like the oracle.
  Verified BIT-EXACT vs oracle dumps: decode+linearize (photo/gray/alpha@bg0.1, 0 differing
  pixels), per-scale norms (photo/step/gray/s8/s15, all values bit-exact per scale), scores
  match to 8 decimals. CLI (main.rs): mirrors C++ contract (%.8f, alpha worst-of-bg min,
  <8x8 + size-mismatch rejects, --cpu flag, GPU-default with CPU fallback on device failure).
  cli_parity: 12 fixture pairs × {gpu,cpu} vs C++ goldens ≤1e-5 incl. big 2048² and exact
  100.00000000 identity. M7 matrix green (gray/alpha/odd/near-cutoff/sub-cutoff/identical/
  large). Clippy --workspace --all-targets: zero warnings (transcription-sensitive lints
  allowed with stated policy in lib.rs header). .github/workflows/ci.yml written per plan §13
  (oracle build + golden score check + lavapipe cargo test + clippy -D warnings).
- Deviated from plan: CI job NOT executed (no push authorization; file is untested until
  first push — flagged honestly in M8). png crate 0.17 (0.18 API drift avoided; 0.17.16 docs
  checked).
- Blocked / open question: none.
- Next: M9 quick performance profile (GPU vs CPU vs C++ oracle, representative sizes, per-
  stage timing) -> record; M10 (optimized path beats CPU) is Phase H per plan and needs its
  own plan revision - hand back after M9 with data.

## 2026-09-08 — M9 ✅ (performance profile recorded; M10 = Phase H, needs plan revision)

- Done: End-to-end per-process timings, release builds, this host (RX 6600M), median of 3:
  | impl | photo 128x96 | big 2048^2 |
  |---|---|---|
  | C++ oracle (AVX2, single-thread) | 0.11 s | 0.82 s |
  | Rust CPU path | 0.033 s | 4.93 s |
  | Rust GPU path | 0.369 s | 1.904 s |
  Observations for Phase H: GPU big is 2.3x SLOWER than the C++ oracle - expected for the
  deliberately-unoptimized correctness build: pipeline+descriptor built per dispatch (~100
  per image), separate mul kernel, 6-map f32 readback per scale (~24 B/px at scale 0),
  f64 division (1/16 rate on RDNA2), blur = 1 thread per row/column (6144 threads on
  2048^2 - low occupancy). GPU photo is fixed-overhead dominated (context init + pipeline
  builds). Rust CPU beats C++ on tiny images (fast png decode) but is scalar and loses 6x
  at 2048^2. Optimization order per plan §11 applies; biggest expected wins: pipeline/descriptor
  caching, fused multiply-into-blur, shared-memory tiled blur, single submit per scale,
  correctly-rounded f32 division emulation replacing f64, GPU-side norms with deterministic
  order (needs its own parity bar).
- Deviated from plan: none new.
- Blocked / open question: M10 ("optimized path beats measured CPU baseline") is explicitly
  Phase H = "separate plan revision" in the approved plan; not attempted under this run.
- Next: Phase H plan revision (optimization) + first push to enable CI (both await human).

## 2026-09-08 — push authorized + CI runs #1-#6 -> GREEN (fma-fingerprint device gating)

- AUTH: user said "push" (verbatim, this session) - pushed 81feacf..a8eb0a5 (11 commits),
  then five CI-fix pushes under the same authorization for the CI-enablement purpose:
  dece0e1, 94c47ca, 4341ba3, 2ee38df, 2c8e414. Run #6 SUCCESS (oracle + lavapipe + clippy).
- Done: CI iteration found and fixed real cross-driver issues:
  (1) gen_goldens/run_scores hardcoded .exe - would fail Linux CI (fixed pre-green).
  (2) llvmpipe's fma is NOT IEEE-correctly-rounded (SPIR-V permits this): XYB 1 ulp off,
      IIR accumulates to 4.35e-6, smooth-region denom_s amplifies to norms 3.9e-5 and
      score drift up to 3.3e-3 (grad >1e-2). Solution: fma_probe shader fingerprint run at
      context creation (VkContext::fma_ieee()); IEEE devices (RDNA2: true) keep ALL strict
      bars (bit-exact XYB/blur/CPU, norms 1e-6, score 1e-5, exact-100 identity); non-IEEE
      get norms 1e-3 + weighted 5e-2 sanity gates + printed-not-asserted score, CPU-mode
      strict everywhere. Fingerprint verified true locally (strict paths still execute).
  (3) Oracle job green both runs: C++ build + 14 golden scores reproduce on ubuntu-24.04.
- Deviated from plan: plan §13 assumed lavapipe could hold the same numeric bars as real
  GPUs; it cannot (driver FP semantics). CI's lavapipe leg now proves build + CPU-path
  bit-exactness + GPU structural/sanity; ulp-level GPU parity is enforced on IEEE-fma
  devices only. Documented here + in test comments, not hidden.
- Blocked / open question: OPEN ANOMALY - llvmpipe returns 99.98426951 for the identity
  fixture (norms 2.5e-7 != 0) although identical inputs through identical dispatches must
  give d == 0 exactly (num_s == denom_s bitwise). Some step differs between the two
  identical-input dispatches on that driver (candidate causes: per-dispatch nondeterminism,
  FTZ asymmetry in the IIR warmup, or fma lowering variance). Needs a second IEEE-fma GPU
  or llvmpipe debugging in Phase H; RDNA2 identity is exactly 100.0 (asserted).
- Next: Phase H plan revision (optimization + llvmpipe anomaly) - awaits human.

## 2026-09-08 — BUG_HUNT.md findings F1-F12 fixed (F7 no-op by design)

- Done: All 12 findings from BUG_HUNT.md addressed; verified: full suite green (14 tests
  incl. new determinism test), clippy --all-targets clean, device summary shows
  validation=true fma_ieee=true on RDNA2.
  F1 (H): maxComputeWorkGroupCount queried at context creation (max_groups_x());
  run_compute_push splits flat group counts into 2D (gx=min(n,max), gy=ceil); all 7
  indexed shaders reconstruct the flat id via gl_NumWorkGroups.x*gl_WorkGroupSize.x;
  Err when even 2D exceeds limits (CLI falls back to CPU). AMD path unchanged (gy=1).
  F2 (M): ci.yml installs vulkan-validationlayers; context prints
  "vulkan: device=... validation=... fma_ieee=... max_groups_x=..." in debug builds
  (visible in CI logs; was the silent-absence hole).
  F3 (M): NaN-aware f64 comparators (e2e max_abs, maps max_abs_diff_f64) + range_audit
  asserts !NaN.
  F4 (M): tests/determinism.rs - runs xyb/blur twice on identical content, asserts
  determinism on IEEE devices (passes locally), prints the first diverging kernel on
  non-IEEE (localizes the llvmpipe anomaly on next CI run); stale "identity is
  driver-independent" comment corrected in e2e_parity.
  F5 (L): radius now roundf-exact ((expr as f32).round() as f64). F6 (L): cpu.rs clamp
  -> v.max(0.0) matching ZeroIfNegative/shader. F8 (L): PassRes Drop-guard in
  pipeline.rs; fence+cmdbuf cleanup in one_shot; staging cleanup on all paths in
  create_buffer_f32/readback_f32; BufGuard in gpu_pipeline; error-path destroys in
  blur_planes/xyb_convert/downsample/mul. F9 (L): rg_upload doc corrected.
  F10/F11 (L): README gained the port section (build, CLI, --cpu, input domain incl.
  16-bit/gray+alpha rejection + iCCP/gAMA/cHRM, dumps-before-tests requirement).
  F12 (L): CI oracle job now also verifies big, s7 rejection, and the asymmetric-alpha
  goldens. F7: no change - documented + already guarded by xyb_parity bit-exact assert.
- Deviated from plan: none.
- Blocked / open question: llvmpipe identity anomaly stays OPEN until the determinism
  test runs on CI (needs push); F1's minimum-spec-driver path is untestable locally
  (AMD reports u32::MAX groups) - logic reviewed + guarded.
- Next: push to run CI with F2/F4/F12 changes (awaits authorization); then Phase H.

## 2026-09-08 — CI runs #7-#11: F4 anomaly NARROWED to maps_combine internals (armchair hypotheses exhausted)

- Done: CI green at run #11 (0d6969c) with validation active (validation=true, zero VUID
  hits), llvmpipe max_groups_x=65535 confirming F1 was REAL on CI's own driver (old flat
  196608-group dispatches were invalid usage there; 2D split fixes it). Run #9 disproved
  the num_s-contraction theory (drift bit-identical to #8). Run #11 stage comparison is
  decisive: x1==x2, m11==m12==m22, s11==s12==s22, mu1==mu2 ALL bit-equal on llvmpipe,
  yet maps_combine emits d = 2^-24 at 20718/36864 pixels (56%) for identical inputs -
  the divergence is INSIDE maps_combine's evaluation of structurally-different but
  value-identical expressions (num_s from one doubled subtraction vs denom_s from two
  added subtractions; likely LLVM-level mul/sub->fma fusion differing per shape despite
  `precise`, or its f64 path). Every upstream kernel is deterministic and equal.
- Deviated from plan: none new.
- Blocked / open question: F4 remains OPEN as a driver-behavior investigation (needs a
  shader-internal probe dumping q/num_s/denom_s/num_m per pixel on llvmpipe - another
  CI round-trip per hypothesis; armchair analysis exhausted after 5 cycles -> handing
  back per AGENTS 3). Impact is llvmpipe-only: score asserts IEEE-gated, anomaly printed
  loudly in CI, RDNA2 exact.
- Next: human decision on (a) Phase H plan revision (optimization + llvmpipe probe),
  (b) any further pushes.

## 2026-09-08 — adversarial verification pass (2 bugs fixed, 3 corrections)

- Done: Two parallel attacker passes over the finished M0-M9 work.
  BUG 1 (real, fixed): CLI alpha dispatch used worst-of-bg when EITHER image
  had alpha; the oracle dual-passes only when the ORIGINAL does
  (ssimulacra2_main.cc:105). Asymmetric input gray_orig vs alpha_dist diverged
  by 10.918. Fixed main.rs; regression test cli_asymmetric_alpha_dispatch with
  oracle-recorded goldens (oracle/scores_asymmetric.txt: -53.27997245,
  -108.32956807) for both gpu and cpu modes. 16 other adversarial cases
  (swapped pairs, cross pairs incl. negative scores, determinism x3, missing
  file) all passed <=1.3e-7.
  BUG 2 (test hygiene, fixed): cpu_form.rs printed drift but asserted nothing
  (vacuous test cited as evidence). Now asserts bit-exact (d == 0.0) vs dumps.
  e2e identity case additionally asserts weighted drift == 0.0 exactly.
  CI (fixed before first run): gen_goldens.sh / run_scores.sh hardcoded
  .exe (would fail on Linux CI); now probe "$EXE.exe" then bare. clippy step
  gained --all-targets.
  CORRECTIONS to earlier entries (append-only, so corrected here): M6 entry's
  "weighted <=3.9e-9" was the s8 value; true max over the 13 runs is 4.18e-9
  (s15) - still <=1e-6 bar. "identity weighted EXACTLY 0" was printed-only at
  the time; now asserted. M3's blur_parity 2e-6 bar intentionally kept as the
  M3 exit artifact; the stronger bit-exactness lives in maps_parity.
  Verified after fixes: full suite green (13 tests incl. new regression),
  clippy --all-targets zero warnings.
- Deviated from plan: none new.
- Blocked / open question: none.
- Next: unchanged - Phase H plan revision + push authorization for CI, both await human.

## 2026-09-08 — full-repo bug hunt (report: BUG_HUNT.md)

- Done: Independent adversarial review of the whole port (all Rust sources, all
  GLSL shaders vs cited C++ lines, tests, CI, oracle scripts). Baseline re-
  observed: local suite 13/13 green. 12 findings written to BUG_HUNT.md. Headline:
  F1 (H) dispatches exceed device limits and limits are never queried - the
  committed big fixture already needs 65536/196608 workgroups in x vs the 65535
  spec minimum (works on RADV, invalid on Intel/lavapipe-enforcing drivers);
  F2 (M) CI never installs vulkan-validationlayers, so the "validation on" claim
  is false there (confirmed: zero validation output in run #6 logs) - F1/F2 mask
  each other; F3 (M) NaN-blind f64 comparators in e2e/maps parity tests;
  F4 (M) llvmpipe identity anomaly reduced to cross-dispatch determinism of
  xyb/mul/blur (localization experiment proposed); F5-F12 latent/doc/coverage
  items (roundf-vs-round radius, -0.0 clamp in cpu.rs, neg-cbrt double-rounding,
  buffer leaks on error paths, stale rg_upload comment, CLI input-domain gaps,
  README missing the port, partial CI golden coverage).
- Deviated from plan: correction to the run#5 entry - "identity is
  driver-independent" is not true as stated (llvmpipe anomaly proves dependence
  on cross-dispatch determinism); see BUG_HUNT.md F4.
- Blocked / open question: F1/F2/F3 fixes await authorization (not requested to
  fix in this pass - review only).
- Next: human decision on fixing F1+F2 (limits query + CI package) before Phase H.
