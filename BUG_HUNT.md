# Bug hunt — ssimulacra2-vulkan (2026-09-08)

Full-repo adversarial review of the M0–M9 state: every Rust source file and every
GLSL shader in `ssimulacra2-vulkan/`, cross-checked line-by-line against the
C++ reference (`src/ssimulacra2.cc`, `src/lib/jxl/{gauss_blur,enc_xyb,fast_math-inl,
transfer_functions-inl,rational_polynomial-inl,linalg,opsin_params,image_ops}.h/cc`),
plus the test suite, CI workflow, and oracle scripts.

Baseline observation: `cargo test --workspace` on this host (RX 6600M, IEEE-fma
path, validation active) — 13/13 green, 2026-09-08. CI run #6 (lavapipe) — green.

Severity: **H** = can produce wrong results or invalid API use on reachable inputs;
**M** = weakens the verification net / silent divergence; **L** = latent, currently
unreachable or cosmetic.

---

## F1 (H) — Compute dispatches exceed device limits; limits are never queried

`pipeline.rs:150` dispatches `cmd_dispatch(cb, groups_x, 1, 1)` with `groups_x`
computed by callers as `ceil(N/64)` over flat element counts. `context.rs` never
reads `maxComputeWorkGroupCount`, `maxStorageBufferRange`, or `maxBufferSize`
(grep: zero hits for `limits` in the crate).

The committed `big` fixture (2048²) already exceeds the Vulkan **guaranteed
minimum** `maxComputeWorkGroupCount = (65535, 65535, 65535)` (verified against
the spec's required-limits table):

| kernel | groups for 2048² | spec min |
|---|---|---|
| `xyb_convert` (`gpu_pipeline.rs:72`, `xyb.rs:79`) | 65 536 | 65 535 |
| `mul_planes` (`gpu_pipeline.rs:74-81`) | 196 608 | 65 535 |
| `maps_combine` (`gpu_pipeline.rs:88`) | 196 608 | 65 535 |

Any driver reporting the minimum (the spec floor; several do) makes these
dispatches invalid usage → undefined behavior (truncated work → silently wrong
scores, not a clean error). The local AMD/RADV path works because that driver
reports a far larger x-limit. CI does not catch it because of F2 (no
validation) and because llvmpipe tolerates over-limit dispatches. (Buffer sizes
are fine: the 50 MB maps buffers stay under the 2^27 `maxStorageBufferRange`
and 2^30 `maxBufferSize` minimums.)

Fix: query limits at context creation; either reject images whose
`3*w*h/64 > maxComputeWorkGroupCount.x` (fall back to CPU in the CLI), or
dispatch 2D/3D (y/z up to 65 535 each covers 2^42 elements).

## F2 (M) — CI runs without the validation layer it claims to enable

`.github/workflows/ci.yml:5` says "validation layer on"; the rust job installs
only `mesa-vulkan-drivers libvulkan1 vulkan-tools` (ci.yml:52). On Ubuntu
24.04 the Khronos layer ships in **`vulkan-validationlayers`**, which is absent.
`context.rs:56-66` probes for the layer and silently proceeds without it.
Evidence: CI run #6 job logs contain zero validation output (checked
2026-09-08). The layer is what caught the real M1 bug (missing
`TRANSFER_SRC`, VUID-vkCmdCopyBuffer-srcBuffer-00118) — that protection is
inactive in CI, and it is also why F1 goes unflagged there.

Fix: add `vulkan-validationlayers` to the apt list; optionally fail the job on
validation errors (`VK_LAYER_MSG_IDFILTER`/abort-on-error via a debug messenger).

## F3 (M) — NaN-blind f64 comparators in the parity tests

`oracle_dump::max_abs_diff` (f32) is NaN-aware (`oracle_dump.rs:65`:
`d.is_nan() || d > worst.0`). The three f64 comparators are not:

- `e2e_parity.rs:8-10` — `fold(0.0, f64::max)`: `f64::max` returns the non-NaN
  operand, so a NaN drift is silently dropped and reported as 0.
- `maps_parity.rs:12-19` — `if d > worst.0`: NaN comparisons are false → skipped.
- `maps_parity.rs:22-35` — `range_audit`: `a > 0.0` and `a > maxabs` are both
  false for NaN → NaN values are invisible to the FTZ audit too.

