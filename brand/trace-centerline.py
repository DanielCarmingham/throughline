"""Trace the Throughline logo from image.png into an animatable SVG centerline.

The logo is built as a wide cyan body with a thin white ribbon running down
its middle. That white ribbon IS the path, so we threshold it, thin it to a
one-pixel skeleton, walk the skeleton into an ordered polyline, simplify, and
emit smooth cubic beziers. The result can be stroked at any width, recoloured
per theme, and revealed with stroke-dashoffset.
"""

import numpy as np
from PIL import Image

SRC = "/Users/daniel/Developer/DanielCarmingham/throughline/image.png"


def load_mask():
    im = Image.open(SRC).convert("RGB")
    a = np.asarray(im).astype(np.int16)
    r, g, b = a[..., 0], a[..., 1], a[..., 2]
    # The inner highlight is the only near-white region; the cyan body and
    # glow both have a strong blue bias, so requiring a small red/blue gap
    # isolates the ribbon cleanly.
    white = (r > 175) & (g > 185) & (b > 185) & ((b - r) < 45)
    return white


def zhang_suen(img):
    """Thin a binary mask to a 1px skeleton."""
    img = img.astype(np.uint8).copy()

    def neighbours(p):
        return [p[0, 1], p[0, 2], p[1, 2], p[2, 2], p[2, 1], p[2, 0], p[1, 0], p[0, 0]]

    changed = True
    while changed:
        changed = False
        for step in (0, 1):
            pad = np.pad(img, 1)
            P = [pad[0:-2, 1:-1], pad[0:-2, 2:], pad[1:-1, 2:], pad[2:, 2:],
                 pad[2:, 1:-1], pad[2:, 0:-2], pad[1:-1, 0:-2], pad[0:-2, 0:-2]]
            P2, P3, P4, P5, P6, P7, P8, P9 = P
            B = sum(P)
            seq = [P2, P3, P4, P5, P6, P7, P8, P9, P2]
            A = sum(((seq[i] == 0) & (seq[i + 1] == 1)).astype(np.uint8) for i in range(8))
            if step == 0:
                c1 = P2 * P4 * P6
                c2 = P4 * P6 * P8
            else:
                c1 = P2 * P4 * P8
                c2 = P2 * P6 * P8
            kill = (img == 1) & (B >= 2) & (B <= 6) & (A == 1) & (c1 == 0) & (c2 == 0)
            if kill.any():
                img[kill] = 0
                changed = True
    return img


NB = [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)]


def longest_path(skel):
    pts = {(int(y), int(x)) for y, x in zip(*np.nonzero(skel))}

    def nbrs(p):
        y, x = p
        return [(y + dy, x + dx) for dy, dx in NB if (y + dy, x + dx) in pts]

    ends = [p for p in pts if len(nbrs(p)) == 1]
    if not ends:
        ends = [min(pts)]

    def bfs(start):
        prev = {start: None}
        order = [start]
        i = 0
        while i < len(order):
            cur = order[i]
            i += 1
            for n in nbrs(cur):
                if n not in prev:
                    prev[n] = cur
                    order.append(n)
        return prev, order[-1]

    # Double BFS gives the graph diameter — the true run of the mark, so
    # branches into the arrowhead get discarded.
    _, far = bfs(ends[0])
    prev, other = bfs(far)
    path = []
    cur = other
    while cur is not None:
        path.append(cur)
        cur = prev[cur]
    return path


def rdp(points, eps):
    if len(points) < 3:
        return points
    start, end = np.array(points[0]), np.array(points[-1])
    seg = end - start
    n = np.hypot(*seg)
    if n == 0:
        d = [np.hypot(*(np.array(p) - start)) for p in points]
    else:
        # numpy 2 removed the 2-D cross product; z of the 3-D cross is all we need.
        d = [
            abs(seg[0] * (np.array(p) - start)[1] - seg[1] * (np.array(p) - start)[0]) / n
            for p in points
        ]
    i = int(np.argmax(d))
    if d[i] > eps:
        return rdp(points[: i + 1], eps)[:-1] + rdp(points[i:], eps)
    return [points[0], points[-1]]


def to_bezier(pts, tension=0.5):
    """Catmull-Rom through the points, emitted as cubic beziers."""
    d = [f"M{pts[0][0]:.1f} {pts[0][1]:.1f}"]
    ext = [pts[0]] + list(pts) + [pts[-1]]
    for i in range(1, len(ext) - 2):
        p0, p1, p2, p3 = ext[i - 1], ext[i], ext[i + 1], ext[i + 2]
        c1 = (p1[0] + (p2[0] - p0[0]) / 6 * tension * 2, p1[1] + (p2[1] - p0[1]) / 6 * tension * 2)
        c2 = (p2[0] - (p3[0] - p1[0]) / 6 * tension * 2, p2[1] - (p3[1] - p1[1]) / 6 * tension * 2)
        d.append(f"C{c1[0]:.1f} {c1[1]:.1f} {c2[0]:.1f} {c2[1]:.1f} {p2[0]:.1f} {p2[1]:.1f}")
    return " ".join(d)


mask = load_mask()
ys, xs = np.nonzero(mask)
print(f"white pixels: {mask.sum()}  bbox x {xs.min()}-{xs.max()} y {ys.min()}-{ys.max()}")

skel = zhang_suen(mask)
print(f"skeleton pixels: {skel.sum()}")

path_px = longest_path(skel)
print(f"longest run: {len(path_px)} px")

# (row, col) -> (x, y), normalised into a 0..300 wide viewBox.
raw = [(float(c), float(r)) for r, c in path_px]
x0 = min(p[0] for p in raw)
y0 = min(p[1] for p in raw)
x1 = max(p[0] for p in raw)
y1 = max(p[1] for p in raw)
scale = 300.0 / (x1 - x0)
norm = [((p[0] - x0) * scale, (p[1] - y0) * scale) for p in raw]

# Orient the path so it ENDS at the arrow: the animation must draw the
# curve first, then run out to the head.
if norm[0][0] > norm[-1][0]:
    norm.reverse()

# The arrowhead is a solid triangle, so the skeleton runs into it and back
# down its edge. The ribbon ends at the rightmost point; everything after
# that belongs to the head, which is drawn as its own shape.
cut = max(range(len(norm)), key=lambda i: norm[i][0])
norm = norm[: cut + 1]

# The tail of the mark is a genuinely straight run. Thinning wobbles through
# it and picks up spurs, so detect the run and emit one clean line — that is
# closer to the artwork than a traced approximation.
tail = [p for p in norm if p[0] > 180]
y_run = float(np.median([p[1] for p in tail]))
in_band = [i for i, p in enumerate(norm) if abs(p[1] - y_run) < 4.0]
entry, last = in_band[0], in_band[-1]
x_end = max(norm[i][0] for i in in_band)
print(f"straight run: y={y_run:.1f}, from x={norm[entry][0]:.1f} to x={x_end:.1f}")

curve = rdp(norm[: entry + 1], eps=1.2)
print(f"curve simplified to {len(curve)} points")

d = to_bezier(curve) + f" L{x_end:.1f} {y_run:.1f}"

hy = max(max(p[1] for p in curve), y_run)
print(f"viewBox 0 0 {max(p[0] for p in curve + [(x_end, 0)]):.1f} {hy:.1f}")
print()
print(d)
