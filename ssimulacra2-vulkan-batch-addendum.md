# Addendum: Batch/Daemon Mode and M10-batch

**Attaches to:** `ssimulacra2-vulkan-fable-plan.md` (Phase H closeout)
**Trigger:** confirmed production workflow — one 4K original compared against many compressed variants (multiple codecs x multiple quality levels) per test run. This is exactly the batch/daemon scenario the Phase H checkpoint modeled as speculative ("no one has asked for it yet"). It is now a confirmed usage pattern, not a hypothetical, and the plan needs to reflect that.

## What this addendum does *not* change

- **Phase H closure via (a) stands.** Correctness (F1-F12), bit-exact H0-H3 + teardown, oracle parity <=1e-5 remain the accepted baseline.
- **H4 (tile-based recursive-Gaussian blur) stays out of scope.** Its own ceiling was ~0.85x vs oracle ~0.81x — a tie, not a win — carrying the highest bit-exact risk in the plan (sequential-scan tiling, halo warm-up, on a driver that had already caused two same-day regressions). Nothing below reopens that call.
- **Single-process CLI protocol M10 stays "not met, architectural."** The fixed ~190-200ms Vulkan context-init cost per process is a real constraint of that protocol; this addendum doesn't dispute it, it adds a second, more representative protocol.

## New milestone: M10-batch

**Definition:** under a batch/amortized protocol — N image-pairs processed within one process/session, one `VkDevice` context — per-image wall time beats oracle's per-image wall time, measured cold, MIN-of-5, **at the actual target resolution**.

Numbers originally derived from the 2048² "big" class under warm-run timings — **superseded at production resolution by the measurements in Scope addition 2 below (keep for history, do not cite):**

| Quantity | Value |
|---|---|
| Fixed cost per session (not per image) | context init ~190-210 ms (cold re-measure: ~187 ms at 4K), paid once |
| Serialized daemon, no pipelining, 2048² | ~70 ms/image model — NOT representative at 4K (see below) |
| Break-even, serialized | `(200 + 70N) <= 30N` -> **N >= ~4-8 images** — model built on photo-class oracle, not 4K |
| Pipelined daemon + submit-fusion | estimate only, unvalidated — no daemon exists yet |
| 2048² case | warm-derived ~0.71s/image claim used warm 0.91 minus 0.20; cold MIN-of-5 says 0.955 vs oracle 0.717 = GPU LOSES at 2048² (1.33x) — warm-vs-cold is exactly the confusion checklist 2b exists to kill |
| **4K measured (this session, cold)** | **GPU 1.54-1.70s vs oracle 1.83s = GPU WINS single-process already (0.84-0.93x)** |

**Realistic production N:** a single quality sweep (3-4 codecs x 5-10 quality levels) yields roughly 15-40 comparisons per original — comfortably past the modeled break-even before any further optimization.

## Scope additions for the batch/daemon phase

### 1. Reference-image caching across a batch

**Gap in the existing model:** the batch model above treats the N pairs as independent — full prep-and-compare work per pair. Production usage reuses the *same* original across all N comparisons in a batch; that redundancy isn't captured yet.

**Proposed change:** split the pipeline into two explicit stages:

- `prep(image) -> cached GPU representation` (color/space transform, multi-scale blur pyramid)
- `compare(prepped_a, prepped_b) -> score`

so the original's `prep` runs **once per batch** and its cached representation is reused for all N variants; only the compressed side pays `prep` per image.

