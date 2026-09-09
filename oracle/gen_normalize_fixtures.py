#!/usr/bin/env python3
"""Generate the normalize-policy fixtures committed under tests/fixtures/:
  opaque_orig/opaque_dist.png  RGBA with A=255 everywhere (lossless-strip case)
  gama_orig.png                photo_orig + gAMA/cHRM chunks injected (tolerated by ingest)
  icc_orig.png                 photo_orig + dummy iCCP chunk (rejected everywhere)
Chunk bytes are real PNG chunks (CRC32 over type+data); pixel payloads are the
photo fixtures unchanged, so scores of the chunked files must equal the plain
ones. Run from repo root:  python oracle/gen_normalize_fixtures.py
"""
import struct
import zlib
from PIL import Image

SRC_O = "tests/fixtures/photo_orig.png"
SRC_D = "tests/fixtures/photo_dist.png"


def chunk(tag: bytes, data: bytes) -> bytes:
    return (struct.pack(">I", len(data)) + tag + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF))


def inject(src: str, dst: str, chunks: bytes) -> None:
    b = open(src, "rb").read()
    # insert after IHDR: signature(8) + len(4)+type(4)+data(13)+crc(4) = 33
    pos = 8 + 4 + 4 + 13 + 4
    open(dst, "wb").write(b[:pos] + chunks + b[pos:])


def main():
    for src, dst in ((SRC_O, "tests/fixtures/opaque_orig.png"),
                     (SRC_D, "tests/fixtures/opaque_dist.png")):
        im = Image.open(src).convert("RGBA")
        im.putalpha(Image.new("L", im.size, 255))
        im.save(dst)
    gama = chunk(b"gAMA", struct.pack(">I", 45455))          # 1/2.2
    chrm = chunk(b"cHRM", struct.pack(">8I", 31270, 32900, 64000, 33000,
                                       30000, 60000, 15000, 6000))
    inject(SRC_O, "tests/fixtures/gama_orig.png", gama + chrm)
    iccp = chunk(b"iCCP", b"profile\x00\x00" + zlib.compress(b""))
    inject(SRC_O, "tests/fixtures/icc_orig.png", iccp)
    print("wrote 4 fixtures")


if __name__ == "__main__":
    main()