A maps_combine regression that emits NaN would pass `maps_parity` end-to-end
(blurred planes are checked with the NaN-aware f32 comparator, so the NaN would
have to originate in the combine kernel itself — exactly the unguarded step).
`e2e_parity` still catches it at the score level (`NaN <= bar` is false). Fix:
mirror the `is_nan()` guard in the f64 paths.

 ## F4 (M) — llvmpipe identity anomaly — **CLOSED 2026-09-08 (CI run #17)**

 Resolution: the reduction below was right that only `maps_combine` could
 diverge, but wrong to exclude "fma contraction in num_s/denom_s" - it
 reasoned about runtime values, not the compiler's SSA pattern. num_s's
 `delta+delta` (one value used twice) matches NIR's `x+x -> 2*x -> fma`
 inexact rule while denom_s's `d1+d2` (two values) does not, so llvmpipe's
 non-IEEE fma hit only num_s -> num_s != denom_s -> d ~ 7*2^-24 (deterministic
 throughout, which is why per-dispatch tests never showed it). Root cause
 confirmed by the synthetic-tree probe (f4_probe.rs, runs #13-#15) and fixed by
 `precise`/NoContraction on every ssim_d-chain result in maps_combine.comp.
 Run #17 on llvmpipe: probe A-E all 0/12288 differ, identity ssim_d nonzero
 0/36864 (was 20718), identity score exactly 100.0 (was 99.98426951). Gates
 tightened to assert on every device (e2e, cli, determinism, probe). The
 historical reduction and localization text below is kept as the case file.

 (was: M, known/open) — with a reduction

 CHECKPOINT 2026-09-08 (run #5): identity fixture returns 99.98426951 on
llvmpipe (norms 2.5e-7 ≠ 0) while RDNA2 is exactly 100.0.

This review's algebraic reduction: for bitwise-identical `lin1 == lin2`,
`x1 == x2` ⇒ `m12 == m11 == m22` ⇒ `s12 == s11 == s22`, `mu1 == mu2` ⇒
`dm = +0`, `num_m = 1.0`, `num_s == denom_s` (both are `fl(2·(s11−mu11)) + kC2`
— the doubling is exact, so the non-`precise` mul-add contraction in
`maps_combine.comp` cannot break the equality) ⇒ `prod == denom_s` ⇒ `q = 1.0`
⇒ `d = 0` exactly. So identity failure reduces to **one shader producing
different bits for identical input content across two dispatches** (candidates:
`xyb_positive`, `mul_planes`, `blur_h/v`). llvmpipe's non-IEEE fma is
deterministic per-input, so the prime suspects are alignment-dependent JIT
specialization or genuine per-dispatch nondeterminism.

Concrete localization experiment for CI (cheap, no second GPU needed): upload
the same `linear_orig_s0` twice, run `xyb_convert` on both, `readback_f32` and
memcmp; then `blur_planes` twice on one buffer's content. The first kernel whose
two outputs differ is the culprit. Until then the IEEE-only gating stands, but
the report should not let "driver-independent identity" (e2e_parity.rs:19
comment) stand as written — it is driver-dependent, as the anomaly proves.

## F5 (L) — `create_recursive_gaussian` rounds the radius differently than the oracle

`blur.rs:21`: `(3.2795 * sigma + 0.2546).round()` (f64 round). The oracle,
`gauss_blur.cc:504`: `const double radius = roundf(3.2795 * sigma + 0.2546);` —
**float** rounding of a double expression (narrow→roundf→widen). Identical for
sigma = 1.5 (5.17385 → 5 either way), so no observable effect in this port, but
it is a transcription deviation that would bite if any other sigma is ever
profiled (Phase H). Either transcribe `roundf` exactly or add a comment pinning
sigma=1.5 as the only supported value.

## F6 (L) — `cpu.rs` negative-zero clamp diverges from `ZeroIfNegative`

`cpu.rs:192`: `if v < 0.0 { v = 0.0 }`. The oracle (`enc_xyb.cc` via hwy
`ZeroIfNegative` = `IfThenZeroElse(IsNegative(v), v)`, `IsNegative` = sign-bit
test — verified in the installed `hwy/ops/generic_ops-inl.h`) and the shader
(`max(mixed, 0.0)`) both map
**−0.0 → +0.0**; cpu.rs keeps −0.0, and `cbrt_and_add(−0.0)` then takes the
non-zero seed path (`m1 = 0x80000000 ≠ 0` → seed ≈ 2.9e38 → r² overflows →
NaN). Unreachable with real data (the fma chain includes bias kB0 > 0, so
v ≥ 0.00379), but `v.max(0.0)` is the exact-semantics fix.

## F7 (L) — `neg_bias_cbrt` is a double-rounded match to one libm's output

`xyb.rs:35-38` computes `-((kB0 as f64).cbrt() as f32)` to match the oracle's
`-cbrtf(kB0)` (mingw), after UCRT's f32 cbrt was found 1 ulp off. f64→f32
double-rounding equals a correctly-rounded f32 cbrt only when the f64 result is
not near an f32 midpoint — verified for this constant, not guaranteed in
general. Guarded by the `xyb_parity` bit-exact assert (good). Risk only if the
bias constant or the oracle's libm changes; the assert will catch it.

## F8 (L) — No RAII on `GpuBuffer`; error paths leak

`GpuBuffer` has no `Drop`. Every `?` in `gpu_pipeline.rs` / `blur.rs` /
`pipeline.rs` after a buffer creation leaks device memory; `pipeline.rs` leaks
shader module + layout + pipeline + descriptor pool on any mid-failure;
`one_shot` (`context.rs:296-321`) leaks the command buffer if begin/end/submit
fails. `VkContext::drop` then destroys the device with live buffers (validation
"leaked object" errors). Happy paths are clean (verified: local suite green
with validation active). Consider a `Drop` impl or a scope guard.

## F9 (L) — Stale/incorrect doc comment on `rg_upload`

`blur.rs:122-124` claims the upload layout has "[48] = radius (as f32 bits…)".
Wrong: `[48..60)` is `mul_in` (broadcast ×4); radius travels in push constants
(`blur.rs:139-145`). Misleading to anyone editing the shader's rg indexing.

## F10 (L) — CLI input-domain divergence from the oracle

`cpu.rs:64-66, 99`: 16-bit PNGs and gray+alpha PNGs are rejected by the Rust
CLI; the C++ oracle accepts and scores them. Silent behavior difference for
inputs the oracle handles (reject vs. score, not wrong score). Fine as a
documented limitation — but see F11: it is not documented anywhere.

## F11 (L, docs) — README describes only the upstream C++ tool

`README.md` is the original ssimulacra2 README (build via `build_ssimulacra`,
C++ usage). Nothing documents the Vulkan port: `cargo build`, the
`ssimulacra2-vulkan` CLI, `--cpu`, the 8-bit-PNG-only input domain, the
iCCP/gAMA/cHRM rejection, or the `oracle/gen_goldens.sh` dump workflow that
every parity test depends on. A fresh user (or agent) cannot discover from the
README that `cargo test` on a clean clone fails until dumps are generated
(`dumps/` is gitignored; six test files hard-require `dumps/*/run1` and panic
with "dump file" otherwise — CI regenerates them, local clones don't).

## F12 (L, CI coverage) — Oracle job re-verifies only part of the goldens

ci.yml:33-38 checks 10 pairs + identity on Linux. Not re-verified on Linux:
`big` (2048²), the `s7` rejection message, and the asymmetric-alpha goldens
(`oracle/scores_asymmetric.txt`, the numbers that pin the BUG-1 fix). The
Rust-side tests do check those against the Windows-recorded numbers, so a Rust
regression is caught; an oracle-side Linux divergence in those three cases is
not.

---

## Verified correct (no findings)

The following were checked against the cited C++ and found faithful:
XYB opsin fma chain + bias + `ZeroIfNegative` + `CubeRootAndAdd` op-for-op
(including the `IfThenZeroElse` m1==0 case and the 3+1 Newton structure,
`fast_math-inl.h:177-210`); `MakePositiveXYB` order (B pre-shift-Y, `precise`
against mul+add contraction); `StoreXYB` 0.5 forms; blur 3-phase horizontal
(border lane-0 / 4-output unrolled with `ShiftLeftLanes`-equivalent index
direction / remainder, `first_aligned = RoundUpTo(N+1,4)`, unroll bound
`len−N+1−3`), vertical naive form with `NegMulSub` semantics and the
SingleInput/TwoInputs-with-zero-row border cases; `CreateRecursiveGaussian`
f64 derivation incl. `Inv3x3Matrix`/`MatMul` order (`linalg.h`); rg upload
layout vs shader indices (0/12/24/36/48); `Downsample` clamp-replicate,
iy→ix order, `*0.25` last; `Multiply`; SSIMMap/EdgeDiffMap f32 op order,
double `d`/`d1` promotion points, `max(d,0)`, kC2; norms `onePerPixels *`
order and `sqrt(sqrt())`; 108-weight loop order (c-major, scale, norm) with
the missing-scale index shift; score polynomial precedence and the
`ssim > 0 → else 100` branch; `build.rs` weight extraction (bit-exact via
`to_bits`); TF_SRGB rational polynomial (Horner order, `Div` under `#if 1`,
`kLowDivInv = 1.0f/12.92f`, threshold `Gt 0.04045f`); `ConvertToFloat`
`*(1/255)`; AlphaBlend in sRGB space; CLI contract (argc, <8×8 check order,
size mismatch, worst-of-bg only when the **original** has alpha, `%.8f`);
push-constant layouts vs GLSL blocks (68 B XYB ≤ 128 min); descriptor binding
order vs shader bindings for all 8 shaders; u32 overflow margins for all
committed sizes; dispatch bounds guards in every shader; `fma_probe`
discriminating constants; dump header format vs `oracle_dump.rs`.

## Recommended order of work

1. F1 + F2 together (limits query + validation package) — they mask each other.
2. F3 (comparator NaN guards — five lines).
3. F4 localization experiment (one CI test, resolves the open anomaly).
4. F5–F9 as opportunistic cleanups; F10–F12 are documentation/coverage.
