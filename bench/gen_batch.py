# Generate the M10-batch corpus: one 4K original + N variants (distorted at a
# spread of noise levels, as a codec sweep would vary compression), plus the
# intermediate-resolution pairs used to calibrate the small-image routing
# threshold (decision 3: measure the crossover, do not guess it).
import os
import sys

import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "oracle"))
import gen_fixtures as G  # noqa: E402

BASE = os.path.join(HERE, "fixtures4k")


def make_pair(w, h, outdir, n_variants=0):
    os.makedirs(os.path.join(outdir, "vars"), exist_ok=True)
    p = G.photo(w, h)
    Image.fromarray(G.to_rgb(p), "RGB").save(os.path.join(outdir, "orig.png"))
    Image.fromarray(G.to_rgb(G.distort(p)), "RGB").save(os.path.join(outdir, "vars", "v000.png"))
    for i in range(1, n_variants):
        nz = 3.0 + i * 0.9
        arr = G.to_rgb(G.distort(p, noise=nz))
        Image.fromarray(arr, "RGB").save(os.path.join(outdir, "vars", f"v{i:03d}.png"))
    print(outdir, f"{w}x{h}", n_variants, "variants")


if __name__ == "__main__":
    make_pair(3840, 2160, BASE, n_variants=20)
    # Routing-threshold calibration sizes (~4.9MP, ~6.3MP): bracket the
    # crossover between 2048^2 (4.2MP, GPU loses) and 4K (8.3MP, GPU wins).
    make_pair(2560, 1920, os.path.join(HERE, "fixtures5mp"), n_variants=1)
    make_pair(3072, 2048, os.path.join(HERE, "fixtures6p3mp"), n_variants=1)
