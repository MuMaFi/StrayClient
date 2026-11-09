"""Generates ui/assets/demo-skin.png – the built-in "StraySMP" skin (64×64, classic arms).
A navy hoodie with the blue pixel orb on the chest, dark jeans and blue eyes."""
import random
import struct
import sys
import zlib

random.seed(4)
W = H = 64
px = [[(0, 0, 0, 0)] * W for _ in range(H)]

SKIN, SKIN_D = (198, 140, 106), (176, 120, 90)
HAIR, HAIR_D = (42, 30, 26), (30, 21, 18)
HOOD, HOOD_D, HOOD_L = (30, 31, 40), (24, 25, 33), (47, 48, 64)
BLUE, BLUE_L, BLUE_D = (81, 162, 255), (140, 200, 255), (28, 111, 181)
JEANS, JEANS_D = (43, 47, 69), (34, 37, 55)
SHOE = (20, 20, 24)


def put(x, y, c):
    px[y][x] = (*c, 255)


def fill(x0, y0, w, h, a, b=None):
    for y in range(h):
        for x in range(w):
            put(x0 + x, y0 + y, b if b and random.random() < 0.18 else a)


def box(x0, y0, w, h, d, a, b=None):
    """Fill all six faces of a Minecraft skin cuboid (w×h×d) at texture origin x0,y0."""
    fill(x0 + d, y0, w, d, a, b)              # top
    fill(x0 + d + w, y0, w, d, a, b)          # bottom
    fill(x0, y0 + d, d, h, a, b)              # right
    fill(x0 + d, y0 + d, w, h, a, b)          # front
    fill(x0 + d + w, y0 + d, d, h, a, b)      # left
    fill(x0 + 2 * d + w, y0 + d, w, h, a, b)  # back


# head (8×8×8 at 0,0)
box(0, 0, 8, 8, 8, SKIN, SKIN_D)
fill(8, 0, 8, 8, HAIR, HAIR_D)              # top: hair
fill(24, 8, 8, 8, HAIR, HAIR_D)             # back: hair
for face_x in (0, 16):                       # sides: hair on the upper half
    fill(face_x, 8, 8, 4, HAIR, HAIR_D)
fill(8, 8, 8, 2, HAIR, HAIR_D)              # fringe
put(8, 10, HAIR); put(15, 10, HAIR)
for x, c in ((9, (255, 255, 255)), (10, BLUE), (13, BLUE), (14, (255, 255, 255))):
    put(x, 12, c)                            # eyes
for x in (11, 12):
    put(x, 13, SKIN_D)                       # nose shadow
for x in range(10, 14):
    put(x, 14, (150, 92, 72))                # mouth

# body (8×12×4 at 16,16): hoodie
box(16, 16, 8, 12, 4, HOOD, HOOD_D)
for y in range(20, 32):                      # zip line
    put(23 if y % 2 else 24, y, HOOD_L)
orb = ["..##..", ".#LL#.", "#LBBB#", "#BBBD#", ".#BD#.", "..##.."]
cols = {"#": BLUE_D, "L": BLUE_L, "B": BLUE, "D": BLUE_D}
for j, row in enumerate(orb):                # chest logo
    for i, ch in enumerate(row):
        if ch != ".":
            put(21 + i, 22 + j, cols[ch])
fill(20, 31, 8, 1, HOOD_L)                   # hem

# arms (4×12×4): right at 40,16 – left at 32,48
for ax, ay in ((40, 16), (32, 48)):
    box(ax, ay, 4, 12, 4, HOOD, HOOD_D)
    fill(ax + 4, ay + 13, 4, 3, SKIN, SKIN_D)    # hands (front)
    fill(ax, ay + 13, 4, 3, SKIN, SKIN_D)        # hands (side)
    fill(ax + 8, ay + 13, 4, 3, SKIN, SKIN_D)
    fill(ax + 12, ay + 13, 4, 3, SKIN, SKIN_D)
    fill(ax + 4, ay + 12, 4, 1, BLUE_D)          # cuff stripe
    fill(ax + 12, ay + 12, 4, 1, BLUE_D)

# legs (4×12×4): right at 0,16 – left at 16,48
for lx, ly in ((0, 16), (16, 48)):
    box(lx, ly, 4, 12, 4, JEANS, JEANS_D)
    for face in (0, 4, 8, 12):
        fill(lx + face, ly + 14, 4, 2, SHOE)

# hood (hat layer at 32,0): only the back and sides, so the face stays visible
fill(56, 8, 8, 8, HOOD, HOOD_D)              # back
fill(32, 8, 8, 5, HOOD, HOOD_D)              # right side, upper
fill(48, 8, 8, 5, HOOD, HOOD_D)              # left side, upper
fill(40, 0, 8, 8, HOOD, HOOD_D)              # top

raw = b"".join(b"\x00" + bytes(v for p in row for v in p) for row in px)


def chunk(t, d):
    return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)


png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
open(sys.argv[1] if len(sys.argv) > 1 else "demo-skin.png", "wb").write(png)
print(len(png), "bytes")
