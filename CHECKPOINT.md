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
