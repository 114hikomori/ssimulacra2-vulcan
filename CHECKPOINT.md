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
