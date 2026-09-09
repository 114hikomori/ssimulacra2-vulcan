# Engine-routing calibration (M10-batch decision 3, wired 2026-09-09).
#
# COMPARATOR: this binary's own CPU fallback engine (`--cpu` = the in-process
#   Rust reference, compute_ssimulacra2_cpu) -- NOT the C++ oracle. Routing
#   chooses between the two engines the binary can actually run, so the
#   oracle comparison answers a different question (see README "two
#   crossovers"); conflating them is the error class behind the stale
#   >=8MP proposal.
# PROTOCOL: single-pair CLI invocation per measurement (context-init inside
#   the timed region, as production pays it), fixture file cache warmed by
#   one untimed run per side first, MIN of --reps timed runs per engine,
#   engines interleaved. Run on a quiet machine; this host swings ~150ms
#   batch-to-batch.
#
# Usage: python bench/routing_calib.py [--reps 3] [--exe target/release/ssimulacra2-vulkan.exe]
import argparse
import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from gen_batch import make_pair  # noqa: E402

# w, h, label - spans the 0.3MP (GPU loses) to 4.2MP (GPU wins 4.3x) band.
SIZES = [
    (480, 320, "0.15MP"),
    (640, 480, "0.31MP"),
    (850, 640, "0.54MP"),
    (900, 675, "0.61MP"),
    (1280, 800, "1.02MP"),
    (1600, 1250, "2.00MP"),
    (2048, 1600, "3.28MP"),
    (2048, 2048, "4.19MP"),
]


def time_run(exe, flags, orig, dist):
    t0 = time.perf_counter()
    r = subprocess.run([exe, *flags, orig, dist], capture_output=True)
    if r.returncode != 0:
        raise SystemExit(f"cli failed: {r.stderr.decode(errors='replace')}")
    return (time.perf_counter() - t0) * 1e3


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--reps", type=int, default=3)
    ap.add_argument("--exe", default=os.path.join(
        HERE, "..", "target", "release",
        "ssimulacra2-vulkan.exe" if os.name == "nt" else "ssimulacra2-vulkan"))
    args = ap.parse_args()
    exe = os.path.abspath(args.exe)

    dirs = []
    for w, h, label in SIZES:
        d = os.path.join(HERE, f"fixtures_route_{label}")
        make_pair(w, h, d)
        dirs.append((label, d))

    print(f"\nrouting calibration | comparator: in-binary Rust CPU (--cpu) | "
          f"MIN-of-{args.reps}, warmed, quiet machine")
    print(f"{'size':>8} {'CPU ms':>8} {'GPU ms':>8} {'GPU/CPU':>8}")
    for label, d in dirs:
        o, v = os.path.join(d, "orig.png"), os.path.join(d, "vars", "v000.png")
        # --gpu pins the GPU engine: post-wiring the DEFAULT routes by size,
        # so measuring with [] would report the routed (sometimes-CPU) path
        # below the threshold and silently contaminate the crossover column.
        time_run(exe, ["--cpu"], o, v)   # warm file cache both sides
        time_run(exe, ["--gpu"], o, v)
        c, g = [], []
        for _ in range(args.reps):
            c.append(time_run(exe, ["--cpu"], o, v))
            g.append(time_run(exe, ["--gpu"], o, v))
        cmin, gmin = min(c), min(g)
        print(f"{label:>8} {cmin:8.0f} {gmin:8.0f} {gmin / cmin:8.2f}")
    print("\n(reference: pre-wiring run, GPU implicit-default, crossover ~0.45MP")
    print("(1.44@0.31, 0.81@0.61); post-wiring pinned run ~0.4MP (1.17@0.31,")
    print("0.72@0.54). Wired GPU_ROUTE_MIN_PIXELS = 500_000 sits on the")
    print("GPU-winning side of both. NOTE: any re-run must keep the --gpu")
    print("pin - measuring [] after wiring reports the routed path.)")


if __name__ == "__main__":
    main()
