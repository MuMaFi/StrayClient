"""Generates ui/assets/demo-cape.png – the built-in "StraySMP" cape (64×32, vanilla cape layout).

Layout (in pixels of a 64×32 texture):
  front 1..11 × 1..17, back 12..22 × 1..17, edges around them, elytra area 22..46 × 0..22.
"""
import math
import struct
import sys
import zlib

W, H = 64, 32
NAVY, NAVY2, EDGE = (26, 27, 36), (34, 36, 50), (47, 48, 64)
ORB = [(11, 42, 74), (15, 61, 107), (20, 85, 143), (28, 111, 181), (46, 139, 214), (81, 162, 255), (140, 200, 255), (214, 236, 255)]
px = [[(0, 0, 0, 0)] * W for _ in range(H)]


def put(x, y, c, a=255):
    if 0 <= x < W and 0 <= y < H:
        px[y][x] = (*c, a)


def panel(x0, y0, w, h, logo=True):
    for y in range(h):
        for x in range(w):
            c = NAVY if (x + y) % 2 else NAVY2
            if x in (0, w - 1) or y in (0, h - 1):
                c = ORB[5]  # blue trim like the shop's accent
            put(x0 + x, y0 + y, c)
    if not logo:
        return
    # pixel orb (like the launcher logo), centred in the upper half
    cx, cy, r = x0 + w / 2 - 0.5, y0 + 6.0, 3.2
    for y in range(h):
        for x in range(w):
            dx, dy = x0 + x - cx, y0 + y - cy
            d = math.hypot(dx, dy)
            if d <= r:
                light = (-dx * 0.5 - dy * 0.6) / r * 0.6 + math.sqrt(max(0, r * r - d * d)) / r * 0.7
                i = max(0, min(len(ORB) - 1, int((light + 0.25) * len(ORB) / 1.25)))
                put(x0 + x, y0 + y, ORB[i])
    # small "S" stripes below the orb
    for x in range(2, w - 2):
        put(x0 + x, y0 + 11, ORB[4] if x % 2 else ORB[3])
        put(x0 + x, y0 + 13, ORB[3] if x % 2 else ORB[4])


panel(1, 1, 10, 16)               # front
panel(12, 1, 10, 16, logo=False)  # back (inside)
for y in range(1, 17):            # sides
    put(0, y, EDGE)
    put(11, y, EDGE)
for x in range(1, 11):            # top / bottom
    put(x, 0, EDGE)
    put(x + 10, 0, EDGE)
panel(22, 0, 24, 22, logo=False)  # elytra
for x in range(24, 44, 3):
    for y in range(3, 20):
        if (y + x) % 4 == 0:
            put(x, y, ORB[4])

raw = b"".join(b"\x00" + bytes(v for p in row for v in p) for row in px)


def chunk(t, d):
    return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)


png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
open(sys.argv[1] if len(sys.argv) > 1 else "demo-cape.png", "wb").write(png)
print(len(png), "bytes")