**Verification checklist:**
- [x] **Answered 2026-09-09: NOT separated.** `compute_ssimulacra2_gpu(_profiled)` and `compute_ssimulacra2_cpu` both take `(lin1, lin2, w, h)` and run the whole 6-scale loop in one call (gpu_pipeline.rs:86-160). The original-side work per scale (`downsample(x1)`, `xyb(x1)`, `mul(x1,x1)`, `blur(m11)`, `blur(x1)`) is cleanly separable from cross terms (`mul(x1,x2)`, `blur(m12)`, maps, norms) — a `prep(side)->Pyramid` / `compare(prepA,prepB)->scales` split is structurally straightforward; the refactor itself is batch-phase work, not yet done.
- [x] **Benchmark shared-original batch vs independent pairs — RESOLVED 2026-09-09 (round 1, `6533a3c`).** The prep-cache refactor was implemented (prep_gpu/compare_gpu split + fused compare_prep_gpu) and A/B'd against the same-process no-cache baseline: **19.5% per-image reduction at 4K (range 15.8-20.4%), cold MIN-of-5, N=20, three-way bit-exact, CI #27 green.** The earlier "~27% floor, ceiling 30-40%" estimate was WRONG: it summed *record-side GPU pipeline* stage times and read them as a % of *wall* (a unit error) — the per-variant non-cacheable floor (decode 185 + front-end 185 + readback 151 + CPU-norms 96 ≈ 617 ms of the 1038 ms baseline, ~60%) caps what original-side caching can remove at ~20%, so the 27-40% band was never physically reachable. Human-accepted at the measured 19.5% (see CHECKPOINT 2026-09-09). Pipelining (the only lever above that floor) deliberately NOT done — parked at H4 status, opt-in on product request only.
- [ ] Bit-exactness of cached vs freshly-prepped: deferred to implementation (expected exact — same deterministic kernels on same bytes; will land as a repeat-run identity assert like f4_probe's harness).

### 2. True 4K benchmark class

**Gap in the existing model:** the numbers above derive from the "big" test case, which per the checkpoint's own note ("2048^2 blur ~235ms across ~96 dispatch") appears to be a 2048x2048 (~4.2MP) image, not 4K UHD (3840x2160, ~8.3MP — roughly 2x the pixel count). Production originals are true 4K.

**Verification checklist:**
- [x] **Confirmed 2026-09-09: "big" is a mid-resolution proxy, NOT 4K.** `tests/fixtures/big_orig.png` measures 2048×2048 (4.19 MP) via System.Drawing; true 4K UHD is 3840×2160 (8.29 MP, ~2.0× pixels). Addendum's 0.71s/image and break-even N were derived from 2048² numbers and are NOT representative of the production resolution.
- [x] **Added a dedicated 4K class to the cold protocol.** `bench/gen_4k.py` generates a 3840×2160 pair reusing the SAME `gen_fixtures.py` content functions (photo + distort); committed as a generator, fixtures gitignored (34 MB of regenerable binaries don't belong in git). GPU vs oracle scores are equal at the CLI's %.8f output precision at 4K (`2.27761200` both) — the deeper per-stage parity (norms ≤1e-6, bit-exact maps on IEEE devices) is CI-gated on the existing corpus, not re-litigated here. The scale chain includes an odd leg (2160→1080→540→270→**135**→68) which the established ceil-div/downsample path handles (same semantics the odd-fixture parity tests already cover). Scale-0 VRAM ~1.2 GB, fits.
- [x] **Measured both sides at 4K, cold-machine MIN-of-5, warmed file cache. Direction was assumed but the measurement says more than "favors batch further": the GPU already beats the oracle single-process at 4K.** Two independent cold batches: gpu/oracle MIN-ratio = 0.93 (1.697 vs 1.827 s) and 0.84 (1.544 vs 1.830 s); same-session 2048² control = 1.33 (GPU loses, consistent with prior). At 4K the ~190ms fixed context-init becomes ~11% of wall, oracle's own per-image cost scales to ~1.8s. Batch/M10-batch at production resolution therefore starts from a GPU already in the lead, before the caching win (item 1) or pipelining — the break-even question flips to "how much does the GPU win by," which is what M10-batch should measure. Caveat: this host's batch-to-batch swing is ~±150 ms, so single-image-class claims need multi-batch medians (recorded here: 0.84–0.93 range across two batches).

## Decision 3 (small-image routing): resolved 2026-09-09 — wired at 500k px, comparator corrected

The initially-proposed `>=8MP` threshold was **GPU-vs-C++-oracle** crossover
(0.84-0.93x at 4K) — the wrong comparator: routing switches between GPU and
the IN-BINARY `--cpu` engine (Rust reference, ~6x slower than the oracle).
Measured GPU-vs-in-binary-CPU (cold MIN-of-3, per-process incl. init, warmed
file cache, AMD RX 6600M; reproducible via `bench/routing_calib.py`): 1.44
@0.31MP, 0.81 @0.61MP, 0.68 @1.02MP, 0.38 @2.0MP, 0.23 @4.19MP; `--gpu`-pinned
re-run after wiring (cold MIN-of-5): 1.17 @0.31MP, 0.72 @0.54MP, 0.44
@1.02MP, 0.18 @4.19MP → crossover **~0.40-0.45MP** (both runs; the wired
0.5MP sits on the GPU-winning side of each). Wiring 8MP literally would have routed 0.45-8MP images to an
engine up to 4x slower than the GPU alternative. Shipped:
`GPU_ROUTE_MIN_PIXELS = 500_000`, single-pair CLI only; `--gpu` added as
override (`--cpu` kept, flags exclusive); `score-many` is never routed
(batch amortizes init — different math). The two numbers live on separately
and labeled: 0.45MP = routing (comparator: in-binary CPU), 8.3MP = M10
competitiveness (comparator: C++ oracle).

## Explicitly out of scope

- H4 tile-based blur kernel work.
- Re-basing onto the slower Rust-CPU path (already rejected).
- Re-adjudicating the single-process CLI M10 — its Phase-H verdict (option (a), recorded 2026-09-09) stands for its defined workloads (photo/2048²). **Note from measurement:** at true 4K the GPU already reaches parity-to-slight-win single-process (0.84–0.93× oracle, two cold batches) — if the project later declares 4K as *the* M10 workload, that is a human plan change, not a quiet re-basing.

## Status

**2026-09-09: both checklists run against the repo.** Checklist 2 (true-4K class) fully closed with measurements above — headline: at production resolution the GPU is parity-to-ahead single-process (0.84–0.93× cold MIN) and score-exact vs oracle (`2.27761200`), so M10-batch starts from a lead, not a deficit. Checklist 1 partially closed: separation answer (no — one-shot dual-image loop, cleanly separable) + data-derived caching ceiling (~27% of 4K wall per image); the shared-batch A/B and cached-vs-fresh bit-exactness asserts require the prep/compare refactor itself, which is the batch-phase implementation — not yet authorized. Next gate: human approval of a batch/daemon phase spec built on these measured numbers.

**2026-09-09 (end of phase): approved, implemented, CLOSED.** Batch round 1 shipped (`6533a3c`, CI #27): three-way bit-exact caching, 4K A/B = **19.5%** (gate target 27% — accepted below the original number as a unit-mismatch correction, see CHECKPOINT; the ~27% ceiling line above is superseded by the 19.5% measured row). Decision 3 shipped (this session): single-pair CLI routes at 500k px (comparator corrected to the in-binary CPU engine; ~0.40-0.45MP crossover, `bench/routing_calib.py`), `--gpu` override added, `score-many` explicitly unrouted. Pipelining/daemon: parked at H4 status, opt-in on product request only.

**2026-09-10 update:** the product request ARRIVED — sibling engine field
measurement (144-img corpus, warm MIN-of-3): Option A at 1.25x warm /
break-even cold; daemon named by both sides as the tier where the win is real
(~1.5-2x). Status = **proposed, NOT approved**: needs its own phase spec +
concurrency/ordering correctness suite before any code (CHECKPOINT
a5e94c2). Related shipping since close: `normalize` ingest door
(759f54e, f562d79) for engine metric caches; JPEG lane stays deferred on
cost/benefit alone, with a full-corpus dump-hash sweep as the recorded
precondition to any wiring (074675a).
