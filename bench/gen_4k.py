# True-4K UHD (3840x2160) benchmark class for the batch-phase spec (M10-batch,
# checklist 2b). Reuses the SAME content generators as oracle/gen_fixtures.py so
# structure matches the parity corpus; written under bench/fixtures4k/ (not the
# golden corpus - these are benchmark data, not CI fixtures).
import os
import sys

import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "oracle"))
import gen_fixtures as G  # noqa: E402  (module-level rng seeds deterministically)

OUT = os.path.join(HERE, "fixtures4k")
os.makedirs(OUT, exist_ok=True)

p = G.photo(3840, 2160)
Image.fromarray(G.to_rgb(p), "RGB").save(os.path.join(OUT, "orig.png"))
Image.fromarray(G.to_rgb(G.distort(p)), "RGB").save(os.path.join(OUT, "dist.png"))
for name in ("orig.png", "dist.png"):
    im = Image.open(os.path.join(OUT, name))
    print(name, im.size, os.path.getsize(os.path.join(OUT, name)), "bytes")
