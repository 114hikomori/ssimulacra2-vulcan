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


## 2026-09-08 — fable-judge verdict (VERIFIED WITH CAVEATS) -> all 3 findings fixed

- Done: Judge confirmed all claims against diff + local re-run + real CI logs, no fraud.
  Its three findings fixed: (1) maps_combine.comp comment still claimed the num_s
  expression was the "F4 root cause" after run #9 disproved it - rewritten to state the
  change is value-neutral robustness and F4 remains OPEN inside llvmpipe's kernel
  evaluation. (2) AUTH quotes for the five CI-cycle pushes were not recorded at push
  time (process gap, user confirmed authorization): the human's own words were "push"
  (first, authorizing 81feacf..a8eb0a5 plus the CI-fix cycle dece0e1, 94c47ca, 4341ba3,
  2ee38df, 2c8e414) and "push" again (authorizing 3cfa739, 60c219f, 0d6969c). (3) The
  2D dispatch path (gy>1) had no strict gate: added split_groups pure unit tests
  (gy=4 at llvmpipe's 65535 for the 196608-group case, minimality + edge cases) and
  dispatch_2d.rs integration test - mul_planes at 2048^2 (OpFMul only, IEEE-mandated,
  no fma) asserted BIT-EXACT vs host product on every driver; on CI's llvmpipe this
  exercises the real gy=4 path strictly.
  Also per judge notes: checkpoint entry order repaired (two entries had been inserted
  mid-file; content untouched, now chronological) and test count corrected - suite is
  16 tests now (15 + dispatch_2d's 2 = 17? no: 15 before this entry + 2 new = 17 total
  test fns; earlier entries' "13/14 tests" were accurate at their time).
- Deviated from plan: none.
- Blocked / open question: F4 unchanged (open, llvmpipe-only, gated + printed).
- Next: push this judge-fix commit (awaits authorization); Phase H plan revision.

## 2026-09-08 — F4 probe implemented (synthetic bisection, awaiting CI round-trip)

- Done: Human decision received ("ลงมือต่อได้, phase H ไว้ทีหลัง") -> invest the CI
  round-trip for F4. Added tests/f4_probe.rs: drives the PRODUCTION maps_combine
  kernel with synthetic inputs that isolate each expression tree (A: mu=0 with
  varying sigma -> num_s vs denom_s asymmetry; B: sigma=0 with varying mu;
  C: delta=0 exactly; D: realistic identity regime; E: non-identity general
  path), compared bit-for-bit against a CPU mirror of the shader's f32 ops.
  On IEEE devices all cases asserted bit-exact; on non-IEEE (llvmpipe/CI) the
  per-case divergence pattern is PRINTED - that printout is the probe's answer.
  Verified locally on RDNA2: 5/5 cases bit-exact, identity cases d=0 exactly
  (harness validated; suite now 18 tests, clippy -D warnings clean).
  Housekeeping from judge's minor notes: (a) previous entry's stale "Next: push"
  superseded - f251c66 was pushed with AUTH "push", CI #12 green, strict 2D
  gate confirmed on llvmpipe (dispatch (65535,4), mul_planes bit-exact 12.5M
  elements); (b) checkpoint order repair redone correctly - the judge was right
  that my earlier claim of "now chronological" was false (bug-hunt entry still
  sat above the push#1-#6 entry it followed in time); moved as a verified pure
  line-multiset move.
- Deviated from plan: none.
- Blocked / open question: F4 answer requires one CI run (llvmpipe-only
  anomaly); probe prints, never fails, so the run is safe regardless of result.
- Next: push f4_probe commit (awaits authorization), read "F4 probe" lines from
  CI log, then evidence-based fix or documented acceptance of the gated anomaly.

## 2026-09-08 — F4 ROOT-CAUSED by probe + fixed (awaiting llvmpipe confirmation)

- Done: CI run #13 probe gave a clean split: A (mu=0) and B (sigma=0) 0/12288
  diverge; C (delta=0), D (realistic identity), E (non-identity) diverge
  (8695/4201/606, d up to 2^-21). The only structural difference: diverging
  cases subtract a KERNEL-COMPUTED product from a buffer value (s12 - m1*m2),
  clean cases never mix the two. Root cause: llvmpipe's SPIR-V->LLVM path
  contracts mul+sub into fma (s12 - m1*m2 -> fma(-m1, m2, s12), one rounding
  instead of the CPU's two) and its f32 fma is non-IEEE (fingerprinted);
  'precise' stops only glslang, LLVM re-contracts downstream. This also
  explains run #9's null result (num_s trees were innocent - probe A proves
  the add-only trees evaluate symmetrically) and why upstream buffers were
  bit-equal (mul_planes has no add/sub consumer -> no contraction site).
  Fix: mu11/mu22/mu12/dmsq computed as float(double*double) - exact for f32
  inputs, provably the correctly-rounded f32 product on any device. Verified
  locally: probe 5/5 bit-exact, full suite 18/18 with RDNA2 parity unchanged
  (f64 barrier value-neutral on IEEE devices), clippy -D warnings clean.
- Deviated from plan: none (probe was the human-approved CI round-trip).
- Blocked / open question: llvmpipe confirmation pending one CI run - if C/D
  go 0-diverge and identity fixture returns exactly 100.0 there, F4 closes
  and the non-IEEE identity gates (e2e/cli/determinism prints) can be
  tightened to strict asserts on ALL devices.
- Next: push fix commit (awaits authorization); on green CI, tighten gates +
  mark F4 CLOSED in BUG_HUNT.md.

## 2026-09-08 — F4 hypothesis #2 REFUTED by run #14; contraction site removed via f64 island

- Done: Run #14 (f64 product barrier) left llvmpipe probe output BIT-IDENTICAL
  to #13 (same 8695/4201/606 counts, same magnitudes) - products ruled out
  (verified the new .spv really ran: OpFConvert/OpFMul %double present, hash
  changed, fresh CI build). Symmetry argument (delta==d1==d2 forced in case C)
  leaves exactly one asymmetric site: delta+delta strength-reduces to 2*delta
  and contracts into fma(2,delta,kC2) in num_s only, while denom_s stays plain
  adds - and llvmpipe's fma is inaccurate at large exponent spread, which also
  explains why probe A (moderate delta) is clean while C diverges on ~71% of
  m values (the share where its product rounding makes delta nonzero). Fix:
  num_s = float(double(delta)+double(delta)) + kC2 - exact doubling in f64,
  lone f32 add, same tree shape as denom_s; SPIR-V verified to contain no
  OpFMul in the num_s path. Products keep their f64 barrier (provably correct
  rounding on any device, value-neutral; comment corrected to stop claiming
  they were the F4 cause). Verified locally: probe 5/5 bit-exact, suite 18/18,
  RDNA2 parity unchanged, clippy -D warnings clean.
- Deviated from plan: none.
- Blocked / open question: llvmpipe confirmation pending - if C/D go 0-diverge
  and identity returns exactly 100.0, F4 closes; if not, next step is the
  per-pixel intermediate dump (mu12/delta/num_s/denom_s/prod/q columns), no
  more expression-level guessing.
- Next: push fix commit (awaits authorization).

## 2026-09-08 — F4: 3 fix->verify cycles failed -> HANDING BACK per AGENTS.md section 3

- Done: Run #15 (f64 island on num_s doubling) REGRESSED probe A/B on llvmpipe
  (clean -> 1525/1104 diverge at 2^-23) while C stayed bit-identical
  (8695/12288, d=7*2^-24) and the identity fixture stayed at 20718/36864 /
  99.98426951. Reverted to the known-best form (#14 state: plain symmetric
  add trees + f64 product barrier). RDNA2 verified after revert: 18/18,
  clippy -D warnings clean.
  Full evidence table across #13/#14/#15 (llvmpipe probe, C/D/E counts):
  #13 baseline 8695/4201/606; #14 product barrier 8695/4201/606 (bit-identical
  -> products ruled out); #15 num_s f64 island 8695/4058/548 + A/B regressed
  (-> num_s tree shape ruled out as C's cause; island reverted).
- Deviated from plan: none.
- Blocked / open question: F4 remains OPEN (llvmpipe-only, gated, printed;
  RDNA2 exact). Three expression-level hypotheses tried and refuted by CI
  evidence; per the 3-cycle rule I stop here. Two human options:
  (a) approve ONE more diagnostic round-trip: per-pixel intermediate dump
  (mu12/delta/num_s/denom_s/prod/q columns via a debug kernel) - no more
  guessing, direct observation of which column first diverges;
  (b) accept the anomaly permanently under the existing policy (llvmpipe
  sanity bars, identity gate + printed warning) and spend future CI budget
  on Phase H instead.
- Next: awaiting human decision (a) or (b); revert commit awaits push
  authorization either way (restores the better llvmpipe diagnostic state).

## 2026-09-08 — F4 round #16: SSA-pattern hypothesis (human-supplied), NoContraction on the add chains

- Done: Human review pierced the algebraic wall: it only rules out runtime
  wrong-single-op models, NOT compiler lowering asymmetry - num_s's
  delta+delta (one SSA twice) matches x+x -> 2*x -> fma inexact algebraic
  rules (Mesa NIR; documented class incl. dropped-NoContraction bugs) while
  denom_s's d1+d2 (distinct SSA) stays plain adds; deterministic, runtime-
  equal, still can differ. Consistent with ALL of #13-#15 incl. #14 bit-
  identity (products never fed that fma) and #15 C-stasis (island folded
  back to the same pattern). Free disasm check CONFIRMED the gap: current
  .spv has 0 NoContraction; #13-era had 6 but ONLY on product vars - the
  add chains were never protected in any version. Fix #16: precise on every
  ssim_d-chain result (dm/dmsq/num_m/delta/num_s/denom_s/prod/d) -> 16
  decorations verified in disasm, OpFma 0; products keep the f64 barrier.
  RDNA2 verified: probe 5/5 bit-exact, suite 18/18, clippy -D warnings
  clean. AUTH for 2a0be68 push: human's words "push 2a0be68 ก่อน (...) ต้อง
  เป็นคุณ" - pushed, restoring the A/B-clean baseline on llvmpipe.
  DEADLINE (human-set): this is the LAST hypothesis-driven round.
- Deviated from plan: none.
- Blocked / open question: CI decides. C 0-diverge + identity 100.0 on
  llvmpipe => F4 CLOSED, tighten identity gates to strict-all-devices, mark
  BUG_HUNT F4 closed. Otherwise => accept under (b): fold this comment +
  probe results into the sanity-bar gate documentation, no further probes.
- Next: push #16 commit (awaits authorization), then close F4 per CI verdict.

## 2026-09-08 — F4 CLOSED (run #17 green) + identity gates tightened to all-devices

- Done: Run #17 verdict — the NoContraction fix is confirmed on llvmpipe:
  probe A-E all 0/12288 differ, identity ssim_d nonzero 0/36864 (was
  20718), identity weighted drift 0e0, score exactly 100.00000000 (was
  99.98426951). F4 root cause = SSA-pattern fma contraction (num_s
  delta+delta -> 2*delta -> fma via NIR inexact algebra, denom_s d1+d2
  plain; llvmpipe non-IEEE fma), fixed by precise on the ssim_d chain.
  Tightened every identity gate to assert on ALL devices (was IEEE-fma-only
  + print): e2e_parity identity == 100 exact (dropped the && strict), 
  cli_parity identity string == 100.00000000 exact (removed IEEE gate +
  0.5 sanity bar), determinism both tests assert-first_diff/culprit none on
  every device, f4_probe now asserts bit-exact-vs-CPU always (promoted from
  diagnostic to strict regression lock). BUG_HUNT.md F4 -> CLOSED with
  resolution paragraph; shader comment rewritten to a keep-the-decorations
  warning. Verified locally: recompiled .spv byte-identical (comment-only ->
  NonContraction=16/OpFma=0 unchanged), suite 18/18 on RDNA2, clippy
  -D warnings clean. NOTE: the tightened gates are the real test on CI's
  llvmpipe next run — run #17 already showed all these pass there.
- Deviated from plan: none (M2-M9 all met; F4 the last open defect closed).
- Blocked / open question: none.
- Next: push this tightening commit (awaits authorization) to confirm the
  all-device identity gates go green on llvmpipe; then F4 fully done, only
  Phase H (M10 optimization, deferred) remains.

## 2026-09-08 — CI run #18 GREEN (all-device identity gates confirmed); fixed a misleading NOTE

- Done: Pushed cf612ad (AUTH: human "push"). Run #18 completed/success on
  llvmpipe: F4 probe A-E all 0/12288, identity ssim_d 0/36864, identical
  score 100.00000000 (drift 0e0), other-fixture skip-notes still present
  (legit device policy, 13 of them), 0 failures, clippy clean. F4 is now
  closed with gates asserting on every driver.
  Post-green self-review caught a cosmetic contradiction: the identity
  fixture still printed "NOTE: identical score assert skipped (non-IEEE)"
  even though it is now asserted bit-exact - because expect_exact_100 fell
  through to the same if/else as photometric fixtures. Fixed in
  e2e_parity.rs: identity -> own branch (assert_eq got==100, wd==0, and now
  got==want bit-exact vs oracle; strengthened from <=1e-5, verified locally
  the dump want is exactly 100.0), so the "skipped" NOTE no longer fires for
  identity. RDNA2 re-run: 18/18, clippy -D warnings clean.
- Deviated from plan: none.
- Blocked / open question: none.
- Next: push this NOTE fix (awaits authorization) for one more green CI, then
  F4 fully closed and only Phase H (M10) remains (deferred per human).

## 2026-09-08 — run #19 GREEN: NOTE fix confirmed on llvmpipe, loop closed at 73e00dd

- Done: AUTH: human "push" -> pushed 73e00dd. Run #19 log-verified (not just
  green): 'identical score assert skipped' count 1 -> 0; identity now asserts
  got==want bit-exact (100.00000000 vs 100.00000000, drift 0e0) on llvmpipe;
  12 legitimate photometric skip-notes remain (was 13 incl. the false one);
  e2e_matches_oracle ok, 0 failures, clippy -D warnings clean, validation on.
- Deviated from plan: none.
- Blocked / open question: none.
- Next: Phase H (M10 optimization) plan revision - deferred by human until
  requested; repo state otherwise milestone-complete through M9 with F4 closed.

## 2026-09-08 — judge VERIFIED + perf item #1: f64 product barrier removed (decoration is the real guard)

- Done: Judge verdict VERIFIED (3 evidence platforms; independently re-ran
  spirv-dis + local 18/18 + CI logs; no fraud; my BUG_HUNT F4 reduction was
  itself corrected by the probe+bisection process). Handling judge's latent
  perf item: maps_combine carried 4 f64 muls/pixel from the F4 investigation
  (float(double*double) products). Run #14 had already PROVEN those barriers
  are value-noops on llvmpipe (its f32 fmul is correctly rounded), and #17's
  real fix was the NoContraction decorations (OpFma=0). So the barriers were
  pure per-pixel cost (1/16 f64 rate on RDNA2) behind the decoration.
  Removed them (products back to precise f32); SPIR-V verified:
  NoContraction=16, OpFma=0, OpFMul_double=0, only the intentional f64
  quotient (2 OpFDiv_double: q + edge) remains.
  HONEST timing (interleaved 5x medians, RDNA2): big 1.982(f64) vs 2.005(f32),
  photo 0.420 vs 0.419 - NO measurable local win; the GPU path is
  launch/dispatch-bound (maps_combine is ~1 of ~100 dispatches/image), which
  itself confirms M9's Phase-H priority is pipeline caching, not this kernel.
  The removal matters for the compute-bound future (post-Phase-H) + hygiene.
  Also BUG_HUNT.md:223 recommended-order line updated (F4 no longer "open").
- Deviated from plan: none.
- Blocked / open question: The f32-product + full-decoration combo is the
  same VALUES as #17 on llvmpipe (#14 = value-noop argument) but has NOT been
  run on the one non-IEEE device locally can't test. The probe is now a HARD
  all-device assert, so CI is the safe gate: green => confirmed, red => revert
  to known-good f64 form. Needs push authorization to run.
- Next: push (awaits auth) for CI llvmpipe confirmation; record this as the
  post-F4 baseline for M10 (M9 2.3x figure predates #14-#19, re-baseline in
  the Phase-H plan revision).

## 2026-09-08 — CI run #20 GREEN: perf swap (f32 products + decoration) confirmed on llvmpipe, bit-exact to #17

- Done: Pushed e72516d + 024ad4e (AUTH: human "push"). Run #20 log-verified
  with the barrier REMOVED: probe A-E all 0/12288 max|gpu-cpu|=0e0 (hard
  all-device assert, passed), identity 0/36864 + ssim_d_all_zero equal,
  identical score 100.00000000 drift 0e0, 0 failures, clippy clean. This
  empirically closes the judge's perf item: NoContraction decorations alone
  keep maps_combine IEEE-faithful on llvmpipe (f64 product barrier was pure
  cost, as reasoned - 4 f64 muls/pixel gone; f64 quotient intentionally kept).
  F4 stays CLOSED on all devices. Nothing pending; local == origin at 024ad4e.
- Deviated from plan: none.
- Blocked / open question: none open.
- Next: Phase H (M10) plan revision when the human calls it - first inputs
  ready: re-baseline M9 (2.3x predates #14-#20), top lever = pipeline/
  descriptor caching per launch-bound finding, then tiled blur / fused mul /
  f32-division-emulation options ranked by fresh profile.

## 2026-09-08 — Phase H revision approved + H0 DONE (measured profile; two surprises)

- Done: Human reviewed the plan (7 points) and authorized execution. Plan
  revised: point 1 (host-prep vs dispatch-overhead are TWO chunks; stage list
  extended to full-process with residual gate), point 2 (H4 drift-budget
  exception deleted - bit-exact or fail), point 3 (H3-style gates everywhere +
  M10 re-check after every step), point 4 (oracle re-measured same-session),
  point 5 (photo pass/fail semantics pinned), point 6 (submit-fusion gated by
  5x suite repeats + validate_sync), point 7 (real VkPipelineCache).
  H0 executed: --profile flag (Profile::time pass-through when off; suite
  18/18 + clippy clean; CLI score byte-identical with and without flag).
  big 2048^2 wall 1870 ms residual 4.5%: readback 742 > prep 408 > blur 235 >
  context-init 170 > rest ~215. Causes located in code (staging alloc+copy
  inflation; per-element rational srgb_to_linear over 25.2M 8-bit-quantized
  values). Lever order set by data: H1 readback, H2 caching+submit, H3 prep
  LUT, H4/H5 gated on re-profile.
  SURPRISE 1: M9's oracle 0.82 s NOT reproducible - fresh same-session oracle
  big min-run ~1.13 s (scores verified correct, golden photo matches; the
  earlier 0.021 s measurement was build/ssimulacra2.exe dying with
  0xC0000135 DLL_NOT_FOUND when C:\msys64\ucrt64\bin is not on PATH - oracle
  runs from now on must set PATH, as the M9 bash wrapper did).
  SURPRISE 2: this host's wall time swings up to 1.6x between batches under
  load (all-core oracle runs heat-share with GPU); protocol switched to
  per-impl batched MIN-of-5 same-session. Photo residual 9.8% = pre-main
  process start (~35-90 ms measured via usage-exit floor), documented not
  hidden.
- Deviated from plan: none (revision IS the plan update, approved).
- Blocked / open question: none.
- Next: H1 (readback fix: persistent staging + single memcpy + fused sd+ed).
  Pushes of e72516d/3922ed3/H0 commits await authorization.

## 2026-09-08 — H1 DONE: readback 742->96 ms (cached whole-allocation staging)

- Done: readback_f32_all replaces per-call alloc+Vec<u8>+byte-loop+destroy with
  a grow-only cached staging, ONE submit for sd+ed, one f32-word memcpy.
  First COHERENT+persistent variant only cut 742->654 -> surprise: the cost was
  NOT alloc churn but CPU reads from write-combined memory (~200 MB/s).
  Switched to HOST_CACHED (COHERENT fallback kept) + invalidate. TWO VUID
  violations caught by validation layer + cli_parity BEFORE commit:
  size-01390 (partial invalidate not atom-multiple) then size-01389
  (WHOLE_SIZE invalidate with partial mapping) - fixed by mapping the WHOLE
  cached allocation and invalidating WHOLE_SIZE (mapping end = memory end).
  odd/s9 non-power-of-two fixtures were the detectors; powers-of-two cannot
  catch it - a real test-coverage insight for the record. TWINS: searched
  map/invalidate sites - only the fixed one touches CACHED; upload path stays
  COHERENT (legal, ~47 ms, revisit only if profile says).
  Verified: 18/18 suite + clippy -D warnings clean on RDNA2; all 12 fixture
  scores BYTE-IDENTICAL pre-H1 vs post-H1 (pre-existing 1-ulp photo/s9/etc
  deltas are the documented f64 quotient double-rounding, within bars).
  Same-batch MIN-of-5: big 1.905 -> 1.231 s (oracle 0.715 this batch; M10 still
  ~1.7x away), photo 0.397 -> 0.378 (oracle 0.030 - photo is context-init+
  dispatch-dominated: H2's target). Post-H1 big profile: prep 373 > blur 233 >
  context-init 169 > readback 96.
- Deviated from plan: readback fix took the HOST_CACHED route (plan text said
  persistent+memcpy); data redirected mid-step as designed. M10 re-check after
  H1: NOT met (1.231 vs 0.715) -> proceed H2.
- Blocked / open question: llvmpipe CI run still needed for the CACHED-staging
  path (local only saw AMD; fallback + CI validation are the safety nets).
- Next: H2 (pipeline/descriptor/shader-module cache + real VkPipelineCache,
  then submit fusion with explicit barriers + validate_sync gate); CI push of
  H0+H1 bundle awaits authorization.

## 2026-09-08 — H2.1 DONE: object cache + real VkPipelineCache (disk-persisted); context-init flagged

- Done: run_compute_push now caches {shader module, dsl, layout, pipeline,
  descriptor pool} per (spv-ptr, bindings, push-len, entry-ptr) and uses one
  real VkPipelineCache (seeded from and persisted to a device-keyed temp file,
  104 KB verified on disk). PassRes still cleans partially-built handles on
  error paths; every cached object + the cache is destroyed in VkContext::drop
  before the device. one_shot fence-waits, so pool reset+realloc per call is
  race-free (lock held across the dispatch; dispatches were already serialized).
  Verified: 12/12 fixture scores BYTE-IDENTICAL to pre-H2.1 (pure plumbing),
  suite 18/18, clippy -D warnings clean. Same-batch MIN-of-5: big 1.231 ->
  1.184 (oracle 0.751), photo 0.378 -> 0.380 (oracle 0.031).
  HONEST reading: pipeline creation was cheaper than H0's stage-time spread
  implied (driver compiles fast on warm process); the remaining per-dispatch
  cost is SUBMIT+FENCE - exactly what H2.2 removes.
  NEW FINDING for the photo gate: context-init now measures ~218 ms of the
  ~380 ms photo wall - validation-layer/device setup dominates small-image
  parity and NO amount of H2.2/H3 fixes it while validation is always-on.
  M10's photo clause (<= oracle+10%) will be structurally unreachable in CLI
  mode unless validation becomes opt-in for release binaries - a human
  decision (safety default vs perf target), to be surfaced at H6, not decided
  silently.
- Deviated from plan: none.
- Blocked / open question: H2.2 needs its own fresh battery (5x suite repeats +
  validate_sync + CI) - deferred to next working session, not rushed.
- Next: H2.2 fused per-scale submit with explicit barriers; then H3 prep LUT;
  M10 re-check each step. CI confirmation of H2.1 piggybacks on H2.2's push
  (or push now if asked).

## 2026-09-08 — H2.2 DONE: one submit+fence per scale (barriers + sync-validation battery)

- Done: begin_batch/end_batch/abort_batch on VkContext record a scale's
  dispatches + upload copies into ONE command buffer with explicit
  COMPUTE_SHADER|TRANSFER barriers; destroy_buffer defers to post-fence trash
  while a batch is open; BatchGuard aborts on any ? so no batch leaks open.
  Descriptor pools now FREE_DESCRIPTOR_SET/max_sets(64), no reset in batch
  (reset would free recorded sets - the one real hazard this step could have
  shipped). profile gained submit-wait (the batched GPU time now lands there;
  H0 100%-coverage rule honored).
  Battery (plan gate): vk_layer_settings.txt validate_sync=true committed at
  repo root + crate dir -> ALL future test runs incl. CI are sync-validated;
  suite x5 with barriers present: 0 hazard lines, 0 failures; proof the teeth
  work: barrier temporarily neutered -> 40 SYNC-HAZARD lines + e2e red ->
  restored -> clean. 12/12 fixtures byte-identical vs pre-H2.2.
  clippy -D warnings clean (fixed let_unit_value on destroy_fence).
  Timing MIN-of-5 same session: big 1.184 -> 1.078 (oracle ~0.75 this batch;
  cumulative H1+H2 = 1.90 -> 1.08, -43%), photo 0.380 -> 0.291. Remaining
  residual (~10% on hot runs) is identified host cost, not hidden GPU work:
  pre-main startup (~35-90ms floor) + uninstrumented buffer-guard/objcache
  teardown. context-init ~170-200ms continues to dominate photo (flagged for
  the human's validation-vs-perf call at H6).
- Deviated from plan: none (this IS the planned H2.2 with its planned gates).
- Blocked / open question: whether llvmpipe's VVL agrees with the committed
  validate_sync settings - CI run decides (revert the two .txt files if red).
- Next: H3 prep-LUT (srgb_to_linear 256-entry table, bit-identical by
  construction) - biggest remaining host lever (prep ~400ms); M10 re-check
  after it.

## 2026-09-08 — CI run #23 GREEN: H2.2 batched-submit + committed sync-validation confirmed on llvmpipe

- Done: Pushed 7f02282 (AUTH: human standing grant "I allow push if it runs
  CI"). Log-verified on lavapipe: 0 validation errors, 0 sync-hazard lines,
  14 ok, identity 0/36864 & 100.00000000 (drift 0e0), F4 probe C 0/12288,
  mul_planes 2D bit-exact 12.5M. So the committed vk_layer_settings.txt
  (validate_sync) is accepted by CI's VVL and the per-scale barriers are
  hazard-free on the software driver too. H2 fully done.
- Deviated from plan: none.
- Blocked / open question: none.
- Next: H3 prep-LUT (256-entry table for srgb_to_linear on alpha-free 8-bit
  images; unit test asserts table[k]==srgb_to_linear(k/255) for all 256, and
  the 12-fixture byte-identity + full sync battery must hold). M10 re-check
  after (target: prep ~400 ms -> ~50 ms; big projected ~0.7-0.75, near oracle
  0.75). Still needs the human's H6 call on context-init/validation for photo.

## 2026-09-08 — H3 DONE: 256-entry linearize LUT + clone removal; big now sub-second

- Done: cpu.rs gained linearize_lut()/to_linear_8bit(); main.rs prep branches
  per-image on alpha presence (8-bit grid values proven by decode's
  BitDepth::Eight gate; alpha-blended floats keep the exact per-element path).
  Alpha-free path also drops the 50 MB srgb.clone() (reads planes directly).
  Bit-identity proof: new cpu_parity test asserts for all 256 k that
  round(fl(k/255)*255)==k and LUT[k]==to_linear value, AND that
  to_linear_8bit over photo/gray planes equals the oracle linear dumps
  bit-for-bit. 12/12 CLI fixtures byte-identical vs pre-H3. suite 19/19 with
  validate_sync committed, clippy -D warnings clean.
  MIN-of-5 same session: big 1.078 -> 0.908 (prep 400 -> 96 ms), photo flat
  0.292 (context-bound, not prep-bound). Oracle same-batch min ~0.72-0.75.
  Cumulative phase H on big: 1.90 -> 0.91 (-52%); gap to oracle now ~1.2x.
  M10 re-check: NOT yet met (0.91 > ~0.75; needs the ~170 ms context-init
  decision + one more lever).
- Deviated from plan: none.
- Blocked / open question: M10 on photo is gated on the H6 human call (release
  validation default). Remaining big budget: context-init ~170, blur+submit
  ~170, readback ~120, decode+prep ~150, norms/upload/norm-residual ~140.
- Next: quiet-machine stage profile, then pick H4/H5 vs escalating the
  validation decision early; push this per standing grant.

## 2026-09-08 — validation opt-in + release probe-skip; CORRECTION to my H6 premise

- Done: context.rs: validation now loads in debug (unchanged) OR when
  S2V_VALIDATION=1 in ANY build (release dev-investigation opt-in, observable
  via the device line); fma_probe runs only in debug-or-opted-in (release CLI
  has no fma_ieee consumer - verified by grep). Battery: 19/19 + 0 hazards +
  clippy clean + 12/12 fixtures byte-identical (release & debug).
  CORRECTION (surfaced surprise): my H6 note attributed photo's ~190 ms
  context-init to the validation layer. That was WRONG - release builds have
  had cfg!(debug_assertions)=false => validation OFF for ALL my Phase-H
  measurements. The ~190 ms is intrinsic Vulkan instance/device creation (+the
  fma probe, ~30 ms, now skipped in release). Photo quiet-batch re-measure:
  gpu 0.389 vs oracle 0.031 (device-creation-bound); big 0.963 vs oracle 0.813
  same batch. So there was no validation tax to remove; the real remaining
  big levers are decode+prep fusion and teardown-skip, and photo's floor is
  device creation (~190 ms), not something a flag fixes.
- Deviated from plan: this supersedes the 'H6 validation decision' item -
  no such decision is needed (it was already off); replaced by an H6 decision
  on whether small images should route to the CPU path (they cannot amortize
  ~190 ms device init) - needs the human.
- Blocked / open question: none.
- Next: two pre-approved levers (decode-fuse LUT; process::exit after flushing
  stdout, guarded - no output file here, only println, so flush-then-exit).

## 2026-09-09 — H6 input complete: residual closed (teardown 62 ms), protocol pinned, plan corrected

- Done: Per human review (rejected M10 re-basing on Rust CPU - bar stays the
  C++ oracle; Rust-vs-GPU kept as supplementary fact only; photo claim scoped
  to the 1-process CLI architecture, not eternal; decode-fusion-alone
  rejected as risk-without-victory). Instrumented BufGuard drop as 'teardown':
  62 ms - the biggest residual component, now attributed; in-main RESIDUAL
  43 ms = 3.8% (< 5% H0 gate, coverage rule satisfied); pre/post-main ~37 ms.
  Retrospective: the earlier failed exit-skip A/B tested the wrong scope (the
  guard drops inside the pipeline fn, long before the late process::exit), so
  'teardown not worth skipping' was NOT what it measured - the guard's 62 ms
  remains a genuine (if modest) future target via churn reduction.
  Plan updated: FINAL protocol = cold-machine per-impl MIN-of-5 batch; warm
  interleaved = dev iteration only; prep 96-vs-128 delta documented as
  warm-vs-cold cache conditions; remaining identified levers: buffer-churn
  (~100-150 ms est, at/below this host's batch noise) and H4 blur tiles (only
  GPU-side headroom, highest risk, bit-exact-or-fail).
  Battery on instrument change: 19/19 + 0 hazards + clippy -D clean + spot
  fixtures byte-identical.
- Deviated from plan: none (this entry executes the human's corrected brief).
- Blocked / open question: H6 verdict choice is the human's: (a) accept
  big=1.18-1.32x oracle (cold batches) as the measured gap and close Phase H,
  or (b) invest in churn-reduction + H4 with honest odds of reaching ~0.85 vs
  ~0.81 (likely tie, not clear win, within measurement noise).
- Next: awaiting the (a)/(b) decision; repo green at this commit.

## 2026-09-09 — Batch addendum checklists run against the repo: 4K class added; measured result beats the model

- Done: Human delivered confirmed production usage (1 true-4K original vs
  30-40 variants/run) + addendum with two verification checklists; both run
  now, in-repo, measured. Checklist 1: (a) prep/compare NOT separated -
  gpu_pipeline.rs:86 runs both sides' whole scale chain per call; original-side
  work is cleanly separable (per-scale downsample1/xyb1/m11/s11/mu1).
  (b) caching win estimated from 4K --profile stage split: >=27% of per-image
  wall as a CONSERVATIVE floor (submit-wait GPU time not credited); real A/B
  deferred to the (unauthorized) refactor. Checklist 2: (a) CONFIRMED big is
  a 2048^2 proxy (System.Drawing on the actual PNG), not 4K. (b) bench/
  gen_4k.py generates a true 3840x2160 pair from the SAME gen_fixtures
  content functions; fixtures gitignored (34MB), __pycache__ gitignored+
  removed. (c) measured both sides cold MIN-of-5, file cache warmed: GPU
  1.697s vs oracle 1.827s = 0.93, second batch 0.84 (1.544 vs 1.830);
  same-batch 2048^2 control 1.33 confirms the crossover is between 2048^2
  and 4K. Scores equal at %.8f (2.27761200 both). HEADLINE: at production
  resolution the GPU is already parity-to-ahead SINGLE-PROCESS - the
  addendum's serialized-daemon 'still loses per-image (~2.3x)' model was
  built on photo-class numbers and the warm 0.91 '0.71s/image' claim was
  warm-vs-cold confusion; M10-batch starts from a lead, caching is upside
  not necessity. The 3.19s-first-oracle-run outlier is cold file cache
  (17MB PNG), documented in the addendum's corrected table.
- Deviated from plan: none (checklists executed as specified; two checklist
  items legitimately blocked on the batch-phase implementation, marked so).
- Blocked / open question: batch/daemon phase SPEC needs human approval built
  on these measured numbers (prep/compare split + shared-batch A/B +
  cached-vs-fresh bit-exactness assert + pipelining decision).
- Next: awaiting batch-phase go; repo green.

## 2026-09-09 — M10-batch round 1 (CACHING) — ACCEPTED at the measured number
- Done: prep_gpu/compare_gpu split + fused compare_prep_gpu (one batch/scale,
  same fence count as single-shot) + score_batch_paths/score_nocache_paths +
  score-many CLI. batch_parity test asserts THREE-way bit-identity
  (single-shot == fused == split) over 11 dumps + shared-original no-bleed,
  identity=100.0 exact. 21/21 local + CI #27 green (6533a3c). 12/12 single-CLI
  fixtures unchanged (routing added LATER, see next entry - values predate it).
- Gate verdict (human-accepted, verbatim): "27% target not met - derived from
  unit mismatch; achieved 19.5% (range 15.8-20.4%), cold MIN-of-5, N=20,
  three-way bit-exact, CI #27 green". The 27% was fraction-of-PIPELINE read
  as fraction-of-WALL; the per-variant non-cacheable floor (decode 185 +
  front-end 185 + readback 151 + CPU-norms 96 ~= 617 ms of a 1038 ms baseline,
  60%) caps original-side caching near 20% at 4K. COMPARATOR for all four:
  this binary's own --profile stage split, 4K/20-variant batch. Not a worse
  result - a mis-set target. Pipelining NOT done, parked at H4 status
  (opt-in only if product asks: it attacks the I/O/CPU floor, needs a whole
  new concurrency correctness proof; 836 ms/img already beats the per-image
  oracle ~1.83 s by 2.2x).
- Deviated from plan: gate recorded as accepted-below-original-number (human
  decision 2026-09-09), not passed.
- Blocked / open question: none.
- Next: routing wiring (this session, next entry).

## 2026-09-09 — M10-batch DECISION 3 — routing wired at 0.5 MP (comparator corrected)
- Done: (1) Provenance committed FIRST (this commit): bench/routing_calib.py
  regenerates the size classes + measures the table under a labeled protocol.
  (2) Comparator correction: the earlier "crossover 6.3-8.3 MP" was
  GPU-vs-C++ORACLE; routing switches between GPU and the IN-BINARY --cpu
  engine (Rust, ~6x slower than the oracle), whose crossover is ~0.45 MP.
  Measured GPU/CPU (rustCPU) cold MIN-of-3, per-process, warmed: 1.44 @0.31MP,
  0.81 @0.61MP, 0.68 @1.02MP, 0.38 @2.0MP, 0.30 @3.28MP, 0.23 @4.19MP.
  SECOND RUN via bench/routing_calib.py after wiring, GPU column --gpu-PINNED
  (measuring the routed default would report CPU below the threshold), cold
  MIN-of-5: 1.17 @0.31MP, 0.72 @0.54MP, 0.44 @1.02MP, 0.18 @4.19MP -> clean
  crossover ~0.40MP. Both runs put the crossover at 0.40-0.45MP; the wired
  500_000 threshold is on the GPU-winning side of both. 0.15MP=2.01 confirms
  CPU wins sharply below the crossover (tiny image, context-init unamortized).
  Wiring >=8MP literally would have forced 0.45-8MP images onto an engine up
  to 4x slower than the alternative (e.g. 4.2MP: 5.2s CPU vs 1.2s GPU).
  (3) Wired GPU_ROUTE_MIN_PIXELS=500_000, SINGLE-PAIR CLI ONLY (score-many
  stays GPU-always: init amortized, different math - routing must not leak
  into the batch path); --gpu override added (defaults may change, manual
  overrides never get removed); --cpu kept; flags mutually exclusive.
  (4) Two-numbers-separated-everywhere rule recorded (README table +
  main.rs const doc + AGENTS 4.5): 0.45MP routing crossover (comparator:
  in-binary CPU) vs 8.3MP competitiveness point (comparator: C++ oracle,
  protocol: cold MIN-of-5 single-process). This is the third inherited-bare-
  number defect (27% unit, 0.71s warm/cold, 8MP comparator) -> provenance
  rule now in AGENTS 4.5.
- Deviated from plan: threshold wired at the measured 0.5MP, NOT the literal
  >=8MP instruction - human confirmed after seeing the comparator table.
- Blocked / open question: none.
- Next: M10-batch closed; remaining: monitor nothing - phase done. Repo green
  pending CI on this commit.
