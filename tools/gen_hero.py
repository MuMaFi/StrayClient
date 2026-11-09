"""Generates ui/assets/hero.svg – an original pixel-art sunset for the launcher hero.
Rasterize to ui/assets/hero.png (192x96) afterwards; the UI scales it with pixelated rendering."""
import math, random, sys

W, H = 192, 96
px = {}
def put(x, y, c):
    if 0 <= x < W and 0 <= y < H: px[(x, y)] = c
def lerp(a, b, t): return tuple(round(a[i] + (b[i] - a[i]) * t) for i in range(3))
def hexc(c): return '#%02x%02x%02x' % c

stops = [(0, (36, 30, 72)), (0.25, (78, 50, 120)), (0.45, (150, 74, 140)), (0.62, (214, 108, 120)), (0.78, (240, 150, 100)), (1, (246, 190, 110))]
def sky(t):
    for (a, ca), (b, cb) in zip(stops, stops[1:]):
        if t <= b: return lerp(ca, cb, (t - a) / (b - a))
    return stops[-1][1]
for y in range(H):
    t = min(1, y / (H * 0.7))
    t = round(t * 22) / 22          # 22 soft bands
    for x in range(W): put(x, y, hexc(sky(t)))

random.seed(7)
for _ in range(36):
    x, y = random.randrange(W), random.randrange(int(H * 0.28))
    put(x, y, '#e9e1ff' if random.random() < .5 else '#b9a8f0')

cx, cy, r = 138, 54, 11                                     # sun + halo
for y in range(cy - r - 5, cy + r + 6):
    for x in range(cx - r - 5, cx + r + 6):
        d = math.hypot(x - cx, y - cy)
        if d <= r - 3: put(x, y, '#ffe6a6')
        elif d <= r: put(x, y, '#ffcf7e')
        elif d <= r + 4 and (x + y) % 2 == 0: put(x, y, '#f8b878')

def cloud(x0, y0, w, col, sh):
    for i in range(w):
        h = 2 + int(2 * math.sin(i / w * math.pi)) + (1 if i % 5 == 2 else 0)
        for j in range(h): put(x0 + i, y0 - j, col)
        put(x0 + i, y0 + 1, sh)
cloud(18, 24, 26, '#c98fbf', '#a97aa8'); cloud(70, 16, 18, '#b886b8', '#9a72a3')
cloud(150, 34, 22, '#f7c3b4', '#d99a92'); cloud(96, 38, 14, '#f2b4ae', '#d6948f')

def ridge(base, amp, freq, col, seed):
    random.seed(seed); off = random.random() * 10
    for x in range(W):
        h = base - int(amp * (0.5 + 0.5 * math.sin(x * freq + off)) * (0.7 + 0.3 * math.sin(x * freq * 2.7 + off)))
        h -= h % 2
        for y in range(h, H): put(x, y, col)
ridge(66, 20, 0.045, '#5a3a78', 1); ridge(72, 16, 0.07, '#43305f', 2); ridge(80, 10, 0.11, '#2e2546', 3)

for x in range(W):                                           # ground
    g = 84 + (1 if (x // 6) % 3 == 0 else 0)
    for y in range(g, H): put(x, y, '#3b2a2a' if y > g + 2 else '#4c7a3a')
    put(x, g, '#6fa54a')
    if x % 4 == 0: put(x, g - 1, '#6fa54a')

def tree(x0, base, h):
    for y in range(h):
        w = 1 + (h - y) // 3
        for dx in range(-w, w + 1): put(x0 + dx, base - y, '#1f1a30' if dx <= 0 else '#2a2240')
for x, h in [(8, 22), (16, 16), (30, 26), (172, 24), (182, 18), (188, 28), (60, 14), (122, 12)]: tree(x, 84, h)

def lantern(x0, base):
    for dy in range(-22, -9):                                 # soft glow
        for dx in range(-6, 7):
            d = math.hypot(dx, dy + 16)
            if d < 3.5: put(x0 + dx, base + dy, '#d99a78')
            elif d < 6 and (x0 + dx, base + dy) in px: put(x0 + dx, base + dy, '#8a5a86' if px[(x0 + dx, base + dy)] in ('#43305f', '#2e2546', '#5a3a78') else '#e0a07c')
    for y in range(base - 14, base + 1): put(x0, y, '#2a1e1a')
    for dx in range(-1, 2):
        for dy in range(-18, -14): put(x0 + dx, base + dy, '#ffd36e')
    put(x0, base - 19, '#2a1e1a')
lantern(48, 84); lantern(150, 84)

fx, fy = 98, 84                                               # campfire
for dx in range(-3, 4): put(fx + dx, fy, '#5b3a22')
for dx, dy, c in [(0, -1, '#ffb347'), (0, -2, '#ff8c3a'), (-1, -1, '#ff7a2f'), (1, -1, '#ff7a2f'), (0, -3, '#ffd36e'), (-1, -2, '#ffb347'), (1, -2, '#ffd36e'), (0, -4, '#ff8c3a')]:
    put(fx + dx, fy + dy, c)

rows = {}
for (x, y), c in px.items(): rows.setdefault(y, {})[x] = c
out = []
for y in sorted(rows):
    xs = sorted(rows[y]); i = 0
    while i < len(xs):
        x = xs[i]; c = rows[y][x]; j = i
        while j + 1 < len(xs) and xs[j + 1] == xs[j] + 1 and rows[y][xs[j + 1]] == c: j += 1
        out.append(f'<rect x="{x}" y="{y}" width="{xs[j] - x + 1}" height="1" fill="{c}"/>'); i = j + 1
open(sys.argv[1] if len(sys.argv) > 1 else 'hero.svg', 'w').write(
    f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" shape-rendering="crispEdges">' + ''.join(out) + '</svg>')
