# Oracle — C++ SSIMULACRA2 v2.1 reference for the Vulkan port

Everything here is local tooling around the vendored C++ reference; the default
`ssimulacra2` binary and its numeric paths are unchanged by the port (proof below).

## Build (native Windows, MSYS2 ucrt64 — no WSL2 on this host)

```bash
# one-time deps (already installed on this machine):
pacman -S --needed mingw-w64-ucrt-x86_64-{gcc,cmake,ninja,highway,lcms2,libpng,libjpeg-turbo,zlib}

# default (oracle) build:
cmake -S src -B build -G Ninja -DCMAKE_BUILD_TYPE=Release -DJPEGXL_ENABLE_OPENEXR=false
ninja -C build ssimulacra2

# dump build (adds -DSSIMULACRA2_DUMPS only):
cmake -S src -B build-dump -G Ninja -DCMAKE_BUILD_TYPE=Release \
  -DJPEGXL_ENABLE_OPENEXR=false -DCMAKE_CXX_FLAGS=-DSSIMULACRA2_DUMPS
ninja -C build-dump ssimulacra2
```
Run inside the UCRT64 environment (needs /ucrt64/bin on PATH for the DLLs).

## Dump instrumentation (`src/ssimulacra2.cc`, `#ifdef SSIMULACRA2_DUMPS`)

Provenance: local patch to this repo's own BSD-3-Clause source, default-off,
logged per AGENTS.md §10. Sets: `SSIMULACRA2_DUMP_DIR=<dir>`, then run the
dump binary on any image pair. Files: `r<run>_<tag>_s<scale>.bin` + `meta.txt`.

Header (48 bytes, little-endian): magic `0x31443253` ("S2D1"), dtype
(3=u32, 4=f32, 8=f64), xsize, ysize, channels, stride(elements),
payload_bytes(u64), reserved(u64), reserved(u64). Payload follows, packed
plane-major.

Tags: `linear_{orig,dist}` (post-sRGB-linearization input), `xyb_pre_*` (after
ToXYB), `xyb_*` (after MakePositiveXYB), `sigma1_sq/sigma2_sq/sigma12/mu1/mu2`,
`ssim_d_c<0..2>` (per-pixel f64), `edge_d1_c<0..2>` (per-pixel f64),
`ssim_norms` (6×f64), `edge_norms` (12×f64), `score_weighted`, `score_final`,
`rg` (5×12 f32: n2,d1,mul_prev,mul_prev2,mul_in), `rg_radius` (u32).

`oracle/gen_goldens.sh <tag>` regenerates `dumps/<fixture>/<tag>/` for the
fixture corpus (gitignored; regenerate any time). `oracle/run_scores.sh` prints
CLI scores for all pairs. `oracle/gen_fixtures.py` regenerates
`tests/fixtures/` deterministically (seeded) — any change invalidates goldens.

## Default-path invariance (M0 evidence, 2026-09-08)

- 14/14 fixture scores identical pre/post patch (`oracle/scores_prepatch.txt`).
- Assembly diff of the default build: 5287/5287 lines, only 2 instructions
  differ — the `__LINE__` immediates of the two `JXL_CHECK` sites
  (`$436→$609`, `$438→$611`). Byte-identity of the PE is unattainable for any
  in-file patch for this reason; asm-identity is the stronger, achieved bar.
- Dump-build scores == default scores on all fixtures.
- 1008 dump files × 2 runs: SHA-256 all match (byte-reproducible).

## Findings pinned by the dumps

- Gray inputs reach XYB as **3 replicated planes** (`ch=3` in
  `linear_orig` of the gray fixture) — the plan §2 open question is closed.
- `s8` fixture → 2 scales; `photo` → 5 scales (128×96: s5 would be 4×3 < 8).
  Weight-index shift with missing scales is exercised by the near-cutoff set.
- Alpha fixtures produce runs r0 (bg=0.1) and r1 (bg=0.9); CLI takes the min.
