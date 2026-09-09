import subprocess, sys, time, os, re
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXE = os.path.join(ROOT, "target", "release", "ssimulacra2-vulkan.exe")
ORIG = os.path.join(ROOT, "bench", "fixtures4k", "orig.png")
VARS = [os.path.join(ROOT, "bench", "fixtures4k", "vars", f"v{i:03d}.png") for i in range(4)]

for n in (4, 2, 1):
    procs, t0 = [], time.perf_counter()
    for i in range(n):
        procs.append(subprocess.Popen(
            [EXE, "--gpu", ORIG, VARS[i]],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True))
    outs = [p.communicate() for p in procs]
    wall = time.perf_counter() - t0
    print(f"=== --gpu x {n}: wall {wall:.2f}s  effective {wall/n:.2f} s/pair ===")
    for i, (o, e) in enumerate(outs):
        oom = "OOM->CPU-FALLBACK" if "OUT_OF_DEVICE_MEMORY" in e else (
              "FALLBACK" if "falling back" in e else "gpu-clean")
        print(f"  pair{i}: score={o.strip() or '(none)'}  [{oom}]")
