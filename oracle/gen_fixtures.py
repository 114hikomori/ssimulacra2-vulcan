# Deterministic fixture generator for the Vulkan port parity corpus.
# Run: python oracle/gen_fixtures.py  (writes tests/fixtures/*.png)
# Any change here invalidates all goldens - regenerate deliberately.
import numpy as np
from PIL import Image
import os

rng = np.random.default_rng(1234)
OUT = os.path.join(os.path.dirname(__file__), "..", "tests", "fixtures")
os.makedirs(OUT, exist_ok=True)


def clamp8(a):
    return np.clip(np.round(a), 0, 255).astype(np.uint8)


def photo(w, h, seed_off=0, noise=9.0, stripes_on=True):
    y, x = np.mgrid[0:h, 0:w].astype(np.float64)
    base = 40 + 120 * (x / w) + 60 * np.sin(3.0 * y / h + 1.0)
    cx, cy = w * 0.35, h * 0.4
    disc = np.sqrt((x - cx) ** 2 + (y - cy) ** 2) < min(w, h) * 0.22
    base = np.where(disc, 200 - 80 * (disc * (x / w)), base)
    if stripes_on:
        stripes = ((x + y).astype(np.int64) // 7) % 2 == 0
        base = np.where(stripes, base * 0.55, base)
    g = rng.random((h, w)) * 2 * noise - noise
    return base + g


def to_rgb(arr, gnoise=20.0):
    r = clamp8(arr)
    g = clamp8(0.5 * arr + 0.5 * rng.random(arr.shape) * gnoise + 20)
    b = clamp8(255 - arr * 0.8)
    return np.dstack([r, g, b])


def distort(arr, noise=3.0):
    d = np.round(arr / 9.0) * 9.0  # quantization
    h, w = arr.shape
    ph, pw = -h % 8, -w % 8
    dp = np.pad(d, ((0, ph), (0, pw)), mode="edge") if (ph or pw) else d
    H, W = dp.shape[0] // 8, dp.shape[1] // 8
    blocks = dp.reshape(H, 8, W, 8).mean(axis=(1, 3))
    blocks = np.round(blocks / 12.0) * 12.0  # blockiness
    blk = np.repeat(np.repeat(blocks, 8, 0), 8, 1)[:h, :w]
    d = d + blk * 0.35
    edge = np.abs(np.diff(arr, axis=1)) > 60
    ring = np.zeros_like(d)
    ring[:, 1:] += edge * 14
    ring[:, :-1] -= edge * 10  # ringing at edges
    d = d + ring + rng.random(arr.shape) * 2 * noise - noise
    return d


def save(arr, name, mode="RGB"):
    if mode == "RGB":
        Image.fromarray(clamp8(arr), "RGB").save(os.path.join(OUT, name))
    elif mode == "L":
        Image.fromarray(clamp8(arr), "L").save(os.path.join(OUT, name))


def save_rgba(rgb, alpha, name):
    a = clamp8(alpha)
    Image.fromarray(np.dstack([clamp8(rgb), a]), "RGBA").save(
        os.path.join(OUT, name))


def main():
    # photographic-ish pair
    p = photo(128, 96)
    save(to_rgb(p), "photo_orig.png")
    save(to_rgb(distort(p)), "photo_dist.png")

    # smooth gradient pair
    y, x = np.mgrid[0:64, 0:64].astype(np.float64)
    gr = 30 + 180 * (x / 63.0) * (0.6 + 0.4 * y / 63.0)
    save(to_rgb(gr), "grad_orig.png")
    save(to_rgb(gr + 6 * np.sin(9 * x / 64) + rng.random(gr.shape) * 4),
         "grad_dist.png")

    # pure noise pair
    n0 = rng.random((64, 64)) * 255
    save(to_rgb(n0), "noise_orig.png")
    save(to_rgb(np.round(n0 / 16) * 16), "noise_dist.png")

    # hard step edges + overshoot ringing
    st = np.where((x % 32 < 16) ^ (y % 32 < 16), 230.0, 25.0)
    save(to_rgb(st), "step_orig.png")
    ov = np.zeros_like(st)
    ov[:, 1:] += (st[:, 1:] - st[:, :-1]) * 0.35
    save(to_rgb(st + ov + rng.random(st.shape) * 3), "step_dist.png")

    # odd dimensions
    o = photo(101, 67)
    save(to_rgb(o), "odd_orig.png")
    save(to_rgb(distort(o)), "odd_dist.png")

    # near-cutoff sizes (exercise the <8 break rule and weight-index shift)
    for s in (8, 9, 12, 15):
        a = photo(s, s)
        save(to_rgb(a), f"s{s}_orig.png")
        save(to_rgb(distort(a)), f"s{s}_dist.png")

    # sub-cutoff (CLI must reject)
    a = photo(7, 7)
    save(to_rgb(a), "s7_orig.png")
    save(to_rgb(distort(a)), "s7_dist.png")

    # alpha pair (worst-of-bg path)
    ap = photo(64, 64)
    ay, ax = np.mgrid[0:64, 0:64].astype(np.float64)
    alpha = 255 * np.clip(0.5 + 0.5 * np.sin(ax / 9) * np.cos(ay / 11), 0, 1)
    save_rgba(to_rgb(ap), alpha, "alpha_orig.png")
    save_rgba(to_rgb(distort(ap)), np.clip(alpha + 30, 0, 255),
              "alpha_dist.png")

    # gray pair
    gp = photo(64, 64)
    save(gp, "gray_orig.png", mode="L")
    save(distort(gp), "gray_dist.png", mode="L")

    # large pair (perf + memory; score-only golden) - smooth content keeps it small
    bp = photo(2048, 2048, noise=0.0, stripes_on=False)
    save(to_rgb(bp, gnoise=0.0), "big_orig.png")
    save(to_rgb(distort(bp, noise=0.0)), "big_dist.png")

    print("fixtures written to", os.path.normpath(OUT))


if __name__ == "__main__":
    main()
